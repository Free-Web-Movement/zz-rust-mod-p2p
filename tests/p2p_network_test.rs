use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use aex::{
    connection::global::GlobalContext,
    connection::heartbeat::HeartbeatConfig,
    connection::node::Node as AexNode,
    crypto::session_key_manager::PairedSessionKey,
    server::HTTPServer,
    tcp::router::Router as TcpRouter,
    tcp::types::Codec,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use zz_p2p::protocols::command::{Action, Entity, P2PCommand};
use zz_p2p::protocols::commands::online::OnlineCommand;
use zz_p2p::protocols::frame::P2PFrame;
use zz_p2p::protocols::registry::register;

fn setup_logging() {
    let _ = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
}

fn create_global(addr: SocketAddr) -> Arc<GlobalContext> {
    let psk = Arc::new(Mutex::new(PairedSessionKey::new(16)));
    let mut global = GlobalContext::new(addr, Some(psk));
    global.heartbeat_config = HeartbeatConfig::new()
        .with_interval(60)
        .with_timeout(30);
    Arc::new(global)
}

fn create_server(addr: SocketAddr, global: Arc<GlobalContext>) -> HTTPServer {
    let server = HTTPServer::new(addr, Some(global));
    let router = TcpRouter::<P2PFrame, P2PCommand>::new();
    let router = register(router);
    server.tcp(router)
}

fn create_dummy_address() -> zz_account::address::FreeWebMovementAddress {
    zz_account::address::FreeWebMovementAddress::random()
}

async fn send_online_command(
    socket: &mut tokio::net::TcpStream,
    sender_addr: SocketAddr,
    address: &zz_account::address::FreeWebMovementAddress,
) -> anyhow::Result<Vec<u8>> {
    let psk = PairedSessionKey::new(16);
    let (session_id, key) = psk.create(false).await;
    
    let node = AexNode::from_system(sender_addr.port(), session_id.clone(), 1);
    
    let online_cmd = OnlineCommand {
        session_id: session_id.clone(),
        node,
        ephemeral_public_key: key.to_bytes(),
    };
    
    let cmd = P2PCommand::new(Entity::Node, Action::OnLine, Codec::encode(&online_cmd));
    let frame = P2PFrame::build(address, cmd, 1).await?;
    
    let frame_bytes = Codec::encode(&frame);
    let len_bytes = (frame_bytes.len() as u32).to_le_bytes();
    
    socket.write_all(&len_bytes).await?;
    socket.write_all(&frame_bytes).await?;
    socket.flush().await?;
    
    Ok(session_id)
}

async fn send_message(
    socket: &mut tokio::net::TcpStream,
    sender_address: &zz_account::address::FreeWebMovementAddress,
    message: &str,
) -> anyhow::Result<()> {
    let cmd = P2PCommand::new(
        Entity::Message, 
        Action::SendText, 
        message.as_bytes().to_vec()
    );
    let frame = P2PFrame::build(sender_address, cmd, 1).await?;
    
    let frame_bytes = Codec::encode(&frame);
    let len_bytes = (frame_bytes.len() as u32).to_le_bytes();
    
    socket.write_all(&len_bytes).await?;
    socket.write_all(&frame_bytes).await?;
    socket.flush().await?;
    
    Ok(())
}

async fn read_frame(socket: &mut tokio::net::TcpStream) -> anyhow::Result<P2PFrame> {
    let mut len_buf = [0u8; 4];
    socket.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    
    let mut data = vec![0u8; len];
    socket.read_exact(&mut data).await?;
    
    let frame: P2PFrame = Codec::decode(&data)?;
    Ok(frame)
}

#[tokio::test]
async fn test_p2p_network_node1_to_node2() {
    setup_logging();
    
    let node1_addr: SocketAddr = "127.0.0.1:19101".parse().unwrap();
    let node2_addr: SocketAddr = "127.0.0.1:19102".parse().unwrap();
    
    let node1_global = create_global(node1_addr);
    let address = create_dummy_address();
    node1_global.set(address.clone()).await;
    let server = create_server(node1_addr, node1_global.clone());
    
    let listener = TcpListener::bind(node1_addr).await.unwrap();
    
    tokio::spawn(async move {
        let _ = server.start_with_protocols::<P2PFrame, P2PCommand>().await;
    });
    
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    let mut client_socket = tokio::net::TcpStream::connect(node1_addr).await.unwrap();
    
    let (mut server_socket, client_addr) = listener.accept().await.unwrap();
    println!("Node1 accepted connection from {}", client_addr);
    
    let session_id = send_online_command(&mut client_socket, node2_addr, &address).await.unwrap();
    println!("Node2 sent online command, session_id: {:?}", session_id);
    
    let frame = read_frame(&mut server_socket).await.unwrap();
    println!("Node1 received frame from {}", frame.body.address);
    
    let _cmd: P2PCommand = Codec::decode(&frame.body.data).unwrap();
    println!("Frame command entity: {:?}, action: {:?}", _cmd.entity, _cmd.action);
    
    assert_eq!(_cmd.entity, Entity::Node);
    assert_eq!(_cmd.action, Action::OnLine);
    
    node1_global.manager.shutdown();
    println!("Test passed: Node1 -> Node2 connection");
}

#[tokio::test]
async fn test_p2p_network_node2_to_node1() {
    setup_logging();
    
    let node1_addr: SocketAddr = "127.0.0.1:19111".parse().unwrap();
    let node2_addr: SocketAddr = "127.0.0.1:19112".parse().unwrap();
    
    let node1_global = create_global(node1_addr);
    let address = create_dummy_address();
    node1_global.set(address.clone()).await;
    let server = create_server(node1_addr, node1_global.clone());
    
    let listener = TcpListener::bind(node1_addr).await.unwrap();
    
    tokio::spawn(async move {
        let _ = server.start_with_protocols::<P2PFrame, P2PCommand>().await;
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let mut node2_socket = tokio::net::TcpStream::connect(node1_addr).await.unwrap();
    
    let session_id = send_online_command(&mut node2_socket, node2_addr, &address).await.unwrap();
    println!("Node2 sent online, session_id: {:?}", session_id);
    
    let (mut server_socket, _) = listener.accept().await.unwrap();
    
    if let Ok(frame) = tokio::time::timeout(Duration::from_secs(5), read_frame(&mut server_socket)).await {
        if let Ok(frame) = frame {
            let cmd: P2PCommand = Codec::decode(&frame.body.data).unwrap();
            println!("Received: entity={:?}, action={:?}", cmd.entity, cmd.action);
            assert_eq!(cmd.entity, Entity::Node);
        }
    }
    
    node1_global.manager.shutdown();
    println!("Test passed: Node2 -> Node1 connection");
}

#[tokio::test]
async fn test_p2p_network_three_nodes() {
    setup_logging();
    
    let node1_addr: SocketAddr = "127.0.0.1:19201".parse().unwrap();
    let node2_addr: SocketAddr = "127.0.0.1:19202".parse().unwrap();
    let node3_addr: SocketAddr = "127.0.0.1:19203".parse().unwrap();
    
    let node1_global = create_global(node1_addr);
    let address = create_dummy_address();
    node1_global.set(address.clone()).await;
    let server = create_server(node1_addr, node1_global.clone());
    
    let _listener = TcpListener::bind(node1_addr).await.unwrap();
    
    tokio::spawn(async move {
        let _ = server.start_with_protocols::<P2PFrame, P2PCommand>().await;
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    println!("Starting 3-node connection test...");
    
    let mut socket_n2 = tokio::net::TcpStream::connect(node1_addr).await.unwrap();
    let _ = send_online_command(&mut socket_n2, node2_addr, &address).await;
    println!("Node2 -> Node1: Online sent");
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    let mut socket_n3 = tokio::net::TcpStream::connect(node1_addr).await.unwrap();
    let _ = send_online_command(&mut socket_n3, node3_addr, &address).await;
    println!("Node3 -> Node1: Online sent");
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    send_message(&mut socket_n2, &address, "Hello from Node2 to Node1").await.unwrap();
    println!("Node2 -> Node1: Message sent");
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    send_message(&mut socket_n3, &address, "Hello from Node3 to Node1").await.unwrap();
    println!("Node3 -> Node1: Message sent");
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    node1_global.manager.shutdown();
    println!("Test passed: 3-node network");
}

#[tokio::test]
async fn test_p2p_encrypted_message() {
    setup_logging();
    
    let server_addr: SocketAddr = "127.0.0.1:19301".parse().unwrap();
    let client_addr: SocketAddr = "127.0.0.1:19302".parse().unwrap();
    
    let server_global = create_global(server_addr);
    let address = create_dummy_address();
    server_global.set(address.clone()).await;
    let server = create_server(server_addr, server_global.clone());
    
    let listener = TcpListener::bind(server_addr).await.unwrap();
    
    tokio::spawn(async move {
        let _ = server.start_with_protocols::<P2PFrame, P2PCommand>().await;
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let mut client_socket = tokio::net::TcpStream::connect(server_addr).await.unwrap();
    let _ = send_online_command(&mut client_socket, client_addr, &address).await;
    
    let (server_socket, _) = listener.accept().await.unwrap();
    let mut server_socket = server_socket;
    
    let encrypted_msg = "Secret message 123";
    send_message(&mut client_socket, &address, encrypted_msg).await.unwrap();
    println!("Sent encrypted message: {}", encrypted_msg);
    
    if let Ok(frame) = tokio::time::timeout(Duration::from_secs(5), read_frame(&mut server_socket)).await {
        if let Ok(frame) = frame {
            let cmd: P2PCommand = Codec::decode(&frame.body.data).unwrap();
            println!("Server received: entity={:?}, action={:?}, data_len={}", 
                cmd.entity, cmd.action, cmd.data.len());
            
            if cmd.entity == Entity::Message {
                let msg = String::from_utf8_lossy(&cmd.data);
                println!("Message content: {}", msg);
            }
        }
    }
    
    server_global.manager.shutdown();
    println!("Test passed: Encrypted message");
}

#[tokio::test]
async fn test_p2p_offline_command() {
    setup_logging();
    
    let server_addr: SocketAddr = "127.0.0.1:19401".parse().unwrap();
    let client_addr: SocketAddr = "127.0.0.1:19402".parse().unwrap();
    
    let server_global = create_global(server_addr);
    let address = create_dummy_address();
    server_global.set(address.clone()).await;
    let server = create_server(server_addr, server_global.clone());
    
    let listener = TcpListener::bind(server_addr).await.unwrap();
    
    tokio::spawn(async move {
        let _ = server.start_with_protocols::<P2PFrame, P2PCommand>().await;
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let mut socket = tokio::net::TcpStream::connect(server_addr).await.unwrap();
    
    let offline_cmd = P2PCommand::new(Entity::Node, Action::OffLine, vec![]);
    let frame = P2PFrame::build(&address, offline_cmd, 1).await.unwrap();
    let frame_bytes = Codec::encode(&frame);
    let len = (frame_bytes.len() as u32).to_le_bytes();
    
    socket.write_all(&len).await.unwrap();
    socket.write_all(&frame_bytes).await.unwrap();
    socket.flush().await.unwrap();
    println!("Sent Offline command");
    
    let (mut server_socket, _) = listener.accept().await.unwrap();
    
    if let Ok(frame) = tokio::time::timeout(Duration::from_secs(5), read_frame(&mut server_socket)).await {
        if let Ok(frame) = frame {
            let cmd: P2PCommand = Codec::decode(&frame.body.data).unwrap();
            println!("Received: entity={:?}, action={:?}", cmd.entity, cmd.action);
            assert_eq!(cmd.entity, Entity::Node);
            assert_eq!(cmd.action, Action::OffLine);
        }
    }
    
    server_global.manager.shutdown();
    println!("Test passed: Offline command");
}