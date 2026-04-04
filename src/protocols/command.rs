use aex::tcp::types::{Codec, Command};
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

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
            data,
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
