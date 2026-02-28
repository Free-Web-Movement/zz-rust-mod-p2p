use std::sync::Arc;
use aex::time::SystemTime;
use tokio::sync::Mutex;
use zz_account::address::FreeWebMovementAddress as Address;
use crate::protocols::defines::Listener;
use crate::protocols::registry::CommandHandlerRegistry;
use crate::{ context::Context, nodes::servers::Servers };
use crate::{ handlers::{ tcp::TCPHandler, udp::UDPHandler }, nodes::storage::Storeage };
use crate::{ nodes::net_info::NetInfo };

/* =========================
   NODE
========================= */

#[derive(Clone)]
pub struct Node {
    pub net_info: Option<NetInfo>,
    pub storage: Option<Storeage>,
    // pub servers: Option<Servers>,
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
    pub fn new(
        name: String,
        address: Address,
        ip: String,
        port: u16,
        storage: Option<Storeage>
    ) -> Self {
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
            storage: storage,
            // servers: None,
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
        self.start_time = SystemTime::timestamp();
        let ip = self.ip.clone();
        let port = self.port;
        CommandHandlerRegistry::init_registry().await;

        // 节点全局共享的内容，所有持久化的信息都保存在context里面

        self.net_info = Some(NetInfo::collect(port).unwrap());
        let mut servers = self.init_storage_and_servers(port);
        servers.connect().await;

        let context = Arc::new(
            Context::new(ip.clone(), port, self.address.clone(), servers.clone())
        );
        self.context = Some(context.clone());

        servers.notify_online(self.address.clone(), &context.clone()).await.unwrap();

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
        // self.udp_handler.take();

        self.stop_time = SystemTime::timestamp();
    }

    pub fn init_storage_and_servers(&mut self, _port: u16) -> Servers {
        // 1️⃣ 初始化 storage
        if self.storage.is_none() {
            self.storage = Some(Storeage::new(None, None, None, None));
        }
        let storage = self.storage.as_ref().unwrap().clone();

        // 2️⃣ 初始化 Servers（内部完成 external list 的 merge + persist）
        let servers = Servers::new(
            self.address.clone(),
            storage.clone(),
            self.net_info.as_ref().expect("net_info missing").clone()
        );

        // 3️⃣ 保存当前节点 address
        storage.save_address(&self.address).unwrap();
        servers
    }

    pub async fn notify_online(&self) {
        // TODO: Implement notify_online logic
        println!("Notify online!");
        let servers = &self.context.clone().unwrap().servers;
        let servers = servers.lock().await;
        let servers = servers.clone();
        println!("Server found!");
        let address = self.address.clone();
        let context = self.context.clone().unwrap().clone();
        tokio::spawn(async move {
            println!("Server notifyed to {}!", address.clone());
            let _ = servers.notify_online(address, &context).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zz_account::address::FreeWebMovementAddress as Address;
    #[tokio::test]
    async fn test_node_start_and_stop() {
        let node1 = Arc::new(
            Mutex::new(Node::new("node".into(), Address::random(), "127.0.0.1".into(), 7001, None))
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
}
