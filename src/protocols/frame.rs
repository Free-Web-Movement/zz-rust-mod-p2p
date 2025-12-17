use serde::{Deserialize, Serialize};
use zz_account::address::FreeWebMovementAddress;

use bincode::config;
use bincode::serde::{decode_from_slice, encode_to_vec};

use crate::protocols::defines::ProtocolType;

/// ⚠️ 不要写返回类型！
#[inline]
pub fn frame_config() -> impl bincode::config::Config {
    config::standard()
        .with_fixed_int_encoding()
        .with_big_endian()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameBody {
    /// 协议版本
    pub version: u8,

    /// 发送方地址（身份）
    pub address: FreeWebMovementAddress,

    /// 发送方公钥
    pub public_key: Vec<u8>,

    /// 防重放随机数
    pub nonce: u64,

    /// 明文长度
    pub data_length: u32,

    /// ⚠️ 加密后的数据（唯一承载业务的地方）
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

/// 端到端安全帧（只做加密与校验）

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    pub body: FrameBody,

    /// 对 body 的签名
    #[serde(with = "serde_bytes")]
    pub signature: Vec<u8>,
}

impl Frame {
    pub fn sign(body: FrameBody, signer: &FreeWebMovementAddress) -> anyhow::Result<Self> {
        let bytes = encode_to_vec(&body, frame_config())?;
        let signature = FreeWebMovementAddress::sign_message(&signer.private_key, &bytes)
            .serialize_compact()
            .to_vec();
        Ok(Frame { body, signature })
    }
    pub async fn verify(bytes: &Vec<u8>) -> anyhow::Result<Frame> {
        let (frame, _): (Frame, usize) = decode_from_slice(&bytes, frame_config())?;
        let config = frame_config();
        let vecs = encode_to_vec(&frame.body, config).unwrap();
        let bytes = vecs.as_slice();
        let public_key = FreeWebMovementAddress::to_public_key(&frame.body.public_key);
        let signature = FreeWebMovementAddress::to_signature(&frame.signature);
        FreeWebMovementAddress::verify_message(&public_key, bytes, &signature);
        Ok(frame)
    }

    pub async fn read(bytes: &Vec<u8>) -> Frame {
        let (frame, _): (Frame, usize) = decode_from_slice(&bytes, frame_config()).unwrap();
        frame
    }

    pub async fn write(t: ProtocolType) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zz_account::address::FreeWebMovementAddress;

    #[tokio::test]
    async fn test_frame_sign_and_verify() -> anyhow::Result<()> {
        // 1️⃣ 创建随机身份
        let identity = FreeWebMovementAddress::random();

        // 2️⃣ 构造 frame body
        let body = FrameBody {
            version: 1,
            address: identity.clone(),
            public_key: identity.public_key.to_bytes(),
            nonce: 42,
            data_length: 5,
            data: b"hello".to_vec(),
        };

        // 3️⃣ 使用身份签名生成 Frame
        let frame = Frame::sign(body.clone(), &identity)?;
        assert!(!frame.signature.is_empty(), "签名不应该为空");

        // 4️⃣ 序列化 Frame
        let serialized = bincode::serde::encode_to_vec(&frame, frame_config())?;

        // 5️⃣ 验证签名
        let frame1 = Frame::verify(&serialized).await?;

        assert_eq!(frame.signature.to_vec(), frame1.signature.to_vec());

        println!("Frame verified successfully!");

        Ok(())
    }
}
