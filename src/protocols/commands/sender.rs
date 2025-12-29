use crate::protocols::client_type::{ ClientType };
/* =========================
   CommandSender
========================= */

#[derive(Clone)]
pub struct CommandSender {
    /// 控制通道（必须存在，TCP / HTTP / WS 之一）
    pub tcp: ClientType,

    /// 数据通道（可选，UDP 优先）
    pub udp: Option<ClientType>,
}
