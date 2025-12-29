use std::{ collections::HashMap };
use std::net::SocketAddr;

use crate::protocols::client_type::{ ClientType, close_client_type };

#[derive(Clone)]
pub struct ConnectedClients {
    pub inner: HashMap<String, Vec<(ClientType, Vec<SocketAddr>)>>,
    pub external: HashMap<String, Vec<(ClientType, Vec<SocketAddr>)>>,
}

impl ConnectedClients {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
            external: HashMap::new(),
        }
    }

    pub fn add_inner(&mut self, address: &str, tcp: ClientType, sockets: Vec<SocketAddr>) {
        self.inner.entry(address.to_string()).or_insert_with(Vec::new).push((tcp, sockets));
    }

    pub fn add_external(&mut self, address: &str, tcp: ClientType, sockets: Vec<SocketAddr>) {
        self.external.entry(address.to_string()).or_insert_with(Vec::new).push((tcp, sockets));
    }

    pub async fn remove_client(&mut self, address: &str) {
        // 取出 inner + external 的所有连接
        let mut streams = Vec::new();

        if let Some(v) = self.inner.remove(address) {
            for (tcp, _) in v {
                streams.push(tcp);
            }
        }

        if let Some(v) = self.external.remove(address) {
            for (tcp, _) in v {
                streams.push(tcp);
            }
        }

        // 统一优雅关闭
        for tcp in streams {
            close_client_type(&tcp).await;
        }
    }

    pub async fn close_all(&mut self) {
        for (_, list) in self.inner.drain() {
            for (tcp, _) in list {
                close_client_type(&tcp).await;
            }
        }

        for (_, list) in self.external.drain() {
            for (tcp, _) in list {
                close_client_type(&tcp).await;
            }
        }
    }

    /// 根据 address 查找对应的 TCP 连接
    /// 如果同时存在 inner 和 external，可以通过 `include_external` 控制是否包含 external
    pub fn get_connections(&self, address: &String, include_external: bool) -> Vec<ClientType> {
        let mut result = Vec::new();

        if let Some(list) = self.inner.get(address) {
            for (tcp, _) in list {
                result.push(tcp.clone());
            }
        }

        if include_external {
            if let Some(list) = self.external.get(address) {
                for (tcp, _) in list {
                    result.push(tcp.clone());
                }
            }
        }

        println!("Found connection(s): {}", result.len());
        result
    }
}
