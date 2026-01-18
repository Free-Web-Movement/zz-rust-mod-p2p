use anyhow::Result;
use bincode::config;
use bincode::{Decode, Encode};

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

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct Command {
    pub entity: Entity,
    pub action: Action,
    pub data: Option<Vec<u8>>,
}

impl Command {
    pub fn new(entity: Entity, action: Action, data: Option<Vec<u8>>) -> Self {
        Self {
            entity,
            action,
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
    pub fn to_bytes(
        entity: Entity,
        action: Action,
        data: Option<Vec<u8>>,
    ) -> Result<Vec<u8>> {
        let cmd = Command::new(entity, action, data);
        cmd.serialize()
    }

    /// receive = decode from wire bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Command> {
        Command::deserialize(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_serialize_deserialize() {
        let cmd = Command::new(Entity::Node, Action::OnLine, Some(vec![1, 2, 3, 4]));
        let bytes = cmd.serialize().unwrap();
        let cmd2 = Command::deserialize(&bytes).unwrap();
        assert_eq!(cmd, cmd2);
    }

    #[test]
    fn test_send_receive_roundtrip() {
        let payload = vec![10, 20, 30];

        let bytes = Command::to_bytes(Entity::Node, Action::OnLine, Some(payload.clone())).unwrap();

        let cmd = Command::from_bytes(&bytes).unwrap();

        assert_eq!(cmd.entity, Entity::Node);
        assert_eq!(cmd.action, Action::OnLine);
        assert_eq!(cmd.data, Some(payload));
    }
}
