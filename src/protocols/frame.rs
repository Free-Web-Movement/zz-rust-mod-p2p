use serde::{Deserialize, Serialize};
use zz_account::address::FreeWebMovementAddress;

use bincode::config;
use bincode::serde::{decode_from_slice, encode_to_vec};

use crate::protocols::command::Command;

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

impl FrameBody {
    pub fn new(
        version: u8,
        address: FreeWebMovementAddress,
        public_key: Vec<u8>,
        nonce: u64,
        data_length: u32,
        data: Vec<u8>,
    ) -> Self {
        FrameBody {
            version,
            address,
            public_key,
            nonce,
            data_length,
            data,
        }
    }

    pub fn data_from_command(&mut self, cmd: &Command) -> anyhow::Result<()> {
        let bytes = cmd.serialize()?;
        self.data = bytes;
        Ok(())
    }

    pub fn command_from_data(&self) -> anyhow::Result<Command> {
        let cmd = Command::deserialize(&self.data)?;
        Ok(cmd)
    }
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
    pub fn new(body: FrameBody, signature: Vec<u8>) -> Self {
        Frame { body, signature }
    }

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
        let vecs = encode_to_vec(&frame.body, config)?;
        let bytes = vecs.as_slice();

        let public_key = FreeWebMovementAddress::to_public_key(&frame.body.public_key);
        let signature = FreeWebMovementAddress::to_signature(&frame.signature);

        if !FreeWebMovementAddress::verify_message(&public_key, bytes, &signature) {
            return Err(anyhow::anyhow!("Frame signature verification failed"));
        }

        Ok(frame)
    }

    pub fn from(bytes: &Vec<u8>) -> Frame {
        let (frame, _): (Frame, usize) = decode_from_slice(&bytes, frame_config()).unwrap();
        frame
    }

    pub fn to(frame: Frame) -> Vec<u8> {
        encode_to_vec(&frame, frame_config()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::command::{Entity, NodeAction};
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

        let bytes = Frame::to(frame);
        let frame2 = Frame::from(&bytes);

        assert_eq!(frame1.signature.to_vec(), frame2.signature.to_vec());

        Ok(())
    }

    fn make_command() -> Command {
        Command::new(
            Entity::Node as u8,
            NodeAction::OnLine as u8,
            1,
            Some(vec![1, 2, 3, 4]),
        )
    }

    #[test]
    fn test_frame_body_new() {
        let addr = FreeWebMovementAddress::random();

        let body = FrameBody::new(
            1,
            addr.clone(),
            addr.public_key.to_bytes(),
            100,
            4,
            vec![9, 8, 7, 6],
        );

        assert_eq!(body.version, 1);
        assert_eq!(body.address.to_string(), addr.to_string());
        assert_eq!(body.nonce, 100);
        assert_eq!(body.data_length, 4);
        assert_eq!(body.data, vec![9, 8, 7, 6]);
    }

    #[test]
    fn test_frame_body_data_from_command_and_back() -> anyhow::Result<()> {
        let addr = FreeWebMovementAddress::random();
        let mut body = FrameBody::new(1, addr.clone(), addr.public_key.to_bytes(), 1, 0, vec![]);

        let cmd = make_command();
        body.data_from_command(&cmd)?;

        assert!(!body.data.is_empty());

        let decoded = body.command_from_data()?;
        assert_eq!(decoded, cmd);

        Ok(())
    }

    #[test]
    fn test_frame_new() {
        let addr = FreeWebMovementAddress::random();

        let body = FrameBody::new(
            1,
            addr.clone(),
            addr.public_key.to_bytes(),
            1,
            1,
            vec![0xAA],
        );

        let frame = Frame::new(body.clone(), vec![0xBB]);

        assert_eq!(frame.body.version, body.version);
        assert_eq!(frame.signature, vec![0xBB]);
    }

    #[tokio::test]
    async fn test_frame_sign_verify_roundtrip() -> anyhow::Result<()> {
        let identity = FreeWebMovementAddress::random();

        let body = FrameBody::new(
            1,
            identity.clone(),
            identity.public_key.to_bytes(),
            42,
            5,
            b"hello".to_vec(),
        );

        let frame = Frame::sign(body.clone(), &identity)?;
        assert!(!frame.signature.is_empty());

        let encoded = bincode::serde::encode_to_vec(&frame, frame_config())?;
        let verified = Frame::verify(&encoded).await?;

        assert_eq!(frame.signature, verified.signature);
        assert_eq!(
            frame.body.address.to_string(),
            verified.body.address.to_string()
        );

        Ok(())
    }

    #[test]
    fn test_frame_to_from() {
        let identity = FreeWebMovementAddress::random();

        let body = FrameBody::new(
            1,
            identity.clone(),
            identity.public_key.to_bytes(),
            7,
            3,
            vec![1, 2, 3],
        );

        let frame = Frame::sign(body, &identity).unwrap();

        let bytes = Frame::to(frame.clone());
        let decoded = Frame::from(&bytes);

        assert_eq!(frame.signature, decoded.signature);
        assert_eq!(frame.body.nonce, decoded.body.nonce);
    }

    #[tokio::test]
    async fn test_frame_verify_with_tampered_signature_should_fail() {
        let identity = FreeWebMovementAddress::random();

        let mut body = FrameBody::new(
            1,
            identity.clone(),
            identity.public_key.to_bytes(),
            9,
            3,
            vec![1, 2, 3],
        );

        let mut frame = Frame::sign(body.clone(), &identity).unwrap();

        // 🔥 篡改数据
        body.data = vec![9, 9, 9];
        frame.body = body;

        let encoded = bincode::serde::encode_to_vec(&frame, frame_config()).unwrap();

        let res = Frame::verify(&encoded).await;
        assert!(res.is_err(), "篡改后的签名应验证失败");
    }

    #[test]
    fn test_frame_config_consistency() {
        let cfg1 = frame_config();
        let cfg2 = frame_config();

        // 只要能成功编码解码即视为一致
        let addr = FreeWebMovementAddress::random();

        let body = FrameBody::new(1, addr.clone(), addr.public_key.to_bytes(), 0, 0, vec![]);

        let bytes = encode_to_vec(&body, cfg1).unwrap();
        let (decoded, _): (FrameBody, usize) = decode_from_slice(&bytes, cfg2).unwrap();

        assert_eq!(decoded.version, 1);
    }
}
