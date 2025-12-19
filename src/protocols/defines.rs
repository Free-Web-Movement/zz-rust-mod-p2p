use std::sync::Arc;

use std::net::SocketAddr;
use serde::{Deserialize, Serialize};
use tokio::net::{TcpStream, UdpSocket};

// use serde_json::Value;
// use zz_account::address::FreeWebMovementAddress as Address;
use async_trait::async_trait;
use tokio::sync::Mutex;
use bitflags::bitflags;

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

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ProtocolCapability: u8 {
        const TCP       = 1 << 0;
        const UDP       = 1 << 1;
        const HTTP      = 1 << 2;
        const WEBSOCKET = 1 << 3;
    }
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
    UDP {
        socket: Arc<UdpSocket>,
        peer: SocketAddr,
    },
    TCP(Arc<Mutex<TcpStream>>),
    HTTP(Arc<Mutex<TcpStream>>),
    WS(Arc<Mutex<TcpStream>>),
}

#[derive(Debug, Clone)]
pub enum ProtocolCommand {
    HandshakeInit {
        address: String, // FreeWebMovementAddress
    },
    HandshakeAck {
        session_id: [u8; 32],
    },
    Ping,
    Pong,
    Close,
    UpgradeToWebSocket,
    Custom(u16, Vec<u8>),
}

#[async_trait]
pub trait Listener: Send + Sync + 'static {
    async fn run(&mut self) -> anyhow::Result<()>;
    async fn new(context: Arc<Context>) -> Arc<Self>;
    async fn stop(self: &Arc<Self>) -> anyhow::Result<()>;
    /// 原始 / 解密后的数据
    async fn on_data(
        self: &Arc<Self>,
        socket: &ProtocolType,
        received: &[u8],
    ) -> anyhow::Result<()>;

    async fn send(self: &Arc<Self>, protocol_type: &ProtocolType, data: &[u8]) -> anyhow::Result<()>;

    // 协议级指令（握手 / 心跳 / 路由 / 升级）
    // async fn on_cmd(
    //     self: Arc<Self>,
    //     socket: &ProtocolType,
    //     cmd: ProtocolCommand,
    //     remote_peer: &std::net::SocketAddr,
    // ) -> anyhow::Result<()>;
}
