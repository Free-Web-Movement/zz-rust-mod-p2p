use std::sync::Arc;

use aex::tcp::types::Codec;
use anyhow::Result;
use bincode::{ Decode, Encode, config };
use futures::future::BoxFuture;

use crate::context::Context;
use crate::protocols::client_type::{ ClientType, send_bytes };
use crate::protocols::frame::P2PFrame;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Encode, Decode)]
pub enum Entity {
    Node = 1,
    Message,
    Witness,
    Telephone,
    File,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Encode, Decode)]
pub enum Action {
    //Node Actions
    OnLine = 1,
    OnLineAck,
    OffLine,
    Ack,
    Update,

    //Message Actions
    SendText,
    SendBinary,

    //Witness Actions
    Tick,
    Check,

    //Telephone Actions
    Call,
    HangUp,
    Accept,
    Reject,
}


pub type CommandCallback = fn(P2PCommand, P2PFrame, Arc<Context>, Arc<ClientType>) -> BoxFuture<'static, ()>;

#[derive(Clone, PartialEq, Encode, Decode, Debug)]
pub struct P2PCommand {
    pub entity: u8,
    pub action: u8,
    pub data: Option<Vec<u8>>
}

impl P2PCommand {
    pub fn new(entity: u8, action: u8, data: Option<Vec<u8>>) -> Self {
        Self {
            entity,
            action,
            data,
        }
    }

    fn config() -> impl bincode::config::Config {
        config::standard().with_fixed_int_encoding().with_big_endian()
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        Ok(bincode::encode_to_vec(self, Self::config())?)
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        let (cmd, _len): (Self, usize) = bincode::decode_from_slice(bytes, Self::config())?;
        Ok(cmd)
    }
    /* =========================
       Protocol helpers
    ========================= */

    /// send = build + encode (protocol layer, no IO)
    pub fn to_bytes(entity: Entity, action: Action, data: Option<Vec<u8>>) -> Result<Vec<u8>> {
        let cmd = P2PCommand::new(entity as u8, action as u8, data);
        cmd.serialize()
    }

    /// receive = decode from wire bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<P2PCommand> {
        P2PCommand::deserialize(bytes)
    }

    pub async fn send(
        &self,
        context: Arc<Context>,
        client_type: &ClientType,
        version: u8
    ) -> Result<()> {
        let frame = P2PFrame::build(context, self.clone(), version).await?;
        let bytes = Codec::encode(&frame);
        send_bytes(client_type, &bytes).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_serialize_deserialize() {
        let cmd = P2PCommand::new(Entity::Node as u8, Action::OnLine as u8, Some(vec![1, 2, 3, 4]));
        let bytes = cmd.serialize().unwrap();
        let cmd2 = P2PCommand::deserialize(&bytes).unwrap();
        assert_eq!(cmd, cmd2);
    }

    #[test]
    fn test_send_receive_roundtrip() {
        let payload = vec![10, 20, 30];

        let bytes = P2PCommand::to_bytes(Entity::Node, Action::OnLine, Some(payload.clone())).unwrap();

        let cmd = P2PCommand::from_bytes(&bytes).unwrap();

        assert_eq!(cmd.entity, Entity::Node as u8);
        assert_eq!(cmd.action, Action::OnLine as u8);
        assert_eq!(cmd.data, Some(payload));
    }
}
