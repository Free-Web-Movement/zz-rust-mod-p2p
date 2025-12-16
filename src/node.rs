use std::{ path::PathBuf, sync::Arc, time::{ SystemTime, UNIX_EPOCH } };

use tokio::sync::Mutex;
use zz_account::address::FreeWebMovementAddress as Address;

use crate::{
    consts::{ DEFAULT_APP_DIR, DEFAULT_APP_DIR_ADDRESS_JSON_FILE },
    tcp::TCPHandler,
    udp::UDPHandler,
};
use crate::defines::Listener;

use crate::context::Context;

/* =========================
   NODE
========================= */

#[derive(Clone)]
pub struct Node {
    pub name: String, // User defined name for the node, no need to be unique
    pub address: Address, // Unique network address of the node
    pub ip: String, // Bound IP address of the node
    pub port: u16, // Bound port of the node
    pub stun_port: u16, // STUN service port
    pub trun_port: u16, // TURN service port
    pub start_time: u128, // Timestamp when the node was started
    pub stop_time: u128, // Timestamp when the node was started
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
        }
    }

    async fn get_node_dir() -> String {
        let config_path = dirs_next
            ::config_dir()
            .map(|dir| dir.join(DEFAULT_APP_DIR)) // Creates "MyAppName" folder
            .unwrap_or_else(|| PathBuf::from(DEFAULT_APP_DIR)); // Fallback
        std::fs::create_dir_all(&config_path).expect("Failed to create config dir");

        config_path.display().to_string()
    }

    pub async fn get_node_address_file() -> String {
        let mut dir = Node::get_node_dir().await;
        dir.push_str(DEFAULT_APP_DIR_ADDRESS_JSON_FILE);
        dir
    }

    pub async fn read_address() -> Address {
        let file = Node::get_node_address_file().await;
        Address::load_from_file(&file).unwrap()
    }

    pub async fn save_address(address: Address) {
        let file = Node::get_node_address_file().await;
        println!("saving file path: {}", file);
        let _ = Address::save_to_file(&address, &file);
    }

    async fn listen<T: Listener + Send + 'static>(
        &self,
        object: T
    ) -> Arc<Mutex<T>> {
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
        self.start_time = timestamp();
        let ip = self.ip.clone();
        let port = self.port;

        // 节点全局共享的内容，所有持久化的信息都保存在context里面
        
        let context = Arc::new(Context::new(ip.clone(), port, self.address.clone()));
        self.context = Some(context.clone());

        let tcp = TCPHandler::bind(context.clone()).await.unwrap().as_ref().clone();
        let udp = UDPHandler::bind(context.clone()).await.unwrap().as_ref().clone();

        self.tcp_handler = Some(self.listen(tcp).await);
        self.udp_handler = Some(self.listen(udp).await);
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
}

fn timestamp() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use zz_account::address::FreeWebMovementAddress as Address;
    use tokio::fs;

    #[tokio::test]
    async fn test_node_start_and_stop() {
        let node1 = Arc::new(
            Mutex::new(Node::new("node".into(), Address::random(), "127.0.0.1".into(), 7001))
        );

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

    #[tokio::test]
    async fn test_node_address_file_io() -> anyhow::Result<()> {
        // Step 1: 获取节点目录
        let dir = Node::get_node_dir().await;
        println!("Node directory: {}", dir);

        // Step 2: 获取地址文件路径
        let file = Node::get_node_address_file().await;
        println!("Address file path: {}", file);

        // Step 3: 创建随机地址
        let addr = Address::random();

        // Step 4: 保存地址
        Node::save_address(addr.clone()).await;

        // Step 5: 文件确实存在
        assert!(fs::metadata(&file).await.is_ok());

        // // Step 6: 读取地址
        let loaded = Node::read_address().await;

        // // Step 7: 验证保存和读取一致
        assert_eq!(addr.to_string(), loaded.to_string());

        Ok(())
    }
}
