use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::{
    io::AsyncWriteExt,
    net::{TcpStream, UdpSocket},
};
use zz_account::address::FreeWebMovementAddress as Address;

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
    pub tcp_stream: Option<Arc<TcpStream>>,
    pub udp_socket: Option<Arc<UdpSocket>>,
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
            tcp_stream: None,
            udp_socket: None,
            start_time: 0,
            stop_time: 0,
        }
    }
    pub async fn start(&mut self) -> anyhow::Result<Self> {
        // minimal start implementation: return a cloned instance
        self.start_time = timestamp();
        self.tcp_stream = Some(Arc::new(
            TcpStream::connect(format!("{}:{}", self.ip, self.port)).await?,
        ));
        self.udp_socket = Some(Arc::new(
            UdpSocket::bind(format!("{}:{}", self.ip, self.port)).await?,
        ));
        Ok(self.clone())
    }

    pub async fn stop(&mut self) -> anyhow::Result<()> {
        // minimal stop implementation: close tcp and udp connections
        // take ownership of the Arcs we hold and drop them so the underlying sockets
        // are closed when there are no remaining owners
        if let Some(stream) = self.tcp_stream.take() {
            drop(stream);
        }
        if let Some(udp) = self.udp_socket.take() {
            drop(udp);
        }
        self.stop_time = timestamp();
        Ok(())
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
                let _ = n.start().await;
                assert!(n.start_time > 0);
                let __ = n.stop().await.unwrap();
                assert!(n.stop_time > 0);
            });
        });
        start.join().unwrap();
    }
}
