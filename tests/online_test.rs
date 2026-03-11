use std::{net::SocketAddr, sync::Arc, time::Duration};

use aex::{
    connection::{context::AexWriter, node::Node as AexNode},
    on,
    server::HTTPServer,
    storage::Storage,
    tcp::{router::Router, types::Command},
};
use tokio::{io::BufWriter, net::TcpStream};
use zz_p2p::{
    protocols::{
        command::{Action, Entity, P2PCommand},
        commands::online::{OnlineCommand, online_handler},
        frame::P2PFrame,
    },
    stored_files::StoredFiles,
};

// #[tokio::test]
// async fn test_p2p_handshake_real_server() {
//     // --- 1. 初始化全局上下文 ---
//     let addr: SocketAddr = "127.0.0.1:9001".parse().unwrap();

//     // --- 2. 启动真实 Server 监听 ---
//     println!("📡 Server listening on {}", addr);

//     let storage = Storage::new(None);

//     let files = StoredFiles::new(storage, None, None, None);

//     let node = zz_p2p::node::Node::init("".to_string(), files.clone(), addr);

//     let server = {
//         let guard = node.lock().await;
//         HTTPServer::new(addr, Some(guard.context.clone()))
//     };

//     let mut router = Router::new();

//     on!(
//         router,
//         P2PFrame,
//         P2PCommand,
//         [[
//             P2PCommand::to_u32(Entity::Node, Action::OnLine),
//             online_handler
//         ],]
//     );

//     tokio::select! {
//         // 分支 1: 运行服务器（这是一个永远阻塞的 Future，直到被 cancel）
//         res = server.tcp(router).start::<P2PFrame, P2PCommand>(Arc::new(|c| c.id())) => {
//             if let Err(e) = res {
//                 eprintln!("Server stopped with error: {}", e);
//             }
//         }

//         // 分支 2: 计时器（3秒后触发）
//         _ = tokio::time::sleep(Duration::from_secs(3)) => {
//             println!("Timeout reached, initiating shutdown...");
//             let g = node.lock().await;
//             g.context.shutdown_all().await;
//         }
//     }
// }

#[tokio::test]
async fn test_p2p_command_flow() {
    let version: u8 = 1;
    let addr: SocketAddr = "127.0.0.1:9002".parse().unwrap();
    let storage = Storage::new(None);
    let files = StoredFiles::new(storage, None, None, None);
    let node = zz_p2p::node::Node::init("".to_string(), files.clone(), addr);
    let node_clone = node.clone();

    // --- 1. 启动服务器 (使用你的代码逻辑，但放在后台运行) ---
    // 注意：实际测试中，server.start() 通常会阻塞，需要 tokio::spawn
    tokio::spawn(async move {
        let server = {
            let guard = node.lock().await;
            HTTPServer::new(addr, Some(guard.context.clone()))
        };

        let mut router = Router::new();
        on!(
            router,
            P2PFrame,
            P2PCommand,
            [[
                P2PCommand::to_u32(Entity::Node, Action::OnLine),
                online_handler
            ],]
        );

        server
            .tcp(router)
            .start::<P2PFrame, P2PCommand>(Arc::new(|c| c.id()))
            .await
            .unwrap();
    });

    // 给服务器一点启动时间
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // --- 2. 构造客户端并发送 aex 命令 ---

    // let stream = TcpStream::connect(addr).await.expect("Failed to connect");
    let mut stream = None;
    for _ in 0..10 {
        if let Ok(s) = TcpStream::connect(addr).await {
            stream = Some(s);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let stream = stream.expect("Failed to connect after retries");

    // 构造一个 "Online" 命令的 Frame
    // 这里的具体构造方式取决于 P2PFrame 的实现
    let psk = {
        let guard = node_clone.lock().await;
        let g = guard.context.clone();
        g.paired_session_keys.clone().unwrap()
    };

    let (id, key) = {
        let cloned = psk.clone();
        let guard = cloned.lock().await;
        guard.create(false).await
    };

    let aex_node = AexNode::from_system(addr.port(), id.clone(), version);
    let cmd = OnlineCommand {
        session_id: id,
        node: aex_node,
        ephemeral_public_key: key.to_bytes(),
    };
    let address = files.clone().address();

    let psk = {
        let guard = node_clone.lock().await;
        guard.context.clone().paired_session_keys.clone()
    };

    let (_, writer) = stream.into_split();

    let buf_writer = BufWriter::new(writer);

    let mut aex_writer: Box<AexWriter> = Box::new(buf_writer);

    let _ = P2PFrame::send::<OnlineCommand>(
        &address,
        &mut aex_writer,
        &Some(cmd),
        Entity::Node,
        Action::OnLine,
        psk,
    )
    .await;

    println!("✅ Command sent to server");
    tokio::time::sleep(Duration::from_secs(3)).await;
    {
        let g = node_clone.lock().await;
        g.context.shutdown_all().await
    }
}
