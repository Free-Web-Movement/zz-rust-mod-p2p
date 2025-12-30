use std::sync::Arc;
use base64::Engine;

use crate::{context::Context, protocols::client_type::{ClientType, send_bytes}};

/// ==============================
///       WebSocket 常量
/// ==============================

/// RFC 6455 标准固定 Magic GUID（不可随机）
pub const WEBSOCKET_MAGIC_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// WebSocket 标准 Upgrade 值
pub const WEBSOCKET_UPGRADE_VALUE: &str = "websocket";

/// WebSocket Connection: Upgrade
pub const CONNECTION_UPGRADE_VALUE: &str = "Upgrade";


/// WebSocket Upgrade 所需要的固定 Header
pub const HEADER_UPGRADE: &str = "Upgrade";
pub const HEADER_CONNECTION: &str = "Connection";
pub const HEADER_SEC_WEBSOCKET_KEY: &str = "Sec-WebSocket-Key";
pub const HEADER_SEC_WEBSOCKET_ACCEPT: &str = "Sec-WebSocket-Accept";


pub struct WebSocketHandler {
    stream: Arc<ClientType>,
    context: Arc<Context>
}

impl WebSocketHandler {
    pub fn new(stream: Arc<ClientType>, context: Arc<Context>) -> Self {
        Self {stream, context }
    }
    /// 判断是否为 WebSocket Upgrade
    pub fn is_websocket_request(data: &[u8]) -> bool {
        let s = String::from_utf8_lossy(data);
        s.contains(HEADER_UPGRADE) && s.to_lowercase().contains(WEBSOCKET_UPGRADE_VALUE)
    }

    /// 返回 WebSocket 握手响应
    pub async fn respond_websocket_handshake(
        self: Arc<Self>,
        // stream: &mut TcpStream,
        req: &[u8]
    ) -> anyhow::Result<()> {
        use sha1::{ Digest, Sha1 };
        use base64::engine::general_purpose::STANDARD as BASE64;

        let req_str = String::from_utf8_lossy(req);

        // 提取 Sec-WebSocket-Key
        let key_line = req_str
            .lines()
            .find(|line| line.starts_with(HEADER_SEC_WEBSOCKET_KEY))
            .ok_or_else(|| anyhow::anyhow!("Missing Sec-WebSocket-Key"))?;

        let key = key_line.split(':').nth(1).unwrap().trim();

        // SHA1(key + magic)
        let mut hasher = Sha1::new();
        hasher.update(key.as_bytes());
        hasher.update(WEBSOCKET_MAGIC_GUID.as_bytes());

        let accept_key = BASE64.encode(hasher.finalize());

        let response = format!(
            "HTTP/1.1 101 Switching Protocols\r\n\
             Connection: {CONNECTION_UPGRADE_VALUE}\r\n\
             Upgrade: {WEBSOCKET_UPGRADE_VALUE}\r\n\
             Sec-WebSocket-Accept: {accept_key}\r\n\
             \r\n"
        );

        let stream = self.stream.clone();

        send_bytes(&stream, response.as_bytes()).await;
        Ok(())
    }
}
