use tokio::{ io::AsyncWriteExt, net::TcpStream };
use base64::Engine;
use crate::share::{
    HEADER_SEC_WEBSOCKET_KEY,
    HEADER_UPGRADE,
    WEBSOCKET_MAGIC_GUID,
    WEBSOCKET_UPGRADE_VALUE,
    CONNECTION_UPGRADE_VALUE,
};
pub struct WebSocketHandler {
    ip: String,
    port: u16,
    stream: TcpStream,
}

impl WebSocketHandler {
    pub fn new(ip: &String, port: u16, stream: TcpStream) -> Self {
        WebSocketHandler { ip: ip.clone(), port, stream }
    }
    /// 判断是否为 WebSocket Upgrade
    pub fn is_websocket_request(data: &[u8]) -> bool {
        let s = String::from_utf8_lossy(data);
        s.contains(HEADER_UPGRADE) && s.to_lowercase().contains(WEBSOCKET_UPGRADE_VALUE)
    }

    /// 返回 WebSocket 握手响应
    pub async fn respond_websocket_handshake(
        stream: &mut TcpStream,
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

        stream.write_all(response.as_bytes()).await?;
        Ok(())
    }
}
