use std::sync::Arc;
use bincode::config;
use futures::future::join_all;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use zz_account::address::FreeWebMovementAddress as Address;

use crate::protocols::command::{ Action, Entity };
use crate::protocols::commands::message::MessageCommand;
use crate::protocols::defines::Listener;
use crate::protocols::frame::Frame;
use crate::{ context::Context, nodes::servers::Servers };
use crate::{ handlers::{ tcp::TCPHandler, udp::UDPHandler }, nodes::storage::Storeage };
use crate::{ nodes::net_info::NetInfo, util::time::timestamp };

/* =========================
   NODE
========================= */

#[derive(Clone)]
pub struct Node {
    pub net_info: Option<NetInfo>,
    pub storage: Option<Storeage>,
    pub servers: Option<Servers>,
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
            servers: None,
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

        let tcp = TCPHandler::bind(context.clone()).await.unwrap().as_ref().clone();
        let udp = UDPHandler::bind(context.clone()).await.unwrap().as_ref().clone();

        self.tcp_handler = Some(self.listen(tcp).await);
        self.udp_handler = Some(self.listen(udp).await);
        self.net_info = Some(NetInfo::collect(port).unwrap());
        let _ = self.init_storage_and_server_list(port);
        if let Some(servers) = &mut self.servers {
            servers.connect().await;
            servers.notify_online(self.address.clone()).await.unwrap();
        }
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

    pub fn init_storage_and_server_list(&mut self, _port: u16) -> anyhow::Result<()> {
        // 1️⃣ 初始化 storage
        if self.storage.is_none() {
            self.storage = Some(Storeage::new(None, None, None, None));
        }
        let storage = self.storage.as_ref().unwrap().clone();

        // 2️⃣ 初始化 Servers（内部完成 external list 的 merge + persist）
        let servers = Servers::new(
            storage.clone(),
            self.net_info.as_ref().expect("net_info missing").clone()
        );

        // 3️⃣ 保存当前节点 address
        storage.save_address(&self.address)?;

        // 4️⃣ Node 持有 external server list 视图
        self.servers = Some(servers);

        // self.servers

        Ok(())
    }

    pub async fn send_text_message(&self, receiver: String, message: &str) -> anyhow::Result<()> {
        // 构造消息
        let command = MessageCommand::new(
            receiver.clone(),
            timestamp() as u64,
            message.to_string()
        );

        // 编码成 payload
        let payload = bincode::encode_to_vec(command, config::standard())?;
        let frame = Frame::build_node_command(
            &self.address,
            Entity::Message,
            Action::SendText,
            1,
            Some(payload.clone())
        )?;

        let bytes = Frame::to(frame);

        println!("Node is sending text message to {}: {}", receiver, message);

        // 1️⃣ 尝试本地发送
        if let Some(context) = &self.context {
            let clients = context.clients.lock().await;
            let local_conns = clients.get_connections(&receiver, true);

            println!("Found {} local connections for {}", local_conns.len(), receiver);

            if !local_conns.is_empty() {
                let futures = local_conns.into_iter().map(|tcp_arc| {
                    let bytes = bytes.clone();
                    let receiver = receiver.clone();
                    async move {
                        let mut guard = tcp_arc.lock().await;
                        if let Some(stream) = &mut *guard {
                            println!("Locally sending {} bytes to {}", bytes.len(), receiver.clone());
                            let _ = stream.write_all(&bytes).await;
                        }
                    }
                });

                join_all(futures).await;
                return Ok(()); // 本地发送完成后直接返回
            }
        }

        // 2️⃣ 本地没有 -> 向所有已连接服务器发送
        if let Some(servers) = &self.servers {
            if let Some(connected_servers) = &servers.connected_servers {
                // 使用 iter().chain() 合并两个列表
                let all_servers = connected_servers.inner
                    .iter()
                    .chain(connected_servers.external.iter());

                let futures = all_servers.map(|server| {
                    let bytes = bytes.clone();
                    async move {
                        let _ = server.command.send_text_message(&self.address, bytes).await;
                    }
                });

                join_all(futures).await;
            }
        }

        Ok(())
    }

    pub fn notify_online(&self) {
        // TODO: Implement notify_online logic
        if let Some(servers) = &self.servers {
            let servers = servers.clone();
            let address = self.address.clone();
            tokio::spawn(async move {
                let _ = servers.notify_online(address).await;
            });
        }
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

    #[tokio::test]
    async fn test_two_nodes_notify_online_and_stop() {
        use zz_account::address::FreeWebMovementAddress as Address;

        // create two nodes on different ports
        let node_a = Arc::new(
            Mutex::new(
                Node::new("node-a".into(), Address::random(), "127.0.0.1".into(), 7101, None)
            )
        );

        let node_b = Arc::new(
            Mutex::new(
                Node::new("node-b".into(), Address::random(), "127.0.0.1".into(), 7102, None)
            )
        );

        // start both nodes concurrently
        let a_task = {
            let n = node_a.clone();
            tokio::spawn(async move {
                let mut node = n.lock().await;
                node.start().await;
            })
        };

        let b_task = {
            let n = node_b.clone();
            tokio::spawn(async move {
                let mut node = n.lock().await;
                node.start().await;
            })
        };

        // give time for handlers/servers to initialize
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        // trigger notify_online on each node's Servers (no-op if connected_servers is None)
        {
            let mut n = node_a.lock().await;
            let address = n.address.clone();
            if let Some(servers) = &n.servers {
                let _ = servers.notify_online(address).await;
            }
        }

        {
            let mut n = node_b.lock().await;
            let address = n.address.clone();
            if let Some(servers) = &n.servers {
                let _ = servers.notify_online(address).await;
            }
        }

        // stop both nodes
        {
            let mut n = node_a.lock().await;
            n.stop().await;
        }
        {
            let mut n = node_b.lock().await;
            n.stop().await;
        }

        let _ = a_task.await;
        let _ = b_task.await;
    }
}
