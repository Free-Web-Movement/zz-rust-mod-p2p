use rand::Rng;
use serde::{Deserialize, Serialize};
use zz_account::address::FreeWebMovementAddress;

use bincode::config;
use bincode::serde::{decode_from_slice, encode_to_vec};

use crate::protocols::command::{Command, Entity, NodeAction};

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
    pub address: String,

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
        address: String,
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

    pub fn verify_bytes(bytes: &Vec<u8>) -> anyhow::Result<Frame> {
        let (frame, _): (Frame, usize) = decode_from_slice(&bytes, frame_config())?;
        Frame::verify(frame)
    }

    pub fn verify(frame: Frame) -> anyhow::Result<Frame> {
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

    pub fn build_node_command(
        address: &FreeWebMovementAddress,
        entity: Entity,
        action: NodeAction,
        version: u8,
        data: Option<Vec<u8>>,
    ) -> anyhow::Result<Self> {
        let cmd_bytes = Command::send(entity, action, version, data)?;

        let body = FrameBody {
            address: address.to_string(),
            public_key: address.public_key.to_bytes().to_vec(),
            nonce: rand::thread_rng().r#gen(),
            data_length: cmd_bytes.len() as u32,
            version,
            data: cmd_bytes,
        };
        Ok(Frame::sign(body, address)?)
    }

    pub fn extract_node_command(bytes: &Vec<u8>) {
        // 1️⃣ 验证 Frame + 签名
        let frame = match Frame::verify_bytes(bytes) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("❌ Frame verify failed: {:?}", e);
                return;
            }
        };

        // 2️⃣ 解出 Command
        let cmd = match Command::receive(&frame.body.data) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("❌ Command decode failed: {:?}", e);
                return;
            }
        };

        // 3️⃣ 主分发框架（当前只处理 Node）
        match (cmd.entity as Entity, cmd.action as NodeAction) {
            (Entity::Node, NodeAction::OnLine) => {
                // TODO: Node 上线逻辑
                println!(
                    "✅ Node Online: addr={}, nonce={}",
                    frame.body.address, frame.body.nonce
                );
            }

            (Entity::Node, NodeAction::OffLine) => {
                // TODO: Node 下线逻辑
                println!(
                    "⚠️ Node Offline: addr={}, nonce={}",
                    frame.body.address, frame.body.nonce
                );
            }

            _ => {
                // 其他实体 / 动作暂不处理
                println!(
                    "ℹ️ Unsupported command: entity={:?}, action={:?}",
                    cmd.entity,
                    cmd.action
                );
            }
        }
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
            address: identity.to_string(),
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
        let frame1 = Frame::verify_bytes(&serialized)?;

        assert_eq!(frame.signature.to_vec(), frame1.signature.to_vec());

        println!("Frame verified successfully!");

        let bytes = Frame::to(frame);
        let frame2 = Frame::from(&bytes);

        assert_eq!(frame1.signature.to_vec(), frame2.signature.to_vec());

        Ok(())
    }

    fn make_command() -> Command {
        Command::new(
            Entity::Node,
            NodeAction::OnLine,
            1,
            Some(vec![1, 2, 3, 4]),
        )
    }

    #[test]
    fn test_frame_body_new() {
        let addr = FreeWebMovementAddress::random();

        let body = FrameBody::new(
            1,
            addr.to_string(),
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
        let mut body = FrameBody::new(
            1,
            addr.to_string(),
            addr.public_key.to_bytes(),
            1,
            0,
            vec![],
        );

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
            addr.to_string(),
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
            identity.to_string(),
            identity.public_key.to_bytes(),
            42,
            5,
            b"hello".to_vec(),
        );

        let frame = Frame::sign(body.clone(), &identity)?;
        assert!(!frame.signature.is_empty());

        let encoded = bincode::serde::encode_to_vec(&frame, frame_config())?;
        let verified = Frame::verify_bytes(&encoded)?;

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
            identity.to_string(),
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
            identity.to_string(),
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

        let res = Frame::verify_bytes(&encoded);
        assert!(res.is_err(), "篡改后的签名应验证失败");
    }

    #[test]
    fn test_frame_config_consistency() {
        let cfg1 = frame_config();
        let cfg2 = frame_config();

        // 只要能成功编码解码即视为一致
        let addr = FreeWebMovementAddress::random();

        let body = FrameBody::new(
            1,
            addr.to_string(),
            addr.public_key.to_bytes(),
            0,
            0,
            vec![],
        );

        let bytes = encode_to_vec(&body, cfg1).unwrap();
        let (decoded, _): (FrameBody, usize) = decode_from_slice(&bytes, cfg2).unwrap();

        assert_eq!(decoded.version, 1);
    }

    #[test]
    fn test_build_node_command_online() -> anyhow::Result<()> {
        // 1️⃣ 构造测试地址
        let address = FreeWebMovementAddress::random();

        // 2️⃣ 构造业务数据
        let payload = Some(b"hello node online".to_vec());

        // 3️⃣ 构建 Frame
        let frame = Frame::build_node_command(
            &address,
            Entity::Node,
            NodeAction::OnLine,
            1,
            payload.clone(),
        )?;

        // 4️⃣ 基本结构校验
        assert_eq!(frame.body.version, 1);
        assert_eq!(frame.body.address, address.to_string());
        assert_eq!(
            frame.body.public_key,
            address.public_key.to_bytes().to_vec()
        );

        // nonce 应该存在（不为 0 不是强约束，但通常如此）
        assert!(frame.body.nonce > 0);

        // data 校验
        let cmd_bytes = Command::send(Entity::Node, NodeAction::OnLine, 1, payload)?;

        assert_eq!(frame.body.data_length, cmd_bytes.len() as u32);
        assert_eq!(frame.body.data, cmd_bytes);

        // 5️⃣ 签名存在
        assert!(!frame.signature.is_empty());

        // 6️⃣ 🔐 核心：签名校验（防 MITM）
        Frame::verify(frame)?;
        Ok(())
    }

    #[test]
    fn test_build_node_command_without_data() -> anyhow::Result<()> {
        let address = FreeWebMovementAddress::random();

        let frame =
            Frame::build_node_command(&address, Entity::Node, NodeAction::OffLine, 1, None)?;

        assert_eq!(frame.body.address, address.to_string());
        assert_eq!(frame.body.version, 1);
        assert!(frame.body.data_length > 0);
        assert!(!frame.body.data.is_empty());

        // 签名校验必须通过
        Frame::verify(frame)?;

        Ok(())
    }

    #[test]
    fn test_extract_node_command_online() -> anyhow::Result<()> {
        let address = FreeWebMovementAddress::random();

        let frame = Frame::build_node_command(
            &address,
            Entity::Node,
            NodeAction::OnLine,
            1,
            Some(b"online".to_vec()),
        )?;

        let bytes = Frame::to(frame);

        // 不应 panic
        Frame::extract_node_command(&bytes);

        Ok(())
    }

    #[test]
    fn test_extract_node_command_offline() -> anyhow::Result<()> {
        let address = FreeWebMovementAddress::random();

        let frame =
            Frame::build_node_command(&address, Entity::Node, NodeAction::OffLine, 1, None)?;

        let bytes = Frame::to(frame);

        // 不应 panic
        Frame::extract_node_command(&bytes);

        Ok(())
    }

    #[test]
    fn test_extract_node_command_with_tampered_frame_should_not_panic() {
        let address = FreeWebMovementAddress::random();

        let mut frame =
            Frame::build_node_command(&address, Entity::Node, NodeAction::OnLine, 1, None).unwrap();

        // 🔥 篡改数据，制造非法 frame
        frame.body.data = vec![0xFF, 0xEE, 0xDD];

        let bytes = Frame::to(frame);

        // 即使非法，也不能 panic
        Frame::extract_node_command(&bytes);
    }
}
