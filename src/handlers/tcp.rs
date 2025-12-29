use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::net::{TcpListener, TcpStream};

use crate::context::Context;
use crate::protocols::client_type::{ClientType, is_http_connection, on_data, read_http, to_client_type};
use crate::protocols::defines::{Listener};

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
                            tokio::spawn(async move {
                                this.handle_connection(socket, addr).await;
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
    addr: SocketAddr,
) {
    println!("TCP connection from {:?}", addr);

    // ⚠️ 关键：TcpStream 放进 Option
    let client_type = to_client_type(socket);

    // ========= HTTP 探测 =========
    {
        // let mut guard = stream.lock().await;

        match is_http_connection(&client_type).await {
            Ok(true) => {
                println!("HTTP connection detected from {:?}", addr);

                read_http(&client_type, &self.context, addr);
                return;
            }
            Ok(false) => {}
            Err(e) => {
                eprintln!("HTTP detection error: {:?}", e);
                return;
            }
        }
    } // 🔑 释放锁
    on_data(&client_type, &self.context, addr);
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

//   on_data(client_type, &self.context, addr);
//     if let ClientType::TCP(tcp) = client_type {
//         // 只在这个作用域内持有锁
//         let mut guard = tcp.lock().await;

//         let stream = match guard.as_mut() {
//             Some(s) => s,
//             None => {
//                 // TCP 已关闭，直接忽略数据
//                 return Ok(());
//             }
//         };

//         let peer = stream.peer_addr()?;
//         println!(
//             "TCP received {} bytes from {}",
//             received.len(),
//             peer
//         );

//         let bytes = received.to_vec();

//         match Frame::verify_bytes(&bytes) {
//             Ok(frame) => {
//                 // 协议帧：交给 CommandParser
//                 // ⚠️ 不在持锁状态下 await
//                 drop(guard);
//                 CommandParser::on(&frame, self.context.clone(), client_type).await;
//             }
//             Err(_) => {
//                 // 非协议帧：当普通 TCP 数据回写
//                 stream.write_all(received).await?;
//             }
//         }
//     }

    Ok(())
}

    async fn send(self: &Arc<Self>, protocol_type: &ClientType, data: &[u8]) -> anyhow::Result<()> {
    //     if let ClientType::TCP(stream) = protocol_type {
    //         let mut guard = stream.lock().await;
    //         let guard = match guard.as_mut() {
    //             Some(s) => s,
    //             None => {
    //                 // TCP 已关闭，无法发送
    //                 return Err(anyhow::anyhow!("TCP stream already closed"));
    //             }
    //         };
    //         let peer = guard.peer_addr().unwrap();
    //         println!("TCP is sending {} bytes to {}", data.len(), &peer);
    //         guard.write_all(data).await?;
    //     }

        Ok(())
    }
}
