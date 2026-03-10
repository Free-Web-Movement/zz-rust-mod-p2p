use aex::{
    connection::{
        context::TypeMapExt,
        global::GlobalContext,
        types::{ ConnectionEntry, NetworkScope },
    }, crypto::session_key_manager::PairedSessionKey, tcp::{ router::Router, types::Command }
};
use chrono::Utc;
use std::{ net::SocketAddr, sync::Arc };
use tokio::sync::Mutex;
use zz_account::address::FreeWebMovementAddress;

use crate::{
    record::{ NodeRecord, NodeRegistry },
    protocols::{ command::P2PCommand, frame::P2PFrame },
    stored_files::StoredFiles,
};

#[derive(Clone)]
pub struct Node {
    pub id: FreeWebMovementAddress,
    pub inner: NodeRegistry,
    pub external: NodeRegistry,
    pub files: StoredFiles, // Unique network address of the node, also id for the node
    pub name: String, // User defined name for the node, no need to be unique
    pub addr: SocketAddr,
    pub context: Arc<GlobalContext>, // global context
}

impl Node {
    pub fn new(
        name: String,
        files: StoredFiles,
        addr: SocketAddr,
        context: Arc<GlobalContext>
    ) -> Self {
        let id = files.clone().address();
        let inner = NodeRegistry::load_from_storage(
            &files.storage,
            &files.clone().inner_server_file
        );
        let external = NodeRegistry::load_from_storage(
            &files.storage,
            &files.clone().external_server_file
        );
        Self {
            name,
            id,
            inner,
            external,
            files,
            addr,
            context,
        }
    }

    pub async fn connect(&mut self) {
        // 获取 Router 的 Arc 引用
        let router: Arc<Router> = self.context.routers.get_value().unwrap();
        let global = self.context.clone();
        let manager = self.context.manager.clone();

        // 克隆节点列表以避免在循环中借用 self
        let nodes = self.inner.nodes.clone();

        for entry in nodes {
            let endpoint = entry.endpoint.clone();
            let rc = router.clone();
            let gc = global.clone();

            // 这里的闭包必须是 move，以确保内部变量的所有权被转移到异步任务中
            let _ = manager.connect(endpoint.clone(), move |r, w, _token| {
                // 再次克隆引用以进入 async move 块
                let rc_inner = rc.clone();
                let gc_inner = gc.clone();
                let ep_inner = endpoint.clone();

                async move {
                    // 1. 获取锁
                    let mut r_guard = r.lock().await;
                    let mut w_guard = w.lock().await;

                    // 2. 取出 reader / writer
                    let mut reader_opt = r_guard.take();
                    let mut writer_opt = w_guard.take();

                    // 3. 释放锁（非常重要）
                    drop(r_guard);
                    drop(w_guard);

                    // 4. 调用 router
                    let _ = rc_inner.handle::<P2PFrame, P2PCommand>(
                        ep_inner,
                        gc_inner,
                        &mut reader_opt,
                        &mut writer_opt,
                        Arc::new(|c: &P2PCommand| c.id())
                    ).await;

                    // 5. 放回 reader / writer
                    let mut r_guard = r.lock().await;
                    let mut w_guard = w.lock().await;

                    *r_guard = reader_opt;
                    *w_guard = writer_opt;
                }
            }).await;
        }
    }

    pub async fn stop(&mut self) {}

    pub fn init(name: String, files: StoredFiles, addr: SocketAddr) -> Arc<Mutex<Self>> {
        let psk = Arc::new(Mutex::new(PairedSessionKey::new(16)));
        let global = Arc::new(GlobalContext::new(addr, Some(psk)));
        let node = Arc::new(Mutex::new(Node::new(name, files, addr, global.clone())));
        node
    }

    /// 核心功能：深度同步活跃连接的元数据到注册表
    pub async fn sync_from_connections(
        &mut self,
        connections: &dashmap::DashMap<SocketAddr, Arc<ConnectionEntry>>
    ) {
        // 获取当前时间戳用于更新 last_seen
        let now_utc = Utc::now();

        for entry_ref in connections.iter() {
            let addr = *entry_ref.key();
            let entry = entry_ref.value();

            // 1. 识别网络范围
            let current_scope = NetworkScope::from_ip(&addr.ip());
            let registry = if current_scope == NetworkScope::Extranet {
                &mut self.external
            } else {
                &mut self.inner
            };

            // 2. 获取或创建 NodeRecord
            // 使用 take 取出以修改（因为 HashSet 元素具有不可变性限制）
            let mut record = registry.nodes
                .take(&NodeRecord::new(addr))
                .unwrap_or_else(|| NodeRecord::new(addr));

            // 3. 基础状态更新
            record.last_seen = now_utc;
            record.is_available = true;
            record.update_status(true);

            // 4. 💡 深度同步：从 ConnectionEntry 的 Option<Node> 中提取信息
            // 注意：这里的 Node 是另一个节点的快照信息，存储在 ConnectionEntry 中
            {
                let node_lock = entry.node.read().await;
                if let Some(peer_info) = &*node_lock {
                    // 同步 ID 和 名称

                    // 如果 peer_info 里有更详细的协议支持，也可以同步
                    record.protocols.extend(peer_info.protocols.iter().cloned());

                    let periods = record.periods.clone();
                    let mut periods1 = record.periods.clone();
                    match periods1.get_mut(periods.clone().len() - 1) {
                        Some(v) => {
                            v.1 = now_utc;
                        }
                        None => {
                            let v = (now_utc, now_utc);
                            record.periods.push(v);
                        }
                    };
                }
            } // 锁在此处释放

            // 5. 放回注册表
            registry.nodes.insert(record);
        }
        let _ = self.save_registries();
    }

    fn save_registries(&self) -> anyhow::Result<()> {
        self.inner.save_to_storage(&self.files.storage, &self.files.inner_server_file)?;
        self.external.save_to_storage(&self.files.storage, &self.files.external_server_file)?;
        Ok(())
    }
}
