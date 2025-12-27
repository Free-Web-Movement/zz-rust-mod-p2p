use std::sync::Arc;

use async_trait::async_trait;
use bitflags::bitflags;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::Mutex;

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

struct NatPair<S, T> {
    pub protocol: NatProtocol,
    server: S, // One Server/Listener Multiple Clients
    clients: Vec<T>,
    plugged_pairs: Vec<(T, T)>,
}

#[derive(Debug, Clone)]
pub enum ClientType {
    UDP {
        socket: Arc<UdpSocket>,
        peer: SocketAddr,
    },
    TCP(Arc<Mutex<Option<TcpStream>>>),
    HTTP(Arc<Mutex<Option<TcpStream>>>),
    WS(Arc<Mutex<Option<TcpStream>>>),
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

    /// 传输层数据
    /// 
    /// 传输层数据接收
    /// 
    /// on_data 方法用于处理接收到的数据包。
    /// 它接收两个参数：clientType 表示使用的传输协议类型（如 TCP、UDP 等），
    /// received 是一个字节切片，包含接收到的数据内容。
    /// 该方法返回一个异步结果，表示数据处理是否成功完成。
    async fn on_data(
        self: &Arc<Self>,
        client: &ClientType,
        received: &[u8],
    ) -> anyhow::Result<()>;

    // 传输层数据发送
    /// 
    /// send 方法用于发送数据包。
    /// 它接收两个参数：clientType 表示使用的传输协议类型（如 TCP、UDP 等），
    /// data 是一个字节切片，包含要发送的数据内容。
    /// 该方法返回一个异步结果，表示数据发送是否成功完成。
    async fn send(
        self: &Arc<Self>,
        client: &ClientType,
        data: &[u8],
    ) -> anyhow::Result<()>;

    // 协议级指令（握手 / 心跳 / 路由 / 升级）
    // async fn on_cmd(
    //     self: Arc<Self>,
    //     socket: &ClientType,
    //     cmd: ProtocolCommand,
    //     remote_peer: &std::net::SocketAddr,
    // ) -> anyhow::Result<()>;
}
