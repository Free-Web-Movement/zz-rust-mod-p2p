use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use anyhow::{Result, anyhow};

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct OnlineAckCommand {
    pub session_id: [u8; 16],          // 临时 session id
    pub address: String,               // ⚠️ 明确：String
    pub ephemeral_public_key: [u8; 32], // 对方 ephemeral 公钥
}

impl OnlineAckCommand {
    pub fn to_bytes(&self) -> Vec<u8> {
        let addr_bytes = self.address.as_bytes();
        let addr_len = addr_bytes.len() as u16;

        let mut buf = Vec::with_capacity(
            16 + 2 + addr_bytes.len() + 32
        );

        // session_id
        buf.extend_from_slice(&self.session_id);

        // address length
        buf.extend_from_slice(&addr_len.to_be_bytes());

        // address utf-8 bytes
        buf.extend_from_slice(addr_bytes);

        // ephemeral public key
        buf.extend_from_slice(&self.ephemeral_public_key);

        buf
    }
}

impl OnlineAckCommand {
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        // 最小长度
        if data.len() < 16 + 2 + 32 {
            return Err(anyhow!("online ack command too short"));
        }

        let mut offset = 0;

        // session_id
        let session_id: [u8; 16] = data[offset..offset + 16]
            .try_into()
            .map_err(|_| anyhow!("invalid session_id"))?;
        offset += 16;

        // address length
        let addr_len = u16::from_be_bytes(
            data[offset..offset + 2]
                .try_into()
                .unwrap()
        ) as usize;
        offset += 2;

        // 边界检查
        if data.len() < offset + addr_len + 32 {
            return Err(anyhow!("invalid address length"));
        }

        // address
        let address = std::str::from_utf8(
            &data[offset..offset + addr_len]
        )
        .map_err(|_| anyhow!("address is not valid utf-8"))?
        .to_string();
        offset += addr_len;

        // ephemeral public key
        let ephemeral_public_key: [u8; 32] = data[offset..offset + 32]
            .try_into()
            .map_err(|_| anyhow!("invalid ephemeral public key"))?;

        Ok(Self {
            session_id,
            address,
            ephemeral_public_key,
        })
    }
}



#[cfg(test)]
mod tests {
    use crate::protocols::commands::online::OnlineCommand;

    use super::*;
    use rand::{rngs::OsRng, RngCore};

    fn rand_session_id() -> [u8; 16] {
        let mut id = [0u8; 16];
        OsRng.fill_bytes(&mut id);
        id
    }

    fn rand_pubkey() -> [u8; 32] {
        let mut k = [0u8; 32];
        OsRng.fill_bytes(&mut k);
        k
    }

    // ---------- OnlineCommand ----------

    #[test]
    fn test_online_command_roundtrip() {
        let cmd = OnlineCommand {
            session_id: rand_session_id(),
            endpoints: vec![1, 2, 3, 4],
            ephemeral_public_key: rand_pubkey(),
        };

        let bytes = cmd.to_bytes();
        let decoded = OnlineCommand::from_bytes(&bytes).unwrap();

        assert_eq!(cmd.session_id, decoded.session_id);
        assert_eq!(cmd.endpoints, decoded.endpoints);
        assert_eq!(cmd.ephemeral_public_key, decoded.ephemeral_public_key);
    }

    #[test]
    fn test_online_command_too_short() {
        let err = OnlineCommand::from_bytes(&[0u8; 10]).unwrap_err();
        assert!(err.to_string().contains("too short"));
    }

    #[test]
    fn test_online_command_bad_endpoint_len() {
        let mut data = vec![0u8; 16];
        data.extend_from_slice(&10u16.to_be_bytes()); // ep_len = 10
        data.extend_from_slice(&[1, 2]);              // 实际只有 2
        data.extend_from_slice(&[0u8; 32]);

        let err = OnlineCommand::from_bytes(&data).unwrap_err();
        assert!(err.to_string().contains("invalid endpoints"));
    }

    // ---------- OnlineAckCommand ----------

    #[test]
    fn test_online_ack_roundtrip() {
        let cmd = OnlineAckCommand {
            session_id: rand_session_id(),
            address: "peer-address-123".to_string(),
            ephemeral_public_key: rand_pubkey(),
        };

        let bytes = cmd.to_bytes();
        let decoded = OnlineAckCommand::from_bytes(&bytes).unwrap();

        assert_eq!(cmd.session_id, decoded.session_id);
        assert_eq!(cmd.address, decoded.address);
        assert_eq!(cmd.ephemeral_public_key, decoded.ephemeral_public_key);
    }

    #[test]
    fn test_online_ack_too_short() {
        let err = OnlineAckCommand::from_bytes(&[0u8; 8]).unwrap_err();
        assert!(err.to_string().contains("too short"));
    }

    #[test]
    fn test_online_ack_invalid_address_len() {
        let mut data = vec![0u8; 16];
        data.extend_from_slice(&5u16.to_be_bytes()); // addr_len = 5
        data.extend_from_slice(&[1, 2]);             // 实际 2
        data.extend_from_slice(&[0u8; 32]);

        let err = OnlineAckCommand::from_bytes(&data).unwrap_err();
        assert!(err.to_string().contains("invalid address"));
    }

    #[test]
    fn test_online_ack_non_utf8_address() {
        let mut data = vec![0u8; 16];
        data.extend_from_slice(&2u16.to_be_bytes());
        data.extend_from_slice(&[0xff, 0xff]); // 非 UTF-8
        data.extend_from_slice(&[0u8; 32]);

        let err = OnlineAckCommand::from_bytes(&data).unwrap_err();
        assert!(err.to_string().contains("utf-8"));
    }
}
