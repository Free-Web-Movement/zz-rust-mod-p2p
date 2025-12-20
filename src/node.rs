use serde::ser;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;
use zz_account::address::FreeWebMovementAddress as Address;

use crate::context::Context;
use crate::nodes::net_info::NetInfo;
use crate::protocols::defines::Listener;
use crate::{
    consts::{DEFAULT_APP_DIR, DEFAULT_APP_DIR_ADDRESS_JSON_FILE},
    handlers::{tcp::TCPHandler, udp::UDPHandler},
    nodes::{net_info, record::NodeRecord, storage::Storeage},
    protocols::defines::ProtocolCapability,
};

/* =========================
   NODE
========================= */

#[derive(Clone)]
pub struct Node {
    pub net_info: Option<NetInfo>,
    pub storage: Option<Storeage>,
    pub server_list: Option<Vec<NodeRecord>>,
    pub name: String,     // User defined name for the node, no need to be unique
    pub address: Address, // Unique network address of the node
    pub ip: String,       // Bound IP address of the node
    pub port: u16,        // Bound port of the node
    pub stun_port: u16,   // STUN service port
    pub trun_port: u16,   // TURN service port
    pub start_time: u128, // Timestamp when the node was started
    pub stop_time: u128,  // Timestamp when the node was started
    pub context: Option<Arc<Context>>,
    pub tcp_handler: Option<Arc<Mutex<TCPHandler>>>,
    pub udp_handler: Option<Arc<Mutex<UDPHandler>>>,
}

impl Node {
    pub fn new(name: String, address: Address, ip: String, port: u16) -> Self {
        Self {
            name,
            address,
            ip: ip,
            port,
            stun_port: port + 1,
            trun_port: port + 2,
            tcp_handler: None,
            udp_handler: None,
            context: None,
            start_time: 0,
            stop_time: 0,
            net_info: None,
            storage: None,
            server_list: None,
        }
    }

    async fn listen<T: Listener + Send + 'static>(&self, object: T) -> Arc<Mutex<T>> {
        let handler = Arc::new(Mutex::new(object));
        let handler_clone = handler.clone();
        tokio::spawn(async move {
            let mut h = handler_clone.lock().await;
            let _ = h.run().await;
        });
        handler
    }

    // Removed incorrect generic overload that used `Arc` as a trait bound (not allowed).
    // Use the parameterless `start` implementation below to start the node's handlers.
    pub async fn start(&mut self) {
        self.start_time = timestamp();
        let ip = self.ip.clone();
        let port = self.port;

        // 节点全局共享的内容，所有持久化的信息都保存在context里面

        let context = Arc::new(Context::new(ip.clone(), port, self.address.clone()));
        self.context = Some(context.clone());

        let tcp = TCPHandler::bind(context.clone())
            .await
            .unwrap()
            .as_ref()
            .clone();
        let udp = UDPHandler::bind(context.clone())
            .await
            .unwrap()
            .as_ref()
            .clone();

        self.tcp_handler = Some(self.listen(tcp).await);
        self.udp_handler = Some(self.listen(udp).await);
        self.net_info = Some(NetInfo::collect(port).unwrap());
        self.init_storage_and_server_list(port);
        
    }

    pub async fn stop(&mut self) {
        // minimal stop implementation: close tcp and udp connections
        // take ownership of the Arcs we hold and drop them so the underlying sockets
        // are closed when there are no remaining owners

        match self.context.clone() {
            Some(context) => {
                context.token.cancel();
            }
            None => (),
        }
        self.tcp_handler.take();
        self.udp_handler.take();

        self.stop_time = timestamp();
    }

    // Client Actions

       pub fn init_storage_and_server_list(&mut self, port: u16) -> anyhow::Result<()> {
        // 1️⃣ 初始化 storage
        self.storage = Some(Storeage::new(None, None, None, None));
        let storage = self.storage.as_ref().unwrap();

        // 2️⃣ 读取现有外部 server list
        let mut external_list = storage.read_external_server_list().unwrap_or_default();

        // 3️⃣ 获取公网节点列表
        let public_nodes = NodeRecord::to_list(
            self.net_info.as_ref().unwrap().public_ips(),
            port,
            ProtocolCapability::TCP | ProtocolCapability::UDP,
        );

        // 4️⃣ 合并公网节点到外部 server list
        external_list = NodeRecord::merge(public_nodes, external_list);

        // 5️⃣ 保存当前节点地址
        storage.save_address(&self.address)?;

        // 6️⃣ 保存更新后的外部 server list
        storage.save_external_server_list(external_list.clone())?;

        // 7️⃣ 更新 Node 自身 server_list
        self.server_list = Some(external_list);

        Ok(())
    }

}

fn timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use zz_account::address::FreeWebMovementAddress as Address;
    #[tokio::test]
    async fn test_node_start_and_stop() {
        let node1 = Arc::new(Mutex::new(Node::new(
            "node".into(),
            Address::random(),
            "127.0.0.1".into(),
            7001,
        )));

        let node_clone = node1.clone();

        let handle = {
            let node_task = node_clone.clone();
            tokio::spawn(async move {
                let mut node = node_task.lock().await;
                node.start().await;
            })
        };

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        {
            let mut node = node_clone.lock().await;
            node.stop().await;
        }

        let _ = handle.await;
    }
}
