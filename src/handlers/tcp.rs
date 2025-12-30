use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::net::{TcpListener, TcpStream};

use crate::context::Context;
use crate::protocols::client_type::{is_http_connection, on_data, read_http, to_client_type};
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

                read_http(&client_type, &self.context, addr).await;
                return;
            }
            Ok(false) => {}
            Err(e) => {
                eprintln!("HTTP detection error: {:?}", e);
                return;
            }
        }
    } // 🔑 释放锁
    on_data(&client_type, &self.context, addr).await;
}

}

#[async_trait]
impl Listener for TCPHandler {
    async fn run(&mut self) -> anyhow::Result<()> {
        // start expects an Arc<Self>, so clone the handler into an Arc and call start on it
        let arc_self = Arc::new(self.clone());
        arc_self.start().await
    }
}
