// use crate::handlers::tcp::TCPHandler;
// use crate::nodes::net_info::NetInfo;
// use crate::protocols::registry::CommandHandlerRegistry;
use aex::connection::global::GlobalContext;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;
use zz_account::address::FreeWebMovementAddress as Address;

use aex::storage::Storage;

/* =========================
   NODE
========================= */

#[derive(Clone)]
pub struct Node {
    pub id: Address,  // Unique network address of the node, also id for the node
    pub name: String, // User defined name for the node, no need to be unique
    pub addr: SocketAddr,
    pub storage: Storage,
    pub context: Arc<Mutex<GlobalContext>>, // global context
}

impl Node {
    pub fn new(
        name: String,
        id: Address,
        addr: SocketAddr,
        storage: Storage,
        context: Arc<Mutex<GlobalContext>>,
    ) -> Self {
        Self {
            name,
            id,
            addr,
            context,
            storage,
        }
    }

    // async fn listen<T: Listener + Send + 'static>(&self, object: T) -> Arc<Mutex<T>> {
    //     let handler = Arc::new(Mutex::new(object));
    //     let handler_clone = handler.clone();
    //     tokio::spawn(async move {
    //         let mut h = handler_clone.lock().await;
    //         let _ = h.run().await;
    //     });
    //     handler
    // }

    // Removed incorrect generic overload that used `Arc` as a trait bound (not allowed).
    // Use the parameterless `start` implementation below to start the node's handlers.
    pub async fn start(&mut self) {

        // self.start_time = SystemTime::timestamp();
        // let _ip = self.ip.clone();
        // let port = self.port;
        // CommandHandlerRegistry::init_registry().await;

        // 节点全局共享的内容，所有持久化的信息都保存在context里面

        // self.net_info = Some(NetInfo::collect(port).unwrap());
        // let mut servers = self.init_storage_and_servers(port);
        // servers.connect().await;

        // let context = Arc::new(Context::new(
        //     ip.clone(),
        //     port,
        //     self.address.clone(),
        //     servers.clone(),
        // ));
        // self.context = Some(context.clone());

        // servers
        //     .notify_online(self.address.clone(), &context.clone())
        //     .await
        //     .unwrap();

        // let tcp = TCPHandler::bind(context.clone())
        //     .await
        //     .unwrap()
        //     .as_ref()
        //     .clone();
        // let _udp = UDPHandler::bind(context.clone()).await.unwrap().as_ref().clone();

        // self.tcp_handler = Some(self.listen(tcp).await);
        // self.udp_handler = Some(self.listen(udp).await);
    }

    pub async fn stop(&mut self) {
        // minimal stop implementation: close tcp and udp connections
        // take ownership of the Arcs we hold and drop them so the underlying sockets
        // are closed when there are no remaining owners

        // match self.context.clone() {
        //     Some(context) => {
        //         context.token.cancel();
        //     }
        //     None => (),
        // }
        // self.tcp_handler.take();
        // self.udp_handler.take();

        // self.stop_time = SystemTime::timestamp();
    }

    pub fn init_storage_and_servers(&mut self, _port: u16) {
        // 1️⃣ 初始化 storage
        // if self.storage.is_none() {
        //     self.storage = Some(Storage::new(None));
        // }
        // let storage = self.storage.as_ref().unwrap().clone();

        // 2️⃣ 初始化 Servers（内部完成 external list 的 merge + persist）
        // let servers = Servers::new(
        //     self.address.clone(),
        //     storage.clone(),
        //     self.net_info.as_ref().expect("net_info missing").clone(),
        // );

        // 3️⃣ 保存当前节点 address
        // storage.save("address".to_string(), &self.address).unwrap();
        // servers
    }

    pub async fn notify_online(&self) {
        // TODO: Implement notify_online logic
        println!("Notify online!");
        // let servers = &self.context.clone().unwrap().servers;
        // let servers = servers.lock().await;
        // let servers = servers.clone();
        // println!("Server found!");
        // let address = self.address.clone();
        // let context = self.context.clone().unwrap().clone();
        // tokio::spawn(async move {
        //     println!("Server notifyed to {}!", address.clone());
        //     let _ = servers.notify_online(address, &context).await;
        // });
    }
}
