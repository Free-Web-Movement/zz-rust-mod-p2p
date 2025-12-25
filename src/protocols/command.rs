use anyhow::Result;
use bincode::config;
use bincode::{Decode, Encode};


#[derive(Debug, Clone, Copy, PartialEq, Encode, Decode)]
pub enum Entity {
    Node = 1,
    Message,
    Telephone,
    File,
}

#[derive(Debug, Clone, Copy, PartialEq, Encode, Decode)]
pub enum NodeAction {
    OnLine = 1,
    OffLine,
    Update,
}

pub enum MessageAction {
    SendText = 1,
    SendBinary,
}

pub enum TelephoneAction {
    Call = 1,
    HangUp,
    Accept,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct Command {
    pub entity: Entity,
    pub action: NodeAction,
    pub version: u8,
    pub data: Option<Vec<u8>>,
}

impl Command {
    pub fn new(entity: Entity, action: NodeAction, version: u8, data: Option<Vec<u8>>) -> Self {
        Self {
            entity,
            action,
            version,
            data,
        }
    }

    fn config() -> impl bincode::config::Config {
        config::standard()
            .with_fixed_int_encoding()
            .with_big_endian()
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
    pub fn send(entity: Entity, action: NodeAction, version: u8, data: Option<Vec<u8>>) -> Result<Vec<u8>> {
        let cmd = Command::new(entity, action, version, data);
        cmd.serialize()
    }

    /// receive = decode from wire bytes
    pub fn receive(bytes: &[u8]) -> Result<Command> {
        Command::deserialize(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_serialize_deserialize() {
        let cmd = Command::new(Entity::Node, NodeAction::OnLine, 1, Some(vec![1, 2, 3, 4]));
        let bytes = cmd.serialize().unwrap();
        let cmd2 = Command::deserialize(&bytes).unwrap();
        assert_eq!(cmd, cmd2);
    }

    #[test]
    fn test_send_receive_roundtrip() {
        let payload = vec![10, 20, 30];

        let bytes = Command::send(
            Entity::Node,
            NodeAction::OnLine,
            1,
            Some(payload.clone()),
        )
        .unwrap();

        let cmd = Command::receive(&bytes).unwrap();

        assert_eq!(cmd.entity, Entity::Node);
        assert_eq!(cmd.action, NodeAction::OnLine);
        assert_eq!(cmd.version, 1);
        assert_eq!(cmd.data, Some(payload));
    }
}
