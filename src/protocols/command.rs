use std::sync::Arc;

use aex::tcp::types::{Codec, Command};
use bincode::{Decode, Encode};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::Mutex;

use crate::context::Context;
use crate::protocols::frame::P2PFrame;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Hash, PartialEq, Eq, Encode, Decode)]
pub enum Entity {
    Node = 1,
    Message,
    Witness,
    Telephone,
    File,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Hash, PartialEq, Eq, Encode, Decode)]
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
    fn(P2PCommand, P2PFrame, Arc<Context>, Arc<Mutex<OwnedWriteHalf>> ) -> BoxFuture<'static, ()>;

#[derive(Clone, PartialEq, Serialize, Deserialize, Encode, Decode, Debug)]
pub struct P2PCommand {
    pub entity: Entity,
    pub action: Action,
    pub data: Vec<u8>,
}

impl Codec for P2PCommand {}

impl P2PCommand {
    pub fn new(entity: Entity, action: Action, data: Vec<u8>) -> Self {
        Self {
            entity,
            action,
            data
        }
    }

    pub fn to_u32(entity: Entity, action: Action) -> u32 {
        ((action as u32) << 8) | (entity as u32)
    }
}

impl Command for P2PCommand {
    fn id(&self) -> u32 {
        P2PCommand::to_u32(self.entity, self.action)
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
            Entity::Node,
            Action::OnLine,
            payload.clone(),
        );

        let bytes = Codec::encode(&cmd);
        let cmd: P2PCommand = Codec::decode(&bytes).unwrap();

        assert_eq!(cmd.entity, Entity::Node);
        assert_eq!(cmd.action, Action::OnLine);
        assert_eq!(cmd.data, payload);
    }
}
