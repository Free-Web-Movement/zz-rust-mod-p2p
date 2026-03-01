use std::{ net::SocketAddr, sync::Arc };

use aex::{http::protocol::method::HttpMethod, tcp::types::Codec};
use tokio::{ io::{ AsyncReadExt, AsyncWriteExt }, net::{ TcpStream, UdpSocket }, sync::Mutex };

use anyhow::Result;
use anyhow::Context as AnContext;

use crate::{
    context::Context,
    // handlers::ws::WebSocketHandler,
    protocols::{ frame::P2PFrame, registry::CommandHandlerRegistry },
};

/// 每个 TCP/HTTP/WS 连接，拆分成 reader/writer
#[derive(Debug, Clone)]
pub struct StreamPair {
    pub reader: Arc<Mutex<tokio::net::tcp::OwnedReadHalf>>,
    pub writer: Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
}

#[derive(Debug, Clone)]
pub enum ClientType {
    UDP {
        socket: Arc<UdpSocket>,
        peer: SocketAddr,
    },
    TCP(StreamPair),
    HTTP(StreamPair),
    WS(StreamPair),
}

impl StreamPair {
    pub async fn close(&self) {
        // 先关闭写端（发送 FIN）
        {
            let mut writer = self.writer.lock().await;
            let _ = writer.shutdown().await;
        }

        // 再关闭读端（drop）
        {
            let reader = self.reader.lock().await;
            drop(reader);
        }
    }

    pub async fn send(&self, bytes: &[u8]) {
        let mut writer = self.writer.lock().await;
        let _ = writer.write_all(&bytes).await;
    }

    pub async fn loop_read(
        &self,
        client_type: &ClientType,
        context: &Arc<Context>,
        addr: SocketAddr
    ) {
        println!("inside tcp loop reading");

        // let reader = self.reader.clone();
        let stream_pair = self.clone();
        let client_type = client_type.clone();
        let context = context.clone();

        tokio::spawn(async move {
            loop {
                if
                    let Err(e) = read_one_tcp_frame(
                        &stream_pair,
                        context.clone(),
                        &client_type.clone(),
                        addr
                    ).await
                {
                    eprintln!("TCP read loop exit {:?} - {:?}", addr, e);
                    break;
                }
            }

            println!("TCP read loop closed {:?}", addr);
        });
    }
}

pub async fn loop_reading(client_type: &ClientType, context: &Arc<Context>, addr: SocketAddr) {
    println!("inside loop read!");
    match client_type {
        ClientType::UDP { socket: _, peer: _ } => todo!(),
        | ClientType::TCP(stream_pair)
        | ClientType::HTTP(stream_pair)
        | ClientType::WS(stream_pair) => {
            stream_pair.loop_read(client_type, context, addr).await;
        }
    }
}
pub async fn close_client_type(client_type: &ClientType) {
    match client_type {
        ClientType::UDP { socket: _, peer: _ } => todo!(),
        ClientType::TCP(sp) | ClientType::HTTP(sp) | ClientType::WS(sp) => {
            sp.close().await;
        }
    }
}
pub fn to_client_type(stream: TcpStream) -> ClientType {
    let (reader, writer) = stream.into_split();
    let reader = Arc::new(Mutex::new(reader));
    let writer = Arc::new(Mutex::new(writer));

    let sp = StreamPair {
        reader: reader.clone(),
        writer: writer.clone(),
    };
    let tcp = ClientType::TCP(sp);
    tcp
}

pub async fn send_bytes(client_type: &ClientType, bytes: &[u8]) {
    match client_type {
        crate::protocols::client_type::ClientType::UDP { socket, peer } => {
            println!("UDP is sending {} bytes to {:?}", bytes.len(), peer);
            if let Err(e) = socket.send_to(bytes, peer).await {
                eprintln!("Failed to send UDP bytes: {:?}", e);
            }
        }
        | crate::protocols::client_type::ClientType::TCP(stream_pair)
        | crate::protocols::client_type::ClientType::HTTP(stream_pair)
        | crate::protocols::client_type::ClientType::WS(stream_pair) => {
            let mut writer = stream_pair.writer.lock().await;
            println!("\nsend tcp {} bytes: {:?}\n", bytes.len(), bytes);

            let len = bytes.len() as u32;
            if let Err(e) = writer.write_all(&len.to_be_bytes()).await {
                eprintln!("Failed to send TCP bytes: {:?}", e);
            }
            if let Err(e) = writer.write_all(&bytes).await {
                eprintln!("Failed to send TCP bytes: {:?}", e);
            }
        }
    }
}

pub async fn on_data(client_type: &ClientType, context: &Arc<Context>, addr: SocketAddr) {
    match client_type {
        ClientType::UDP { socket: _, peer: _ } => todo!(),
        | ClientType::TCP(stream_pair)
        | ClientType::HTTP(stream_pair)
        | ClientType::WS(stream_pair) => {
            on_tcp_data(client_type, stream_pair, context, addr).await;
        }
    }
}

pub async fn read_http(client_type: &ClientType, context: &Arc<Context>, addr: SocketAddr) {
    match client_type {
        ClientType::UDP { socket: _, peer: _ } => todo!(),
        | ClientType::TCP(stream_pair)
        | ClientType::HTTP(stream_pair)
        | ClientType::WS(stream_pair) => {
            on_http_data(client_type, stream_pair, context, addr).await;
        }
    }
}

pub async fn on_http_data(
    _client_type: &ClientType,
    stream_pair: &StreamPair,
    context: &Arc<Context>,
    addr: SocketAddr
) {
    let mut buf = vec![0u8; 64 * 1024];
    let token = context.clone().token.clone();

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                println!("TCP connection shutdown {:?}", addr);
                break;
            }

            res = async {
                // 只在这里持锁
                let mut reader = stream_pair.reader.lock().await;
                reader.read(&mut buf).await
            } => {
                match res {
                    Ok(0) => {
                        // EOF
                        break;
                    }
                    Ok(_) => {
                        // ❗ 已经释放 reader 锁
                    // let data = &buf[..n];

                    // WebSocket upgrade
                    // if WebSocketHandler::is_websocket_request(data) {
                    //     let ws = Arc::new(WebSocketHandler::new(
                    //         Arc::new(client_type.clone()),
                    //         ctx,
                    //     ));

                    //     // ⚠️ 升级阶段不要持锁
                    //     let _ = ws.respond_websocket_handshake(data).await;
                    //     break;
                    // } else {
                      
                    // }
                    }
                    Err(e) => {
                        eprintln!("HTTP read error from {:?}: {:?}", addr, e);
                        break;
                    }
                }
            }
        }
    }

    println!("HTTP connection closed {:?}", addr);
}

pub async fn on_tcp_data(
    client_type: &ClientType,
    stream_pair: &StreamPair,
    context: &Arc<Context>,
    addr: SocketAddr
) {
    loop {
        tokio::select! {
            _ = context.token.cancelled() => {
                println!("TCP connection shutdown {:?}", addr);
                break;
            }

            res = read_one_tcp_frame(
                stream_pair,
                context.clone(),
                client_type,
                addr,
            ) => {
                if let Err(e) = res {
                    eprintln!("TCP connection error {:?}: {:?}", addr, e);
                    break;
                }
            }
        }
    }

    println!("TCP connection closed {:?}", addr);
}

pub async fn is_http_connection(client_type: &ClientType) -> anyhow::Result<bool> {
    match client_type {
        ClientType::UDP { socket: _, peer: _ } => Ok(false),
        | ClientType::TCP(stream_pair)
        | ClientType::HTTP(stream_pair)
        | ClientType::WS(stream_pair) => {
          let reader = stream_pair.reader.clone();
          let mut reader = reader.lock().await;
          HttpMethod::is_http_connection(&mut reader).await
        }
    }
}

pub async fn stop(client_type: &ClientType, context: &Arc<Context>) -> anyhow::Result<()> {
    context.token.cancel();
    match client_type {
        ClientType::UDP { socket: _, peer: _ } => {}
        | ClientType::TCP(_stream_pair)
        | ClientType::HTTP(_stream_pair)
        | ClientType::WS(_stream_pair) => {
            let _ = TcpStream::connect(format!("{}:{}", context.ip, context.port)).await;
        }
    }
    Ok(())
}

pub async fn read_one_tcp_frame(
    stream_pair: &StreamPair,
    context: Arc<Context>,
    client_type: &ClientType,
    addr: SocketAddr
) -> Result<()> {
    let mut len_buf = [0u8; 4];

    // ---------- 1. 读取长度 ----------
    {
        let mut reader = stream_pair.reader.lock().await;
        reader
            .read_exact(&mut len_buf).await
            .with_context(|| format!("TCP read length error from {:?}", addr))?;
    }

    let msg_len = u32::from_be_bytes(len_buf) as usize;

    println!("Received {} bytes from {:?}", msg_len, addr);

    // （可选但强烈建议的防御）
    if msg_len == 0 || msg_len > 16 * 1024 * 1024 {
        anyhow::bail!("invalid frame length {} from {:?}", msg_len, addr);
    }

    // ---------- 2. 读取消息体 ----------
    let mut msg_buf = vec![0u8; msg_len];
    {
        let mut reader = stream_pair.reader.lock().await;
        reader
            .read_exact(&mut msg_buf).await
            .with_context(|| format!("TCP read body error from {:?}", addr))?;
    }

    println!("TCP received {} bytes from {:?}", msg_len, addr);
    println!("{:?}", msg_buf);

    // ---------- 3. Frame 处理 ----------
    let frame:P2PFrame = Codec::decode(&msg_buf).unwrap();
    CommandHandlerRegistry::on(frame, context, Arc::new(client_type.clone())).await;

    Ok(())
}
