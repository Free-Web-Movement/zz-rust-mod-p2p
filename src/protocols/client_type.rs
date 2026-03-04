use std::{net::SocketAddr, sync::Arc};

use aex::tcp::types::Codec;
use tokio::{
    io::AsyncReadExt,
    net::{
        TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
    sync::Mutex,
};

use anyhow::Context as AnContext;
use anyhow::Result;

use crate::{
    context::Context,
    // handlers::ws::WebSocketHandler,
    protocols::{frame::P2PFrame},
};

pub async fn loop_read(
    client_type: &(Arc<Mutex<OwnedReadHalf>>, Arc<Mutex<OwnedWriteHalf>>),
    context: &Arc<Context>,
    addr: SocketAddr,
) {
    println!("inside tcp loop reading");
    let context = context.clone();
    let ct = client_type.clone();

    tokio::spawn(async move {
        loop {
            if let Err(e) = read_one_tcp_frame(context.clone(), &ct.clone(), addr).await {
                eprintln!("TCP read loop exit {:?} - {:?}", addr, e);
                break;
            }
        }

        println!("TCP read loop closed {:?}", addr);
    });
}

pub async fn loop_reading(
    client_type: &(Arc<Mutex<OwnedReadHalf>>, Arc<Mutex<OwnedWriteHalf>>),
    context: &Arc<Context>,
    addr: SocketAddr,
) {
    println!("inside loop read!");

    loop_read(client_type, context, addr).await;
}

pub fn to_client_type(
    stream: TcpStream,
) -> (Arc<Mutex<OwnedReadHalf>>, Arc<Mutex<OwnedWriteHalf>>) {
    let (reader, writer) = stream.into_split();
    let reader = Arc::new(Mutex::new(reader));
    let writer = Arc::new(Mutex::new(writer));
    (reader, writer)
}

pub async fn on_data(
    client_type: &(Arc<Mutex<OwnedReadHalf>>, Arc<Mutex<OwnedWriteHalf>>),
    context: &Arc<Context>,
    addr: SocketAddr,
) {
    on_tcp_data(client_type, context, addr).await;
}

pub async fn read_http(
    client_type: &(Arc<Mutex<OwnedReadHalf>>, Arc<Mutex<OwnedWriteHalf>>),
    context: &Arc<Context>,
    addr: SocketAddr,
) {
    on_http_data(client_type, context, addr).await
}

pub async fn on_http_data(
    client_type: &(Arc<Mutex<OwnedReadHalf>>, Arc<Mutex<OwnedWriteHalf>>),
    context: &Arc<Context>,
    addr: SocketAddr,
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
                let (reader, _) = client_type;
                let mut reader = reader.lock().await;
                reader.read(&mut buf).await
            } => {
                match res {
                    Ok(0)=>{break;}
                    Err(_) => todo!(),
                    Ok(1_usize..) => todo!()
                                    }
            }
        }
    }

    println!("HTTP connection closed {:?}", addr);
}

pub async fn on_tcp_data(
    client_type: &(Arc<Mutex<OwnedReadHalf>>, Arc<Mutex<OwnedWriteHalf>>),
    context: &Arc<Context>,
    addr: SocketAddr,
) {
    loop {
        tokio::select! {
            _ = context.token.cancelled() => {
                println!("TCP connection shutdown {:?}", addr);
                break;
            }

            res = read_one_tcp_frame(
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

pub async fn stop(
    _client_type: &(Arc<Mutex<OwnedReadHalf>>, Arc<Mutex<OwnedWriteHalf>>),
    context: &Arc<Context>,
) -> anyhow::Result<()> {
    context.token.cancel();
    let _ = TcpStream::connect(format!("{}:{}", context.ip, context.port)).await;
    Ok(())
}

pub async fn read_one_tcp_frame(
    context: Arc<Context>,
    client_type: &(Arc<Mutex<OwnedReadHalf>>, Arc<Mutex<OwnedWriteHalf>>),
    addr: SocketAddr,
) -> Result<()> {
    let mut len_buf = [0u8; 4];

    // ---------- 1. 读取长度 ----------
    {
        let (reader, _) = client_type.clone();
        let mut reader = reader.lock().await;
        reader
            .read_exact(&mut len_buf)
            .await
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
        let ( reader, _) = client_type.clone();
        let mut reader = reader.lock().await;
        reader
            .read_exact(&mut msg_buf)
            .await
            .with_context(|| format!("TCP read body error from {:?}", addr))?;
    }

    println!("TCP received {} bytes from {:?}", msg_len, addr);
    println!("{:?}", msg_buf);

    // ---------- 3. Frame 处理 ----------
    let frame: P2PFrame = Codec::decode(&msg_buf).unwrap();

    // let writer = get_writer(client_type).await;
    let (_, writer) = client_type;
    // CommandHandlerRegistry::on(frame, context, writer.clone()).await;
    Ok(())
}
