use std::sync::Arc;

// use serde_json::Value;
// use zz_account::address::FreeWebMovementAddress as Address;
use async_trait::async_trait;

trait NatOperations {
    fn punch_hole(&self, target_ip: &str, target_port: u16) -> anyhow::Result<()>;
    fn maintain_nat(&self) -> anyhow::Result<()>;
    fn plug(&self, one: &(String, u16), another: &(String, u16)) -> bool;
    fn unplug(&self, one: &(String, u16), another: &(String, u16)) -> bool;
}
// trait NetService {
//     async fn start(&self);
//     async fn stop(&self);
//     async fn send(&self, data: &[u8]);
//     async fn receive(&self) -> Vec<u8>;
//     async fn process(&self, context: &mut Context);
//     fn status(&self) -> String;
// }

#[allow(non_camel_case_types)]
enum NatProtocol {
    STUN,
    TURN,
    ICE,
    HOLE_PUNCH,
}

// Net Service Type
#[allow(non_camel_case_types)]
enum TransportProtocol {
    UDP,
    TCP,
    HTTP,
    HTTP2,
    QUIC,
    HTTP3,
    WEB_SOCKET,
    // SMTP,
    // FTP,
    // ICMP,
    // SCTP,
    // SSH,
    // DNS,
}

struct NatInfo {
    pub stun_port: u16,
    pub turn_port: u16,
}

// struct Node {
//     pub ip: String,
//     pub port: u16,
//     pub address: Address,
//     pub stun_port: u16,
//     pub turn_port: u16,
//     pub last_seen: u64,
// }

struct NatPair<S, T> {
    pub protocol: NatProtocol,
    server: S, // One Server/Listener Multiple Clients
    clients: Vec<T>,
    plugged_pairs: Vec<(T, T)>,
}

// struct Context {
//     node: Node,
//     global: Value,
//     local: Value,
// }



#[async_trait]
pub trait Listener: Send + Sync + 'static {
    async fn run(&mut self) -> anyhow::Result<()>;
    async fn new(ip: &String, port: u16) -> Arc<Self>;
    async fn stop(self: Arc<Self>) -> anyhow::Result<()> ;
}