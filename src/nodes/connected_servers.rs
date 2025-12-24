use std::{sync::Arc, time::Duration};

use futures::{StreamExt, stream};
use tokio::{
    net::{TcpStream, UdpSocket},
    sync::Mutex,
    time::timeout,
};

use tokio::io::AsyncWriteExt;

use crate::{node::Node, nodes::record::NodeRecord};

pub struct ConnectedServer {
    pub record: NodeRecord,
    pub tcp: Arc<Mutex<TcpStream>>,  // 控制通道（必须）
    pub udp: Option<Arc<UdpSocket>>, // 数据通道（可选）
}

impl ConnectedServer {
    /// 🔥 事件发送：UDP 优先，失败回退 TCP
    pub async fn send(&self, data: &[u8]) -> anyhow::Result<()> {
        // 1️⃣ UDP 优先
        if let Some(udp) = &self.udp {
            if let Err(e) = udp.send(data).await {
                tracing::warn!("udp send failed, fallback to tcp: {:?}", e);
            } else {
                return Ok(());
            }
        }

        // 2️⃣ TCP fallback
        let mut stream = self.tcp.lock().await;
        stream.write_all(data).await?;
        Ok(())
    }
}

pub struct ConnectedServers {
    pub inner: Vec<ConnectedServer>,
    pub external: Vec<ConnectedServer>,
}

impl ConnectedServers {
    const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
    const MAX_CONCURRENT: usize = 16;

    pub async fn connect(records: Vec<NodeRecord>) -> Vec<ConnectedServer> {
        stream::iter(records)
            .map(|record| async move {
                match timeout(Self::CONNECT_TIMEOUT, TcpStream::connect(record.endpoint)).await {
                    Ok(Ok(stream)) => Some(ConnectedServer {
                        record,
                        tcp: Arc::new(Mutex::new(stream)),
                        udp: None,
                    }),
                    Ok(Err(e)) => {
                        tracing::warn!("tcp connect failed {}: {:?}", record.endpoint, e);
                        None
                    }
                    Err(_) => {
                        tracing::warn!("tcp connect timeout {}", record.endpoint);
                        None
                    }
                }
            })
            .buffer_unordered(Self::MAX_CONCURRENT)
            .filter_map(|x| async { x })
            .collect()
            .await
    }

    pub async fn new(inner_records: Vec<NodeRecord>, external_records: Vec<NodeRecord>) -> Self {
        let (inner, external) = tokio::join!(
            Self::connect(inner_records),
            Self::connect(external_records)
        );

        Self { inner, external }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::defines::ProtocolCapability;
    use chrono::Utc;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use tokio::net::TcpListener;

    fn make_record(port: u16) -> NodeRecord {
        NodeRecord {
            endpoint: SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                port,
            ),
            protocols: ProtocolCapability::TCP,
            first_seen: Utc::now(),
            last_seen: Utc::now(),
            connected: false,
            last_disappeared: None,
            reachability_score: 100,
        }
    }

    /// 🔹 connect 成功
    #[tokio::test]
    async fn test_connect_success() -> anyhow::Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();

        // 后台 accept（不处理数据，只维持连接）
        tokio::spawn(async move {
            let _ = listener.accept().await;
        });

        let records = vec![make_record(port)];
        let connected = ConnectedServers::connect(records).await;

        assert_eq!(connected.len(), 1);
        Ok(())
    }

    /// 🔹 connect 失败被过滤
    #[tokio::test]
    async fn test_connect_failure_filtered() {
        // 随机高概率未监听端口
        let records = vec![make_record(59999)];

        let connected = ConnectedServers::connect(records).await;
        assert!(connected.is_empty());
    }

    /// 🔹 send：UDP 不存在 → TCP fallback
    #[tokio::test]
    async fn test_send_tcp_fallback() -> anyhow::Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();

        let record = make_record(port);

        // 接收端
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 32];
            let n = socket.readable().await.unwrap();
            let n = socket.try_read(&mut buf).unwrap();
            buf[..n].to_vec()
        });

        let mut connected =
            ConnectedServers::connect(vec![record]).await;
        assert_eq!(connected.len(), 1);

        let msg = b"hello-connected-server";
        connected[0].send(msg).await?;

        let received = server.await.unwrap();
        assert_eq!(received, msg);

        Ok(())
    }
}

