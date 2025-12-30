use std::time::Duration;

use futures::{ StreamExt, stream };
use tokio::{ net::TcpStream, time::timeout };
use zz_account::address::FreeWebMovementAddress;

use crate::nodes::record::NodeRecord;
use crate::protocols::client_type::{ ClientType, send_offline, send_online, to_client_type };

/// 已连接的服务器（控制面 + 数据面）
///
#[derive(Clone)]
pub struct ConnectedServer {
    pub record: NodeRecord,
    pub client_type: ClientType,
}

/// 已连接服务器集合（区分 inner / external）
#[derive(Clone)]
pub struct ConnectedServers {
    pub inner: Vec<ConnectedServer>,
    pub external: Vec<ConnectedServer>,
}

impl ConnectedServers {
    const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
    const MAX_CONCURRENT: usize = 16;

    /// 🔗 仅使用 TCP 建立初始控制连接
    pub async fn connect(records: Vec<NodeRecord>) -> Vec<ConnectedServer> {
        stream
            ::iter(records)
            .map(|record| async move {
                match timeout(Self::CONNECT_TIMEOUT, TcpStream::connect(record.endpoint)).await {
                    Ok(Ok(stream)) => {
                        let tcp = to_client_type(stream);
                        tracing::info!("tcp connect succeeded {}", record.endpoint);
                        Some(ConnectedServer {
                            record,
                            client_type: tcp
                        })
                    }
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
            .collect().await
    }

    pub async fn new(inner: Vec<NodeRecord>, external: Vec<NodeRecord>) -> Self {
        let (inner, external) = tokio::join!(Self::connect(inner), Self::connect(external));

        Self { inner, external }
    }

    /// 🔔 通知所有服务器上线
    pub async fn notify_online(
        &self,
        address: &FreeWebMovementAddress,
        data: Option<Vec<u8>>,
        is_external: bool
    ) {
        let all = if is_external { self.inner.iter() } else { self.external.iter() };

        let futures = all.map(|server| {
            let addr = address.clone();
            let bytes = data.clone();
            async move {
                let _ = send_online(&server.client_type, &addr, bytes).await;
            }
        });

        futures::future::join_all(futures).await;
    }

    /// 🔔 通知所有服务器下线
    pub async fn notify_offline(
        &self,
        address: &FreeWebMovementAddress,
        data: Option<Vec<u8>>,
        is_external: bool
    ) {
        let all = if is_external { self.inner.iter() } else { self.external.iter() };
        let futures = all.map(|server| {
            let addr = address.clone();
            let payload = data.clone();
            async move {
                let _ = send_offline(&server.client_type, &addr, payload).await;
            }
        });

        futures::future::join_all(futures).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::client_type::send_bytes;
    use crate::protocols::defines::ProtocolCapability;
    use chrono::Utc;
    use std::net::{ IpAddr, Ipv4Addr, SocketAddr };
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;

    fn make_record(port: u16) -> NodeRecord {
        NodeRecord {
            endpoint: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port),
            protocols: ProtocolCapability::TCP,
            first_seen: Utc::now(),
            last_seen: Utc::now(),
            connected: false,
            last_disappeared: None,
            reachability_score: 100,
            address: None,
        }
    }

    /// 🔹 TCP connect 成功
    #[tokio::test]
    async fn test_connect_success() -> anyhow::Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();

        tokio::spawn(async move {
            let _ = listener.accept().await;
        });

        let records = vec![make_record(port)];
        let connected = ConnectedServers::connect(records).await;

        assert_eq!(connected.len(), 1);
        Ok(())
    }

    /// 🔹 TCP connect 失败会被过滤
    #[tokio::test]
    async fn test_connect_failure_filtered() {
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

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 64];
            let n = socket.read(&mut buf).await.unwrap();
            buf[..n].to_vec()
        });

        let connected = ConnectedServers::connect(vec![record]).await;
        assert_eq!(connected.len(), 1);

        let msg = b"hello-connected-server";

        send_bytes(&connected[0].client_type, msg).await;
        // connected[0].command.send(msg).await?;

        let received = server.await.unwrap();
        assert_eq!(received, msg);

        Ok(())
    }
}
