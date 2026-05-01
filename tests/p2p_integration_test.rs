use std::net::SocketAddr;
use std::sync::Arc;

use aex::{
    connection::manager::ConnectionManager,
    connection::node::Node as AexNode,
    connection::{global::GlobalContext, heartbeat::HeartbeatConfig},
    crypto::session_key_manager::PairedSessionKey,
    tcp::router::Router as TcpRouter,
    tcp::types::{Codec, Command, Frame},
};
use tokio::sync::Mutex;

use zz_p2p::protocols::command::{Action, Entity, P2PCommand};
use zz_p2p::protocols::commands::online::OnlineCommand;
use zz_p2p::protocols::frame::P2PFrame;
use zz_p2p::protocols::registry::register;

#[tokio::test]
async fn test_p2p_command_encoding() {
    let cmd = P2PCommand::new(Entity::Node, Action::OnLine, vec![1, 2, 3]);

    let id = cmd.id();
    assert_eq!(id, P2PCommand::to_u32(Entity::Node, Action::OnLine));
    assert_eq!(id, 257);

    let data = cmd.data();
    assert_eq!(data, &vec![1, 2, 3]);
}

#[tokio::test]
async fn test_online_command_encoding() {
    let node = AexNode::from_system(8080, vec![1u8; 32], 1);
    let online_cmd = OnlineCommand {session_id:vec![1,2,3,4],node, ephemeral_public_key:[0u8;32], announced_ips: vec![], seeds: vec![] };

    let encoded = Codec::encode(&online_cmd);
    assert!(!encoded.is_empty());

    let decoded: OnlineCommand = Codec::decode(&encoded).unwrap();
    assert_eq!(decoded.session_id, online_cmd.session_id);
}

#[tokio::test]
async fn test_message_command_flow() {
    let cmd = P2PCommand::new(Entity::Message, Action::SendText, b"Hello World".to_vec());

    let id = cmd.id();
    assert_eq!(id, P2PCommand::to_u32(Entity::Message, Action::SendText));

    let encoded = Codec::encode(&cmd);
    let decoded: P2PCommand = Codec::decode(&encoded).unwrap();

    assert_eq!(decoded.entity, Entity::Message);
    assert_eq!(decoded.action, Action::SendText);
}

#[tokio::test]
async fn test_offline_command_flow() {
    let cmd = P2PCommand::new(Entity::Node, Action::OffLine, vec![]);

    let id = cmd.id();
    assert_eq!(id, P2PCommand::to_u32(Entity::Node, Action::OffLine));

    let encoded = Codec::encode(&cmd);
    let decoded: P2PCommand = Codec::decode(&encoded).unwrap();

    assert_eq!(decoded.entity, Entity::Node);
    assert_eq!(decoded.action, Action::OffLine);
}

#[tokio::test]
async fn test_ack_command_flow() {
    let cmd = P2PCommand::new(Entity::Node, Action::OnLineAck, vec![1, 2, 3]);

    let id = cmd.id();
    assert_eq!(id, P2PCommand::to_u32(Entity::Node, Action::OnLineAck));

    let encoded = Codec::encode(&cmd);
    let decoded: P2PCommand = Codec::decode(&encoded).unwrap();

    assert_eq!(decoded.entity, Entity::Node);
    assert_eq!(decoded.action, Action::OnLineAck);
}

#[tokio::test]
async fn test_connection_manager_basic() {
    let manager = ConnectionManager::new();

    let status = manager.status();
    assert_eq!(status.total_ips, 0);
    assert_eq!(status.total_clients, 0);
    assert_eq!(status.total_servers, 0);
}

#[tokio::test]
async fn test_all_command_ids() {
    // Test that to_u32 and id() return the same value
    let test_cases = vec![
        (Entity::Node, Action::OnLine),
        (Entity::Node, Action::OnLineAck),
        (Entity::Node, Action::OffLine),
        (Entity::Node, Action::Ack),
        (Entity::Node, Action::Update),
        (Entity::Message, Action::SendText),
        (Entity::Message, Action::SendBinary),
    ];

    for (entity, action) in test_cases {
        let cmd = P2PCommand::new(entity, action, vec![]);
        let expected = P2PCommand::to_u32(entity, action);
        assert_eq!(
            cmd.id(),
            expected,
            "Entity: {:?}, Action: {:?}",
            entity,
            action
        );
    }
}

#[tokio::test]
async fn test_router_registration() {
    let router = TcpRouter::<P2PFrame, P2PCommand>::new();

    let online_id = P2PCommand::to_u32(Entity::Node, Action::OnLine);
    let offline_id = P2PCommand::to_u32(Entity::Node, Action::OffLine);
    let message_id = P2PCommand::to_u32(Entity::Message, Action::SendText);

    let registered_router = register(router);

    assert!(registered_router.handlers.contains_key(&online_id));
    assert!(registered_router.handlers.contains_key(&offline_id));
    assert!(registered_router.handlers.contains_key(&message_id));
}

#[tokio::test]
async fn test_context_global() {
    let addr: SocketAddr = "127.0.0.1:19999".parse().unwrap();
    let psk = Arc::new(Mutex::new(PairedSessionKey::new(16)));
    let mut global = GlobalContext::new(addr, Some(psk));
    global.heartbeat_config = HeartbeatConfig::new().with_interval(30).with_timeout(10);

    let status = global.manager.status();
    assert_eq!(status.total_ips, 0);

    let config = global.heartbeat_config.clone();
    assert_eq!(config.interval_secs, 30);
    assert_eq!(config.timeout_secs, 10);
}

#[tokio::test]
async fn test_entity_and_action_to_u32_combinations() {
    // Test all combinations using the to_u32 function
    let combinations = vec![
        (Entity::Node, Action::OnLine),
        (Entity::Node, Action::OnLineAck),
        (Entity::Node, Action::OffLine),
        (Entity::Node, Action::Ack),
        (Entity::Node, Action::Update),
        (Entity::Message, Action::SendText),
        (Entity::Message, Action::SendBinary),
        (Entity::Witness, Action::Tick),
        (Entity::Witness, Action::Check),
        (Entity::Telephone, Action::Call),
        (Entity::Telephone, Action::HangUp),
        (Entity::Telephone, Action::Accept),
        (Entity::Telephone, Action::Reject),
        (Entity::File, Action::SendText),
        (Entity::File, Action::SendBinary),
    ];

    for (entity, action) in combinations {
        let expected = P2PCommand::to_u32(entity, action);
        let cmd = P2PCommand::new(entity, action, vec![]);
        assert_eq!(cmd.id(), expected, "to_u32 and id() should match");
    }
}

#[tokio::test]
async fn test_p2p_command_validate() {
    let cmd = P2PCommand::new(Entity::Message, Action::SendText, b"test".to_vec());

    let is_valid = cmd.validate();
    assert!(is_valid, "Command should be valid by default");
}

#[tokio::test]
async fn test_command_is_trusted() {
    let cmd = P2PCommand::new(Entity::Message, Action::SendText, vec![]);

    let is_trusted = cmd.is_trusted();
    assert!(!is_trusted, "Default command should not be trusted");
}

#[tokio::test]
async fn test_online_command_all_fields() {
    let node = AexNode::from_system(9000, vec![0u8; 32], 1);
    let online_cmd = OnlineCommand {session_id:vec![1,2,3,4,5],node:node.clone(),ephemeral_public_key:[1u8;32], announced_ips: todo!(), seeds: todo!() };

    let encoded = Codec::encode(&online_cmd);
    let decoded: OnlineCommand = Codec::decode(&encoded).unwrap();

    assert_eq!(decoded.session_id, vec![1, 2, 3, 4, 5]);
    assert_eq!(decoded.node.port, 9000);
    assert_eq!(decoded.ephemeral_public_key, [1u8; 32]);
}
