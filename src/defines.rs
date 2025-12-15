use std::{ net::{TcpStream, UdpSocket}, ops::Add, sync::Arc };

// use serde_json::Value;
// use zz_account::address::FreeWebMovementAddress as Address;
use async_trait::async_trait;
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use zz_account::address::FreeWebMovementAddress as Address;
use std::collections::HashMap;

use crate::context::Context;

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

pub enum ProtocolType {
    UDP(UdpSocket),
    TCP(TcpStream),
    HTTP(TcpStream),
}

#[async_trait]
pub trait Listener: Send + Sync + 'static {
    async fn run(&mut self, token: CancellationToken) -> anyhow::Result<()>;
    async fn new(ip: &String, port: u16, context: Arc<Context>) -> Arc<Self>;
    async fn stop(self: Arc<Self>, token: CancellationToken) -> anyhow::Result<()>;
}
