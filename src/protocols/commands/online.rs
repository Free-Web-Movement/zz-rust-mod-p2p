use std::sync::Arc;
use anyhow::Result;
use anyhow::anyhow;

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use zz_account::address::FreeWebMovementAddress;

use crate::context::Context;
use crate::nodes::servers::Servers;
use crate::protocols::client_type::{ClientType, send_bytes};
use crate::protocols::command::{Action, Entity};
use crate::protocols::frame::Frame;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Encode, Decode)]
pub struct OnlineCommand {
    pub session_id: [u8; 16], // 临时 session id
    pub endpoints: Vec<u8>,
    pub ephemeral_public_key: [u8; 32],
}


impl OnlineCommand {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(
            16 + 2 + self.endpoints.len() + 32
        );

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
        let endpoints_len = u16::from_be_bytes(
            data[offset..offset + 2].try_into().unwrap()
        ) as usize;
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


pub async fn on_node_online(frame: &Frame, context: Arc<Context>, client_type: &ClientType) {
    println!(
        "✅ Node Online: addr={}, nonce={}",
        frame.body.address, frame.body.nonce
    );
    if frame.body.data.len() < 1 {
        eprintln!("❌ Online data too short");
        return;
    }

    let (endpoints, is_inner) = match Servers::from_endpoints(frame.body.data.to_vec()) {
        (endpoints, flag) => (endpoints, flag == 0),
    };

    let addr = frame.body.address.clone();
    let mut clients = context.clients.lock().await;

    if is_inner {
        clients.add_inner(&addr, client_type.clone(), endpoints.clone());
    } else {
        clients.add_external(&addr, client_type.clone(), endpoints.clone());
    }
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