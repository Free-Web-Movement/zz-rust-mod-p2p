use std::{ net::SocketAddr, sync::Arc };

use anyhow::Error;
use tokio::{ io::{ AsyncReadExt, AsyncWriteExt }, net::{ TcpStream, UdpSocket }, sync::Mutex };
use zz_account::address::FreeWebMovementAddress;

use crate::{
    consts::TCP_BUFFER_LENGTH,
    context:: Context ,
    handlers::ws::WebSocketHandler,
    protocols::{ command::{ Action, Entity }, frame::Frame },
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

    pub async fn send_online(&self, address: &FreeWebMovementAddress) -> Result<(), Error> {
        let frame = Frame::build_node_command(
            address, // 本节点地址
            Entity::Node,
            Action::OnLine, // 用 ResponseAddress 表示发送自身地址
            1,
            Some(address.to_string().as_bytes().to_vec())
        )?;
        let bytes = Frame::to(frame);

        let mut guard = self.writer.lock().await;

        let writer = &mut *guard;
        writer.write_all(&bytes).await?;
        Ok(())
    }

    pub async fn loop_read(
        &self,
        client_type: &ClientType,
        context: &Arc<Context>,
        addr: SocketAddr
    ) {
        println!("inside tcp loop reading");
        let reader = self.reader.clone();
        let client_type = client_type.clone();
        let context = context.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; TCP_BUFFER_LENGTH];
            loop {
                let n = {
                    let mut guard = reader.lock().await;
                    match guard.read(&mut buf).await {
                        Ok(0) => {
                            // 对端关闭连接
                            println!("Server closed!");
                            break;
                        }
                        Ok(n) => {
                            println!("Received {} bytes from server {:?}", n, addr);
                            n
                        }
                        Err(e) => {
                            eprintln!("Error reading from server {:?} - {:?}", addr, e);
                            break;
                        }
                    }
                };

                // 处理收到的消息
                let bytes = &buf[..n];
                // 这里可以解析 Frame 或 Command
                println!("Received {} bytes from server {:?}", n, addr);
                let frame = Frame::from(&bytes.to_vec());
                Frame::on(&frame, context.clone(), &client_type).await;
                // TODO: Frame::from(data) 或 CommandSender 处理逻辑
            }
        });
    }

    pub async fn is_http_connection(&self) -> anyhow::Result<bool> {
        let mut buf = [0u8; TCP_BUFFER_LENGTH];

        let mut reader = self.reader.lock().await;

        let n = reader.peek(&mut buf).await?;

        if n == 0 {
            return Ok(false);
        }

        let s = std::str::from_utf8(&buf[..n]).unwrap_or("");
        Ok(matches!(s, "GET" | "POST" | "PUT" | "DELETE" | "HEAD" | "OPTIONS" | "PATCH"))
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
            if let Err(e) = writer.write_all(&bytes).await {
                eprintln!("Failed to send TCP bytes: {:?}", e);
            }
        }
    }
}

pub async fn send_online(
    client_type: &ClientType,
    address: &FreeWebMovementAddress,
    data: Option<Vec<u8>>
) -> anyhow::Result<()> {
    let frame = Frame::build_node_command(
        &address, // 本节点地址
        Entity::Node,
        Action::OnLine, // 用 ResponseAddress 表示发送自身地址
        1,
        data
    )?;
    let bytes = Frame::to(frame);

    send_bytes(client_type, &bytes).await;

    Ok(())
}

pub async fn send_offline(
    client_type: &ClientType,
    address: &FreeWebMovementAddress,
    data: Option<Vec<u8>>
) -> anyhow::Result<()> {
    // 1️⃣ 构建在线命令 Frame
    let frame = Frame::build_node_command(address, Entity::Node, Action::OffLine, 1, data)?;

    // 2️⃣ 序列化 Frame
    let bytes = Frame::to(frame);
    send_bytes(&client_type, &bytes).await;
    // self.send(&bytes).await?;
    Ok(())
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
    client_type: &ClientType,
    stream_pair: &StreamPair,
    context: &Arc<Context>,
    addr: SocketAddr
) {
    let mut buf = vec![0u8; 64 * 1024];
    let token = context.clone().token.clone();

    let ctx = context.clone();

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
                    Ok(n) => {
                        // ❗ 已经释放 reader 锁
                    let data = &buf[..n];

                    // WebSocket upgrade
                    if WebSocketHandler::is_websocket_request(data) {
                        let ws = Arc::new(WebSocketHandler::new(
                            Arc::new(client_type.clone()),
                            ctx,
                        ));

                        // ⚠️ 升级阶段不要持锁
                        let _ = ws.respond_websocket_handshake(data).await;
                        break;
                    }
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
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        tokio::select! {
            _ = context.token.cancelled() => {
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
                    Ok(n) => {
                      println!("TCP received {} bytes", n);
                      let frame = Frame::from(&buf[..n].to_vec());
                      Frame::on(&frame, context.clone(), client_type).await;
                    }
                    Err(e) => {
                        eprintln!("TCP read error from {:?}: {:?}", addr, e);
                        break;
                    }
                }
            }
        }
    }

    println!("TCP connection closed {:?}", addr);
}

pub async fn is_http_connection(client_type: &ClientType) -> anyhow::Result<bool> {
    match client_type {
        ClientType::UDP { socket: _, peer: _ } => { Ok(false) }
        | ClientType::TCP(stream_pair)
        | ClientType::HTTP(stream_pair)
        | ClientType::WS(stream_pair) => {
            stream_pair.is_http_connection().await
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

pub async fn forward_frame(receiver: String ,frame: &Frame, context: Arc<Context>) {
      // ===== 2️⃣ 查本地 clients =====
    {
        let clients = context.clients.lock().await;
        let conns = clients.get_connections(&receiver, true);

        if !conns.is_empty() {
            let bytes = Frame::to(frame.clone());
            for ct in conns {
                send_bytes(&ct, &bytes).await;
            }
            return; // 🚨 非常重要
        }
    }

    // ===== 3️⃣ 查 servers，向其它服务器转发 =====
    let servers = &context.clone().servers;
    let servers = servers.lock().await;
    let bytes = Frame::to(frame.clone());

    if let Some(servers) = servers.connected_servers.clone() {
        let all = servers.inner.iter().chain(servers.external.iter());

        for server in all {
            send_bytes(&server.client_type, &bytes).await;
        }
    }
}
