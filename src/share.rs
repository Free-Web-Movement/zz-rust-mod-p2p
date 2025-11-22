pub const DEFAULT_PORT: u16 = 10086;
pub const DEFAULT_ADDRESS_EXTERNALHOST: &str = "0.0.0.0";
pub const DEFAULT_ADDRESS_LOCALHOST: &str = "127.0.0.1";
pub const PEEK_STREAM_BUFFER_LENGTH: usize = 1024;
// pub const HTTP_BUFFER_LENGTH: usize = 4096;

// pub const WEBSOCKET_REQUEST: &str = "Upgrade: websocket";

/// ==============================
///        HTTP 常量
/// ==============================

/// 默认 HTTP 读取缓冲区大小
pub const HTTP_BUFFER_LENGTH: usize = 8 * 1024; // 8 KB

/// WebSocket Upgrade 所需要的固定 Header
pub const HEADER_UPGRADE: &str = "Upgrade";
pub const HEADER_CONNECTION: &str = "Connection";
pub const HEADER_SEC_WEBSOCKET_KEY: &str = "Sec-WebSocket-Key";
pub const HEADER_SEC_WEBSOCKET_ACCEPT: &str = "Sec-WebSocket-Accept";

/// ==============================
///       WebSocket 常量
/// ==============================

/// RFC 6455 标准固定 Magic GUID（不可随机）
pub const WEBSOCKET_MAGIC_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// WebSocket 标准 Upgrade 值
pub const WEBSOCKET_UPGRADE_VALUE: &str = "websocket";

/// WebSocket Connection: Upgrade
pub const CONNECTION_UPGRADE_VALUE: &str = "Upgrade";

/// ==============================
///       网络/系统常量
/// ==============================

/// 默认最大 UDP 数据包大小
pub const UDP_MAX_PACKET_SIZE: usize = 2048;

/// 默认 TCP 读取缓冲区
pub const TCP_BUFFER_LENGTH: usize = 8 * 1024;

/// ==============================
///       项目级常量
/// ==============================

/// 项目名称（例如日志前缀）
pub const PROJECT_NAME: &str = "Free-Web-Movement-P2P-Node";

/// 默认超时时间（毫秒）
pub const DEFAULT_TIMEOUT_MS: u64 = 10_000;
