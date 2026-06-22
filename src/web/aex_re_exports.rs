//! Re-exports of aex types needed by the root crate, so root never imports `aex` directly.
pub use aex::connection::context::Context;
pub use aex::connection::global::{ConnectionInfo, GlobalContext, PeerInfo};
pub use aex::connection::scope::NetworkScope;
pub use aex::http::meta::HttpMetadata;
pub use aex::http::middlewares::websocket::WsSenderList;
pub use aex::http::protocol::header::HeaderKey;
pub use aex::http::protocol::media_type::SubMediaType;
pub use aex::http::protocol::method::HttpMethod;
pub use aex::tcp::types::Codec;
