use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::context::Context;
use crate::handlers::http::HTTPHandler;
use crate::protocols::commands::parser::CommandParser;
use crate::protocols::defines::{ClientType, Listener};
use crate::protocols::frame::Frame;

/// 默认 TCP 读取缓冲区
pub const TCP_BUFFER_LENGTH: usize = 8 * 1024;

/// HTTP 探测用 peek 缓冲区
pub const PEEK_TCP_BUFFER_LENGTH: usize = 1024;

#[derive(Clone)]
pub struct TCPHandler {
    context: Arc<Context>,
    listener: Arc<TcpListener>,
}

impl TCPHandler {
    /// 创建并 bind TCPHandler
    pub async fn bind(context: Arc<Context>) -> anyhow::Result<Arc<Self>> {
        let addr = format!("{}:{}", context.ip, context.port);
        let listener = TcpListener::bind(&addr).await?;

        println!("TCP listening on {}", addr);

        Ok(Arc::new(Self {
            context,
            listener: Arc::new(listener),
        }))
    }

    /// 启动 accept loop（阻塞）
    pub async fn start(self: Arc<Self>) -> anyhow::Result<()> {
        let cloned = self.context.token.clone();
        loop {
            tokio::select! {
                _ = cloned.cancelled() => {
                    println!("TCP listener shutting down");
                    break;
                }
                res = self.listener.accept() => {
                    match res {
                        Ok((socket, addr)) => {
                            let this = self.clone();
                            let cloned = self.context.token.clone();
                            tokio::spawn(async move {
                                this.handle_connection(socket, addr, cloned).await;
                            });
                        }
                        Err(e) => eprintln!("TCP accept error: {:?}", e),
                    }
                }
            }
        }
        Ok(())
    }



async fn handle_connection(
    self: Arc<Self>,
    socket: TcpStream,
    addr: std::net::SocketAddr,
    token: CancellationToken,
) {
    println!("TCP connection from {}", addr);

    // ⚠️ 关键：TcpStream 放进 Option
    let stream = Arc::new(Mutex::new(Some(socket)));

    // ========= HTTP 探测 =========
    {
        let mut guard = stream.lock().await;

        match HTTPHandler::is_http_connection(&guard).await {
            Ok(true) => {
                println!("HTTP connection detected from {}", addr);

                // ⚠️ HTTPHandler 也必须基于 Option<TcpStream>
                let _ = HTTPHandler::new(stream.clone(), self.context.clone())
                    .start(token)
                    .await;
                return;
            }
            Ok(false) => {}
            Err(e) => {
                eprintln!("HTTP detection error: {:?}", e);
                return;
            }
        }
    } // 🔑 释放锁

    // ========= 普通 TCP =========
    let mut buf = vec![0u8; TCP_BUFFER_LENGTH];
    let protocol = ClientType::TCP(stream.clone());

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                println!("TCP connection shutdown {}", addr);
                break;
            }

            res = async {
                let mut guard = stream.lock().await;
                match guard.as_mut() {
                    Some(s) => s.read(&mut buf).await,
                    None => Ok(0), // 已关闭，等价 EOF
                }
            } => {
                match res {
                    Ok(0) => break,
                    Ok(n) => {
                        // ⚠️ 不在持锁状态下 await
                        self.clone()
                            .on_data(&protocol, &buf[..n])
                            .await
                            .ok();
                    }
                    Err(_) => break,
                }
            }
        }
    }

    println!("TCP connection closed {}", addr);
}

}

#[async_trait]
impl Listener for TCPHandler {
    async fn run(&mut self) -> anyhow::Result<()> {
        // start expects an Arc<Self>, so clone the handler into an Arc and call start on it
        let arc_self = Arc::new(self.clone());
        arc_self.start().await
    }
    async fn new(context: Arc<Context>) -> Arc<Self> {
        TCPHandler::bind(context).await.unwrap()
    }
    async fn stop(self: &Arc<Self>) -> anyhow::Result<()> {
        // TCPListener does not have a built-in stop method.
        // You would need to implement your own mechanism to stop the listener.
        self.context.token.cancel();
        let _ = TcpStream::connect(format!("{}:{}", self.context.ip, self.context.port)).await;
        Ok(())
    }

async fn on_data(
    self: &Arc<Self>,
    client_type: &ClientType,
    received: &[u8],
) -> anyhow::Result<()> {
    if let ClientType::TCP(tcp) = client_type {
        // 只在这个作用域内持有锁
        let mut guard = tcp.lock().await;

        let stream = match guard.as_mut() {
            Some(s) => s,
            None => {
                // TCP 已关闭，直接忽略数据
                return Ok(());
            }
        };

        let peer = stream.peer_addr()?;
        println!(
            "TCP received {} bytes from {}",
            received.len(),
            peer
        );

        let bytes = received.to_vec();

        match Frame::verify_bytes(&bytes) {
            Ok(frame) => {
                // 协议帧：交给 CommandParser
                // ⚠️ 不在持锁状态下 await
                drop(guard);
                CommandParser::on(&frame, self.context.clone(), client_type).await;
            }
            Err(_) => {
                // 非协议帧：当普通 TCP 数据回写
                stream.write_all(received).await?;
            }
        }
    }

    Ok(())
}

    async fn send(self: &Arc<Self>, protocol_type: &ClientType, data: &[u8]) -> anyhow::Result<()> {
        if let ClientType::TCP(stream) = protocol_type {
            let mut guard = stream.lock().await;
            let guard = match guard.as_mut() {
                Some(s) => s,
                None => {
                    // TCP 已关闭，无法发送
                    return Err(anyhow::anyhow!("TCP stream already closed"));
                }
            };
            let peer = guard.peer_addr().unwrap();
            println!("TCP is sending {} bytes to {}", data.len(), &peer);
            guard.write_all(data).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::nodes::servers::Servers;
    use crate::protocols::command::{Entity, Action};

    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use zz_account::address::FreeWebMovementAddress;

    // #[tokio::test]
    // async fn test_tcp_echo() -> anyhow::Result<()> {
    //     let ip = "127.0.0.1";
    //     let port = 18000;
    //     let address = FreeWebMovementAddress::random();
    //     let context = Arc::new(Context::new(ip.to_string(), port, address));
    //     let server = TCPHandler::bind(context).await?;

    //     tokio::spawn(server.clone().start());

    //     tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    //     let mut stream = TcpStream::connect(format!("{}:{}", ip, port)).await?;
    //     let msg = b"hello tcp";

    //     stream.write_all(msg).await?;

    //     let mut buf = vec![0u8; msg.len()];
    //     stream.read_exact(&mut buf).await?;

    //     assert_eq!(buf, msg);

    //     server.stop().await?;

    //     Ok(())
    // }

    #[tokio::test]
    async fn test_node_online_between_two_nodes() -> anyhow::Result<()> {
        let ip = "127.0.0.1";
        let port_b = 19001;

        // ===== 1️⃣ Node B（被通知方） =====
        let addr_b = FreeWebMovementAddress::random();
        let context_b = Arc::new(Context::new(ip.to_string(), port_b, addr_b.clone()));
        let server_b = TCPHandler::bind(context_b.clone()).await?;

        let server_task = tokio::spawn(server_b.clone().start());

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // ===== 2️⃣ Node A（上线方） =====
        let addr_a = FreeWebMovementAddress::random();
        let mut stream_a = TcpStream::connect(format!("{}:{}", ip, port_b)).await?;

        // 构造 Node::OnLine Frame（external 示例）
        let frame = Frame::build_node_command(
            &addr_a,
            Entity::Node,
            Action::OnLine,
            1,
            Some({
                let data = Servers::to_endpoints(&vec![], 1); // 空 endpoint 也合法
                data
            }),
        )?;

        let bytes = Frame::to(frame);

        // ===== 3️⃣ Node A 发送 Online =====
        stream_a.write_all(&bytes).await?;

        // 给 B 一点处理时间
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // ===== 4️⃣ 校验 Node B 状态 =====
        let clients = context_b.clients.lock().await;

        assert!(
            clients.external.contains_key(&addr_a.to_string())
                || clients.inner.contains_key(&addr_a.to_string()),
            "Node B should record Node A as online client"
        );

        // ===== 5️⃣ 清理 =====
        server_b.stop().await?;
        let _ = server_task.await?;

        Ok(())
    }
}
