use aex::{
    connection::{
        context::TypeMapExt,
        global::GlobalContext,
        types::{ConnectionEntry, NetworkScope},
    },
    crypto::session_key_manager::PairedSessionKey,
    server::{HTTPServer, Server},
    storage::Storage,
    tcp::{router::Router, types::Command},
};
use chrono::Utc;
use clap::Parser;
use std::{collections::HashSet, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;
use zz_account::address::FreeWebMovementAddress;

use crate::{
    cli::{Cli, Opt},
    io_storage::{IOStorage, io_stroage_init},
    protocols::{command::P2PCommand, frame::P2PFrame},
    record::{NodeRecord, NodeRegistry},
};

// 用于保存节点的所有信息
// 用于当前程序的基本信息共享

#[derive(Clone)]
pub struct Node {
    pub id: FreeWebMovementAddress,
    pub inner: NodeRegistry,
    pub external: NodeRegistry,
    pub storage: Arc<Storage>,
    pub io_storage: IOStorage, // Unique network address of the node, also id for the node
    pub name: String,          // User defined name for the node, no need to be unique
    pub addr: SocketAddr,
    pub context: Arc<GlobalContext>, // global context
    pub server: Server,
    pub cli: Arc<Cli>,
}

impl Node {
    pub async fn new(
        name: String,
        storage: Arc<Storage>,
        io_storage: IOStorage,
        addr: SocketAddr,
        context: Arc<GlobalContext>,
        server: Server,
        cli: Arc<Cli>,
    ) -> Self {
        let id = io_storage
            .read::<FreeWebMovementAddress>("address", (*storage).clone())
            .await
            .unwrap();
        let inner_nodes = io_storage
            .read::<HashSet<NodeRecord>>("inner_server", (*storage).clone())
            .await
            .unwrap();
        let external_nodes = io_storage
            .read::<HashSet<NodeRecord>>("external_server", (*storage).clone())
            .await
            .unwrap();
        let inner = NodeRegistry::new(inner_nodes);
        let external = NodeRegistry::new(external_nodes);
        Self {
            name,
            id,
            inner,
            external,
            storage,
            io_storage,
            addr,
            context,
            server,
            cli,
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
            let _ = manager
                .connect(endpoint.clone(), gc, move |ctx, _token| {
                    // 再次克隆引用以进入 async move 块
                    let rc_inner = rc.clone();
                    let ctx = ctx.clone();
                    async move {
                        // 1. 获取

                        // 4. 调用 router
                        let _ = rc_inner
                            .handle::<P2PFrame, P2PCommand>(ctx, Arc::new(|c: &P2PCommand| c.id()))
                            .await;
                    }
                })
                .await;
        }
    }

    pub async fn stop(&mut self) {}

    pub async fn init(opt:Opt) -> Self {

        let storage = Arc::new(Storage::new(opt.data_dir.as_deref()));
        let io_storage = io_stroage_init(&opt, storage.clone());

        // let address = files.address();

        let addr = format!("{}:{}", opt.ip.clone(), opt.port)
            .parse::<SocketAddr>()
            .unwrap();

        // let node = Node::init(opt.name.clone(), files.clone(), addr);

        // name = opt., files: StoredFiles, addr: SocketAddr

        // register(&mut router);

        // .start::<P2PFrame, P2PCommand>(Arc::new(|c| c.id()))
        // .await?;

        // cli.run().await?;
        let psk = Arc::new(Mutex::new(PairedSessionKey::new(16)));
        let global = Arc::new(GlobalContext::new(addr, Some(psk)));
        let cli = Cli::new();

        let server = {
            // let guard = node.lock().await;
            HTTPServer::new(addr, Some(global.clone()))
        };

        let router = Router::new();
        server.tcp(router);
        Node::new(
            opt.name,
            storage,
            io_storage,
            addr,
            global.clone(),
            server,
            Arc::new(cli),
        )
        .await
    }

    pub async fn start(&mut self) {
        self.server
            .start::<P2PFrame, P2PCommand>(Arc::new(|c| c.id()))
            .await
            .unwrap();
        let _ = self.cli.clone().run(self.context.clone()).await;
    }

    /// 核心功能：深度同步活跃连接的元数据到注册表
    pub async fn sync_from_connections(
        &mut self,
        connections: &dashmap::DashMap<SocketAddr, Arc<ConnectionEntry>>,
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
            let mut record = registry
                .nodes
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

    async fn  save_registries(&self) -> anyhow::Result<()> {
        self.io_storage.save::<HashSet<NodeRecord>>(
            &self.inner.nodes,
            "inner_server",
            &self.storage.clone(),
        ).await;
        self.io_storage.save::<HashSet<NodeRecord>>(
            &self.external.nodes,
            "external_server",
            &self.storage.clone(),
        ).await;
        Ok(())
    }
}
