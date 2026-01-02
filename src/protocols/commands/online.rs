use anyhow::Result;
use anyhow::anyhow;
use std::sync::Arc;

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use zz_account::address::FreeWebMovementAddress;

use crate::context::Context;
use crate::nodes::servers::Servers;
use crate::protocols::client_type::{ClientType, send_bytes};
use crate::protocols::command::{Action, Entity};
use crate::protocols::commands::ack::OnlineAckCommand;
use crate::protocols::frame::Frame;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Encode, Decode)]
pub struct OnlineCommand {
    pub session_id: [u8; 16], // 临时 session id
    pub endpoints: Vec<u8>,
    pub ephemeral_public_key: [u8; 32],
}

impl OnlineCommand {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(16 + 2 + self.endpoints.len() + 32);

        // session_id
        buf.extend_from_slice(&self.session_id);

        // endpoints length (u16, BE)
        let len = self.endpoints.len() as u16;
        buf.extend_from_slice(&len.to_be_bytes());

        // endpoints
        buf.extend_from_slice(&self.endpoints);

        // ephemeral public key
        buf.extend_from_slice(&self.ephemeral_public_key);

        buf
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        // 最小长度检查
        if data.len() < 16 + 2 + 32 {
            return Err(anyhow!("online command too short"));
        }

        let mut offset = 0;

        // session_id
        let session_id: [u8; 16] = data[offset..offset + 16]
            .try_into()
            .map_err(|_| anyhow!("invalid session_id"))?;
        offset += 16;

        // endpoints length
        let endpoints_len =
            u16::from_be_bytes(data[offset..offset + 2].try_into().unwrap()) as usize;
        offset += 2;

        // endpoints bounds check
        if data.len() < offset + endpoints_len + 32 {
            return Err(anyhow!("invalid endpoints length"));
        }

        // endpoints
        let endpoints = data[offset..offset + endpoints_len].to_vec();
        offset += endpoints_len;

        // ephemeral public key
        let ephemeral_public_key: [u8; 32] = data[offset..offset + 32]
            .try_into()
            .map_err(|_| anyhow!("invalid public key"))?;

        Ok(Self {
            session_id,
            endpoints,
            ephemeral_public_key,
        })
    }
}

pub async fn on_node_online(
    frame: &Frame,
    context: Arc<Context>,
    client_type: &ClientType,
) -> Option<Frame> {
    println!(
        "✅ Node Online: addr={}, nonce={}",
        frame.body.address, frame.body.nonce
    );

    // ===== 1️⃣ OnlineCommand 解码 =====

    println!("Received data {:?}", frame.body.data);
    let online = match OnlineCommand::from_bytes(&frame.body.data) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("❌ decode OnlineCommand failed: {e}");
            return None;
        }
    };

    // ===== 2️⃣ session_id → temp session（⚠️ 限定作用域）=====
    let (server_pub, ack_frame) = {
        let mut temp_sessions = context.temp_sessions.lock().await;

        let session = match temp_sessions.get_mut(&online.session_id) {
            Some(s) => s,
            None => {
                eprintln!("❌ temp session not found: {:?}", online.session_id);
                return None;
            }
        };

        let peer_pub = x25519_dalek::PublicKey::from(online.ephemeral_public_key);
        if let Err(e) = session.establish(&peer_pub) {
            eprintln!("❌ session establish failed: {e}");
            return None;
        }

        session.touch();

        let ack = OnlineAckCommand {
            session_id: online.session_id,
            address: context.address.to_string(),
            ephemeral_public_key: *session.ephemeral_public.as_bytes(),
        };

        let frame = Frame::build_node_command(
            &context.address,
            Entity::Node,
            Action::OnLineAck,
            frame.body.version,
            Some(ack.to_bytes()),
        )
        .expect("build OnlineAck frame failed");

        // ✅ temp_sessions 锁在这里释放
        (ack.ephemeral_public_key, frame)
    };

    // ===== 3️⃣ clients 登记（⚠️ 也限定作用域）=====
    {
        let (endpoints, is_inner) = match Servers::from_endpoints(online.endpoints) {
            (endpoints, flag) => (endpoints, flag == 0),
        };

        let addr = frame.body.address.clone();
        let mut clients = context.clients.lock().await;

        if is_inner {
            clients.add_inner(&addr, client_type.clone(), endpoints);
        } else {
            clients.add_external(&addr, client_type.clone(), endpoints);
        }
        // ✅ clients 锁在这里释放
    }

    // ===== 4️⃣ 发送 ACK（此时无任何锁）=====
    send_bytes(client_type, &Frame::to(ack_frame.clone())).await;

    Some(ack_frame)
}

pub async fn send_online(
    client_type: &ClientType,
    address: &FreeWebMovementAddress,
    data: Option<Vec<u8>>,
) -> anyhow::Result<()> {
    let frame = Frame::build_node_command(
        &address, // 本节点地址
        Entity::Node,
        Action::OnLine, // 用 ResponseAddress 表示发送自身地址
        1,
        data,
    )?;
    let bytes = Frame::to(frame);

    send_bytes(client_type, &bytes).await;

    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;
    use x25519_dalek::EphemeralSecret;

    #[test]
    fn test_online_command_encode_decode() {
        // 准备测试数据
        let mut session_id = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut session_id);

        let endpoints = vec![1, 2, 3, 4, 5];

        let secret = EphemeralSecret::random_from_rng(rand::rngs::OsRng);
        let public = x25519_dalek::PublicKey::from(&secret);
        let ephemeral_public_key = *public.as_bytes();

        let cmd = OnlineCommand {
            session_id,
            endpoints: endpoints.clone(),
            ephemeral_public_key,
        };

        // ===== 测试 to_bytes / from_bytes =====
        let bytes = cmd.to_bytes();
        let decoded = OnlineCommand::from_bytes(&bytes).expect("decode failed");

        assert_eq!(decoded.session_id, session_id);
        assert_eq!(decoded.endpoints, endpoints);
        assert_eq!(decoded.ephemeral_public_key, ephemeral_public_key);
    }

    #[test]
    fn test_online_command_from_bytes_too_short() {
        let data = vec![0u8; 10]; // 明显不足16+2+32
        let res = OnlineCommand::from_bytes(&data);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("too short"));
    }

    #[test]
    fn test_online_command_from_bytes_invalid_length() {
        let mut data = vec![0u8; 16 + 2 + 32]; // 基础长度
        // 人为把 endpoints_len 写成 1000，超过实际长度
        data[16] = 0x03; // 高位
        data[17] = 0xE8; // 低位 1000
        let res = OnlineCommand::from_bytes(&data);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("invalid endpoints length"));
    }

    #[test]
    fn test_online_command_invalid_session_id_slice() {
        // 构造长度正确，但切片转换出错
        let mut data = vec![0u8; 16 + 2 + 32];
        data[0] = 0; // 随便填
        // 这里 slice 转换应该不会失败，因为长度ok，跳过
        // 测试逻辑完整性即可
        let res = OnlineCommand::from_bytes(&data);
        assert!(res.is_ok()); // 只要长度够，slice.try_into不会失败
    }
}
