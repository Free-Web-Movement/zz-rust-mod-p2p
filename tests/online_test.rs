use std::{ net::SocketAddr, sync::Arc };

use aex::{
    on,
    server::HTTPServer,
    storage::Storage,
    tcp::{ router::Router, types::Command },
};
use zz_p2p::{
    protocols::{
        command::{ Action, Entity, P2PCommand },
        commands::online:: online_handler ,
        frame::P2PFrame,
    },
    stored_files::StoredFiles,
};

#[tokio::test]
async fn test_p2p_handshake_real_server() {
    // --- 1. 初始化全局上下文 ---
    let addr: SocketAddr = "127.0.0.1:9001".parse().unwrap();

    // --- 2. 启动真实 Server 监听 ---
    println!("📡 Server listening on {}", addr);

    let storage = Storage::new(None);

    let files = StoredFiles::new(storage, None, None, None);

    let node = zz_p2p::node::Node::init("".to_string(), files.clone(), addr);

    let server = {
        let guard = node.lock().await;
        HTTPServer::new(addr, Some(guard.context.clone()))
    };

    let mut router = Router::new();

    on!(router, P2PFrame, P2PCommand, [
        [P2PCommand::to_u32(Entity::Node, Action::OnLine), online_handler],
    ]);

    server
        .tcp(router)
        .start::<P2PFrame, P2PCommand>(Arc::new(|c| c.id())).await
        .unwrap();
}
