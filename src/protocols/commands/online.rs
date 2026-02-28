use std::sync::Arc;

use aex::crypto::zero_trust_session_key::SessionKey;
use aex::tcp::types::Codec;
use bincode::{ Decode, Encode };
use futures::future::BoxFuture;
use serde::{ Deserialize, Serialize };

use crate::context::Context;
use crate::nodes::servers::Servers;
use crate::protocols::client_type::{ ClientType, send_bytes };
use crate::protocols::command::P2PCommand;
use crate::protocols::command::{ Action, Entity };
use crate::protocols::commands::ack::OnlineAckCommand;
use crate::protocols::frame::P2PFrame;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Encode, Decode)]
pub struct OnlineCommand {
    pub session_id: [u8; 16], // 临时 session id
    pub endpoints: Vec<u8>,
    pub ephemeral_public_key: [u8; 32],
}

// ⚡ 实现 CommandCodec，移除 to_bytes/from_bytes
impl Codec for OnlineCommand {}

pub async fn send_online(
    context: Arc<Context>,
    client_type: &ClientType,
    data: Vec<u8>
) -> anyhow::Result<()> {
    let command = P2PCommand::new(Entity::Node as u8, Action::OnLine as u8, data);

    let frame = P2PFrame::build(context, command, 1).await.unwrap();

    let bytes = Codec::encode(&frame);

    send_bytes(client_type, &bytes).await;

    Ok(())
}

pub fn on_online(
    cmd: P2PCommand,
    frame: P2PFrame,
    context: Arc<Context>,
    client_type: Arc<ClientType>
) -> BoxFuture<'static, ()> {
    Box::pin(async move {
        println!("✅ Node Online: addr={}, nonce={}", frame.body.address, frame.body.nonce);

        // ===== 1️⃣ OnlineCommand 解码 =====

        let online: OnlineCommand = match Codec::decode(&cmd.data) {
            Ok(cmd) => cmd,
            Err(e) => {
                eprintln!("❌ decode OnlineCommand failed: {e}");
                return;
            }
        };

        println!("received session_id: {:?}", online.session_id);

        // ===== 2️⃣ 服务器生成 SessionKey（包含 ephemeral keypair）并派生对称密钥 =====
        let mut session_key = SessionKey::new();
        let ephemeral_public = session_key.ephemeral_public.clone();
        let client_pub = x25519_dalek::PublicKey::from(online.ephemeral_public_key);

        if let Err(e) = session_key.establish(&client_pub) {
            eprintln!("❌ Failed to establish session key: {e}");
            return;
        } else {
            println!(
                "🔐 Session established with {} (session_id={:?})",
                frame.body.address,
                online.session_id
            );
        }
        session_key.touch();

        // 保存 session_key 到 session_keys，key 为客户端 address
        context.session_keys.lock().await.insert(frame.body.address.clone(), session_key);

        // ===== 3️⃣ 构造 OnlineAckCommand =====
        let ack = OnlineAckCommand {
            session_id: online.session_id,
            address: context.address.to_string(),
            ephemeral_public_key: ephemeral_public.to_bytes(),
        };

        println!("send ack session_id : {:?}", ack.session_id);
        println!("send ack: {:?}", Codec::encode(&ack));

        let command = P2PCommand::new(
            Entity::Node as u8,
            Action::OnLineAck as u8,
            Codec::encode(&ack)
        );

        let ack_frame = P2PFrame::build(context.clone(), command, 1).await.unwrap();

        // ===== 4️⃣ clients 登记 =====
        let (endpoints, is_inner) = match Servers::from_endpoints(online.endpoints) {
            (eps, flag) => (eps, flag == 0),
        };
        let addr = frame.body.address.clone();
        let mut clients = context.clients.lock().await;
        if is_inner {
            clients.add_inner(&addr, (*client_type).clone(), endpoints);
        } else {
            clients.add_external(&addr, (*client_type).clone(), endpoints);
        }

        // ===== 5️⃣ 发送 ACK =====
        send_bytes(&client_type, &Codec::encode(&ack_frame.clone())).await;
    })
}
