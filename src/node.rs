use std::{
    sync::Arc, time::{SystemTime, UNIX_EPOCH}
};

use tokio::sync::Mutex;
use zz_account::address::FreeWebMovementAddress as Address;

use crate::{tcp::TCPHandler, udp::UDPHandler};
use tokio_util::sync::CancellationToken;

use crate::defines::{Listener};

use crate::context::Context;

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

    async fn listen<T: Listener + Send + 'static>(
        &self,
        object: T,
        token: CancellationToken,
    ) -> Arc<Mutex<T>> {
        let handler = Arc::new(Mutex::new(object));
        let handler_clone = handler.clone();
        tokio::spawn(async move {
            let mut h = handler_clone.lock().await;
            let _ = h.run(token).await;
        });
        handler
    }

    // Removed incorrect generic overload that used `Arc` as a trait bound (not allowed).
    // Use the parameterless `start` implementation below to start the node's handlers.
    pub async fn start(&mut self, token: CancellationToken) {
        self.start_time = timestamp();
        self.start_time = timestamp();
        let ip = self.ip.clone();
        let port = self.port;

        let context = Arc::new(Context::new(ip.clone(), port, self.address.clone()));
        self.context = Some(context.clone());

        let tcp = TCPHandler::bind(&ip, port, context.clone()).await.unwrap().as_ref().clone();
        let udp = UDPHandler::bind(&ip, port, context.clone()).await.unwrap().as_ref().clone();

        self.tcp_handler = Some(self.listen(tcp, token.clone()).await);
        self.udp_handler = Some(self.listen(udp, token.clone()).await);
    }

    pub async fn stop(&mut self, token: CancellationToken) {
        // minimal stop implementation: close tcp and udp connections
        // take ownership of the Arcs we hold and drop them so the underlying sockets
        // are closed when there are no remaining owners

        token.cancel(); // 🔥 核心
        self.tcp_handler.take();
        self.udp_handler.take();

        self.stop_time = timestamp();
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
    #[tokio::test]
    async fn test_node_start_and_stop() {
        let node1 = Arc::new(Mutex::new(Node::new(
            "node".into(),
            Address::random(),
            "127.0.0.1".into(),
            7001,
        )));

        let node_clone = node1.clone();
        let token = CancellationToken::new();
        let cloned = token.clone();

        let handle = {
            let node_task = node_clone.clone();
            tokio::spawn(async move {
                let mut node = node_task.lock().await;
                node.start(token).await;
            })
        };

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        {
            let mut node = node_clone.lock().await;
            node.stop(cloned).await;
        }

        let _ = handle.await;
    }
}
