use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::AsyncWriteExt;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct ConnectedClients {
    pub inner: HashMap<
        String,
        Vec<(
            (Arc<Mutex<OwnedReadHalf>>, Arc<Mutex<OwnedWriteHalf>>),
            Vec<SocketAddr>,
        )>,
    >,
    pub external: HashMap<
        String,
        Vec<(
            (Arc<Mutex<OwnedReadHalf>>, Arc<Mutex<OwnedWriteHalf>>),
            Vec<SocketAddr>,
        )>,
    >,
}

impl ConnectedClients {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
            external: HashMap::new(),
        }
    }

    pub fn add_inner(&mut self, address: &str, tcp: (Arc<Mutex<OwnedReadHalf>>, Arc<Mutex<OwnedWriteHalf>>), sockets: Vec<SocketAddr>) {
        self.inner.entry(address.to_string()).or_insert_with(Vec::new).push((tcp, sockets));
    }

    pub fn add_external(&mut self, address: &str, tcp: (Arc<Mutex<OwnedReadHalf>>, Arc<Mutex<OwnedWriteHalf>>), sockets: Vec<SocketAddr>) {
        self.external.entry(address.to_string()).or_insert_with(Vec::new).push((tcp, sockets));
    }

    pub async fn close(data: (Arc<Mutex<OwnedReadHalf>>, Arc<Mutex<OwnedWriteHalf>>)) {
        let (reader, writer) = data;
        // 先关闭写端（发送 FIN）
        {
            let mut writer = writer.lock().await;
            let _ = writer.shutdown().await;
        }

        // 再关闭读端（drop）
        {
            let reader = reader.lock().await;
            drop(reader);
        }
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
            Self::close(tcp).await;
        }
    }

    pub async fn close_all(&mut self) {
        for (_, list) in self.inner.drain() {
            for (tcp, _) in list {
                Self::close(tcp).await;
            }
        }

        for (_, list) in self.external.drain() {
            for (tcp, _) in list {
                Self::close(tcp).await;
            }
        }
    }

    /// 根据 address 查找对应的 TCP 连接
    /// 如果同时存在 inner 和 external，可以通过 `include_external` 控制是否包含 external
    pub fn get_connections(
        &self,
        address: &String,
        include_external: bool,
    ) -> Vec<(Arc<Mutex<OwnedReadHalf>>, Arc<Mutex<OwnedWriteHalf>>)> {
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
