pub const DEFAULT_PORT: u16 = 10086;
pub const DEFAULT_ADDRESS_EXTERNALHOST: &str = "0.0.0.0";
pub const DEFAULT_ADDRESS_LOCALHOST: &str = "127.0.0.1";

/// ==============================
///       项目级常量
/// ==============================



pub const HTTP_BUFFER_LENGTH: usize = 8 * 1024;
/// 默认 TCP 读取缓冲区
pub const TCP_BUFFER_LENGTH: usize = 8 * 1024;

/// HTTP 探测用 peek 缓冲区
pub const PEEK_TCP_BUFFER_LENGTH: usize = 1024;

/// 项目名称（例如日志前缀）
pub const PROJECT_NAME: &str = "Free-Web-Movement-P2P-Node";

/// 默认超时时间（毫秒）
pub const DEFAULT_TIMEOUT_MS: u64 = 10_000;
pub const DEFAULT_APP_DIR:&str = ".zz";
pub const DEFAULT_APP_DIR_ADDRESS_JSON_FILE:&str = "address.json";
pub const DEFAULT_APP_DIR_EXTERNAL_SERVER_LIST_JSON_FILE:&str = "external-server-list.json";
pub const DEFAULT_APP_DIR_INNER_SERVER_LIST_JSON_FILE:&str = "inner-server-list.json";