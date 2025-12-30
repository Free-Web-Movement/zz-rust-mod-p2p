use async_trait::async_trait;
use bitflags::bitflags;
use serde::{ Deserialize, Serialize };

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ProtocolCapability: u8 {
        const TCP       = 1 << 0;
        const UDP       = 1 << 1;
        const HTTP      = 1 << 2;
        const WEBSOCKET = 1 << 3;
    }
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
pub trait Listener: Sync + 'static {
    async fn run(&mut self) -> anyhow::Result<()>;
}
