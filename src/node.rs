use std::{
    sync::Arc,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::sync::Mutex;
use zz_account::address::FreeWebMovementAddress as Address;

use crate::{tcp::TCPHandler, udp::UDPHandler};

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
    
    pub async fn start(&mut self) {
        self.start_time = timestamp();
        let ip: String = self.ip.clone();
        let port: u16 = self.port;
        self.tcp_handler = Some(Arc::new(Mutex::new(TCPHandler::new(&ip, port))));
        self.udp_handler = Some(Arc::new(Mutex::new(UDPHandler::new(&ip, port))));

        let Some(tcp_handler) = self.tcp_handler.as_ref() else {
            return;
        };
        let Some(udp_handler) = self.udp_handler.as_ref() else {
            return;
        };
        let tcp_handler_clone = Arc::clone(&tcp_handler);
        let udp_handler_clone = Arc::clone(&udp_handler);

        let handler = thread::spawn(async move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let mut tcp_handler = tcp_handler_clone.lock().await;
                let mut udp_handler = udp_handler_clone.lock().await;
                let _ = tcp_handler.start().await;
                let _ = udp_handler.start().await;
            });
        });
        handler.join().unwrap().await;
    }

    pub async fn stop(&mut self) {
        // minimal stop implementation: close tcp and udp connections
        // take ownership of the Arcs we hold and drop them so the underlying sockets
        // are closed when there are no remaining owners

        let Some(tcp_handler) = self.tcp_handler.as_ref() else {
            return;
        };
        let Some(udp_handler) = self.udp_handler.as_ref() else {
            return;
        };

        let tcp_handler_clone = Arc::clone(&tcp_handler);
        let udp_handler_clone = Arc::clone(&udp_handler);

        let handler = thread::spawn(async move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let tcp_handler = tcp_handler_clone.lock().await;
                let udp_handler = udp_handler_clone.lock().await;
                drop(tcp_handler);
                drop(udp_handler);
            });
        });
        handler.join().unwrap().await;

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
        let mut n = Node::new("node1".to_owned(), address, "127.0.0.1".to_owned(), 3000);

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
