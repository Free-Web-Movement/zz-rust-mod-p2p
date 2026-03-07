use aex::{
    connection::context::Context,
    storage::Storage,
};
use anyhow::Result;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;
use zz_account::address::FreeWebMovementAddress;

use crate::{
    consts::{
        DEFAULT_APP_DIR_EXTERNAL_SERVER_LIST_JSON_FILE, DEFAULT_APP_DIR_INNER_SERVER_LIST_JSON_FILE,
    },
    nodes::record::NodeRegistry,
};

#[derive(Clone)]
pub struct Servers {
    pub address: FreeWebMovementAddress,
    pub inner: NodeRegistry,
    pub external: NodeRegistry,
}

impl Servers {
    pub fn new(address: FreeWebMovementAddress, storage: Storage) -> Self {
        let inner =
            NodeRegistry::load_from_storage(&storage, DEFAULT_APP_DIR_INNER_SERVER_LIST_JSON_FILE);
        let external = NodeRegistry::load_from_storage(
            &storage,
            DEFAULT_APP_DIR_EXTERNAL_SERVER_LIST_JSON_FILE,
        );

        Self {
            inner,
            external,
            address,
        }
    }

    /// 连接到指定节点，并加入 connected_servers，同时持续接收消息
    pub async fn connect_to_node(
        &mut self,
        addr: SocketAddr,
        _ctx: Arc<Mutex<Context>>,
    ) -> Result<()> {
        println!("Connect to node: {}:{}", addr.ip(), addr.port());
        // 尝试 TCP 连接
        // let stream = TcpStream::connect(addr).await?;

        // let tcp = to_client_type(stream);
        // let client_type = Arc::new(tcp.clone());
        // let tcp_clone = tcp.clone();

        // 构造 NodeRecord
        // let record = NodeRecord {
        //     endpoint: addr,
        //     protocols,
        //     first_seen: chrono::Utc::now(),
        //     last_seen: chrono::Utc::now(),
        //     connected: true,
        //     last_disappeared: None,
        //     reachability_score: 100,
        //     address: None,
        // };

        // 构造 ConnectedServer
        // let connected = ConnectedServer {
        //     record,
        //     client_type: tcp,
        // };

        println!("Connected server!");

        // // 判断内网/外网，加入 inner 或 external
        // if self.is_inner_ip(ip) {
        //     if let Some(connected_servers) = &mut self.connected_servers {
        //         connected_servers.inner.push(connected);
        //     }
        // } else {
        //     if let Some(connected_servers) = &mut self.connected_servers {
        //         connected_servers.external.push(connected);
        //     }
        // }

        // let stream: crate::protocols::client_type::ClientType = tcp.clone();

        println!("out side notify_online");

        println!("start loop reading");

        // Clone the Arc<Context> so we can move it into the spawned task without borrowing self.
        // let context_clone = Arc::clone(&context);
        // let context = Arc::clone(&context);
        // let tcp = tcp_clone.clone();
        // tokio::spawn(async move {
        // Move-owned `stream` and `context` are referenced inside the async block,
        // so no non-'static borrow from `self` escapes.
        // loop_reading(&tcp, &context, addr).await;
        // });

        // {
        // let _ = self
        //     .notify_online(self.address.clone(), context.clone())
        //     .await;
        // // }

        Ok(())
    }
}
