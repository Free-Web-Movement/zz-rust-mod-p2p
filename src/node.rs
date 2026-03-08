use aex::{
    connection::{
        context::{BoxReader, BoxWriter, TypeMapExt},
        global::GlobalContext,
    },
    tcp::{router::Router, types::Command},
};
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    io::{AsyncBufRead, AsyncReadExt},
    sync::Mutex,
};
use zz_account::address::FreeWebMovementAddress;

use crate::{
    nodes::record::NodeRegistry,
    protocols::{command::P2PCommand, frame::P2PFrame},
    stored_files::StoredFiles,
};

#[derive(Clone)]
pub struct Node {
    pub id: FreeWebMovementAddress,
    pub inner: NodeRegistry,
    pub external: NodeRegistry,
    pub files: StoredFiles, // Unique network address of the node, also id for the node
    pub name: String,       // User defined name for the node, no need to be unique
    pub addr: SocketAddr,
    pub context: Arc<GlobalContext>, // global context
}

impl Node {
    pub fn new(
        name: String,
        files: StoredFiles,
        addr: SocketAddr,
        context: Arc<GlobalContext>,
    ) -> Self {
        let id = files.clone().address();
        let inner =
            NodeRegistry::load_from_storage(&files.storage, &files.clone().inner_server_file);
        let external =
            NodeRegistry::load_from_storage(&files.storage, &files.clone().external_server_file);
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
            let _ = manager
                .connect(endpoint.clone(), move |r, w, _token| {
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
                        let _ = rc_inner
                            .handle::<P2PFrame, P2PCommand>(
                                ep_inner,
                                gc_inner,
                                &mut reader_opt,
                                &mut writer_opt,
                                Arc::new(|c: &P2PCommand| c.id()),
                            )
                            .await;

                        // 5. 放回 reader / writer
                        let mut r_guard = r.lock().await;
                        let mut w_guard = w.lock().await;

                        *r_guard = reader_opt;
                        *w_guard = writer_opt;
                    }
                })
                .await;
        }
    }

    pub async fn stop(&mut self) {}

    pub fn init(name: String, files: StoredFiles, addr: SocketAddr) -> Arc<Mutex<Self>> {
        let global = Arc::new(GlobalContext::new(addr));
        let node = Arc::new(Mutex::new(Node::new(name, files, addr, global.clone())));
        node
    }
}
