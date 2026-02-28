use std::sync::Arc;

use aex::tcp::types::{Codec, Command};
use anyhow::{Ok, Result};
use bincode::{Decode, Encode};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};

use crate::context::Context;
use crate::protocols::client_type::{ClientType, send_bytes};
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

pub type CommandCallback =
    fn(P2PCommand, P2PFrame, Arc<Context>, Arc<ClientType>) -> BoxFuture<'static, ()>;

#[derive(Clone, PartialEq, Serialize, Deserialize, Encode, Decode, Debug)]
pub struct P2PCommand {
    pub entity: u8,
    pub action: u8,
    pub data: Vec<u8>,
}

impl Codec for P2PCommand {}

impl P2PCommand {
    pub fn new(entity: u8, action: u8, data: Vec<u8>) -> Self {
        Self {
            entity,
            action,
            data
        }
    }

    pub async fn send(
        &self,
        context: Arc<Context>,
        client_type: &ClientType,
        version: u8,
    ) -> Result<()> {
        let frame = P2PFrame::build(context, self.clone(), version).await?;
        let bytes = Codec::encode(&frame);
        send_bytes(client_type, &bytes).await;
        Ok(())
    }
}

impl Command for P2PCommand {
    fn id(&self) -> u32 {
        ((self.action as u32) << 8) | (self.entity as u32)
    }
    
    fn data(&self) -> &Vec<u8> {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_receive_roundtrip() {
        let payload = vec![10, 20, 30];

        let cmd = P2PCommand::new(
            Entity::Node as u8,
            Action::OnLine as u8,
            payload.clone()
        );

        let bytes = Codec::encode(&cmd);
        let cmd: P2PCommand = Codec::decode(&bytes).unwrap();

        assert_eq!(cmd.entity, Entity::Node as u8);
        assert_eq!(cmd.action, Action::OnLine as u8);
        assert_eq!(cmd.data, payload);
    }
}
