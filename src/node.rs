use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};


use tokio::sync::Mutex;
use zz_account::address::FreeWebMovementAddress as Address;

use crate::{
    tcp::{self, TCPHandler},
    udp::UDPHandler,
};

use futures::future::join_all;

use async_trait::async_trait;

#[async_trait]
trait Listener: Send + Sync + 'static {
    async fn start(&mut self) -> anyhow::Result<()>;
    async fn new(ip: &String, port: u16) -> Arc<Self>;
}

#[async_trait]
impl Listener for TCPHandler {
    async fn start(&mut self) -> anyhow::Result<()> {
        self.start().await
    }
    async fn new(ip: &String, port: u16) -> Arc<Self> {
        TCPHandler::bind(ip, port).await.unwrap()
    }
}

#[async_trait]
impl Listener for UDPHandler {
    async fn start(&mut self) -> anyhow::Result<()> {
        self.start().await
    }
    async fn new(ip: &String, port: u16) -> Arc<Self> {
        UDPHandler::bind(ip, port).await.unwrap()
    }
}

/* =========================
   NODE
========================= */

#[derive(Clone)]
pub struct Node {
    pub name: String,     // User defined name for the node, no need to be unique
    pub address: Address, // Unique network address of the node
    pub ip: String,       // Bound IP address of the node
    pub port: u16,        // Bound port of the node
    pub stun_port: u16,   // STUN service port
    pub trun_port: u16,   // TURN service port
    pub start_time: u128, // Timestamp when the node was started
    pub stop_time: u128,  // Timestamp when the node was started

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
            start_time: 0,
            stop_time: 0,
        }
    }

    async fn listen<T: Listener + Send + 'static>(
        &self,
        object: T,
        threads: &mut Arc<Vec<tokio::task::JoinHandle<u8>>>,
    ) -> anyhow::Result<Arc<Mutex<T>>> {
        let handler = Arc::new(Mutex::new(object));
        let handler_clone: Arc<Mutex<T>> = Arc::clone(&handler);
        let threads = Arc::get_mut(threads).unwrap();
        threads.push(tokio::spawn(async move {
            let mut handler = handler_clone.lock().await;
            let _ = handler.start().await;
            0u8
        }));
        Ok(handler)
    }

    // Removed incorrect generic overload that used `Arc` as a trait bound (not allowed).
    // Use the parameterless `start` implementation below to start the node's handlers.
    pub async fn start(&mut self) {
        self.start_time = timestamp();
        let ip: String = self.ip.clone();
        let port: u16 = self.port;
        let mut threads: Arc<Vec<tokio::task::JoinHandle<u8>>> = Arc::new(vec![]);

        let tcp = TCPHandler::bind(&ip, port).await.unwrap().as_ref().clone();
        let udp = UDPHandler::bind(&ip, port).await.unwrap().as_ref().clone();

        self.tcp_handler = Some(
            self.listen(tcp, &mut threads)
                .await
                .unwrap(),
        );
        self.udp_handler = Some(
            self.listen(udp, &mut threads)
                .await
                .unwrap(),
        );
        
        let results: Vec<Result<u8, tokio::task::JoinError>> =
            join_all(Arc::try_unwrap(threads).unwrap()).await;
        for res in results {
            match res {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Error starting handler: {}", e);
                }
            }
        }
    }

    pub async fn stop(&mut self) {
        // minimal stop implementation: close tcp and udp connections
        // take ownership of the Arcs we hold and drop them so the underlying sockets
        // are closed when there are no remaining owners

        // take ownership of our Option<Arc<...>> values
        let Some(tcp_handler) = self.tcp_handler.take() else {
            return;
        };
        let Some(udp_handler) = self.udp_handler.take() else {
            // restore tcp if udp missing
            self.tcp_handler = Some(tcp_handler);
            return;
        };

        // briefly lock both handlers to ensure they are not in use,
        // then drop the locks and the Arc owners so resources can close.
        {
            let _tcp_lock = tcp_handler.lock().await;
            let _udp_lock = udp_handler.lock().await;
            // locks dropped at end of scope
        }

        drop(tcp_handler);
        drop(udp_handler);

        self.stop_time = timestamp();
    }
}

fn timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

/* =========================
   TEST
========================= */

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;
    use tokio::test;

    #[test]
    async fn test_node_init() {
        let address = zz_account::address::FreeWebMovementAddress::random();
        let address_cloned = address.clone();
        let n = Node::new("node1".to_owned(), address, "127.0.0.1".to_owned(), 3000);

        assert_eq!(n.name, "node1");
        assert_eq!(n.address.to_string(), address_cloned.to_string());
        assert_eq!(n.ip, "127.0.0.1".to_owned());
        assert_eq!(n.port, 3000);
        assert!(n.start_time == 0);
        assert!(n.stop_time == 0);
        let start = thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // let _ = n.start().await;
                // assert!(n.start_time > 0);
                // let __ = n.stop().await;
                // assert!(n.stop_time > 0);
            });
        });
        start.join().unwrap();
    }
}
