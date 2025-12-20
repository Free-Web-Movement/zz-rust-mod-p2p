use anyhow::Result;
use bincode::{Encode, Decode};
use bincode::config;

pub enum Entity {
    Node = 1,
    Message,
    Telephone,
    File,
}

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
    pub entity: u8,
    pub action: u8,
    pub version: u16,
    pub length: u32,
    pub data: Option<Vec<u8>>,
}

impl Command {
    pub fn new(
        entity: u8,
        action: u8,
        version: u16,
        data: Option<Vec<u8>>,
    ) -> Self {
        let length = data.as_ref().map(|d| d.len() as u32).unwrap_or(0);
        Self {
            entity,
            action,
            version,
            length,
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
        let (cmd, _len): (Self, usize) =
            bincode::decode_from_slice(bytes, Self::config())?;
        Ok(cmd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_serialize_deserialize() {
        let cmd = Command::new(1, 2, 1, Some(vec![1, 2, 3, 4]));
        let bytes = cmd.serialize().unwrap();
        let cmd2 = Command::deserialize(&bytes).unwrap();
        assert_eq!(cmd, cmd2);
    }
}
