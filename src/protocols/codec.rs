use anyhow::Result;
use bincode::{Decode, Encode, config, decode_from_slice};
use serde::{Deserialize, Serialize};

/// ⚡ 命令序列化 trait
pub trait Codec : Serialize + for<'de> Deserialize<'de> + Sized {
    /// 转为字节
    fn to_bytes(&self) -> Vec<u8>
    where
        Self: Encode,
    {
        bincode::encode_to_vec(self, bincode::config::standard())
            .expect("serialize should not fail")
    }

    /// 从字节恢复
    fn from_bytes(data: &[u8]) -> Result<Self>
    where
        Self: Decode<()>,
    {
        let (cmd, _): (Self, _) = decode_from_slice(data, config::standard())
            .map_err(|e| anyhow::anyhow!("decode command failed: {e}"))?;
        Ok(cmd)
    }
}
