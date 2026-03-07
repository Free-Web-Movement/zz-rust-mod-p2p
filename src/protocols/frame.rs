use aex::{
    connection::context::{AexWriter, Context},
    crypto::session_key_manager::PairedSessionKey,
    tcp::types::{Codec, Frame},
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::{io::AsyncWriteExt, sync::Mutex};
use zz_account::address::FreeWebMovementAddress;

use crate::protocols::command::P2PCommand;
use crate::protocols::command::{Action, Entity};
use bincode::{Decode, Encode};

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
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
    // #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

impl Codec for FrameBody {}

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

    pub fn data_from_command(&mut self, cmd: &P2PCommand) -> anyhow::Result<()> {
        let bytes = Codec::encode(cmd);
        self.data = bytes;
        Ok(())
    }

    pub fn command_from_data(&self) -> anyhow::Result<P2PCommand> {
        let cmd: P2PCommand = Codec::decode(&self.data)?;
        Ok(cmd)
    }
}

/// 端到端安全帧（只做加密与校验）

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct P2PFrame {
    pub body: FrameBody,

    /// 对 body 的签名
    #[serde(with = "serde_bytes")]
    pub signature: Vec<u8>,
}

impl P2PFrame {
    pub fn new(body: FrameBody, signature: Vec<u8>) -> Self {
        P2PFrame { body, signature }
    }

    pub fn sign(body: FrameBody, signer: &FreeWebMovementAddress) -> anyhow::Result<Self> {
        // let bytes = encode_to_vec(&body, frame_config())?;
        let bytes = Codec::encode(&body);
        let signature = FreeWebMovementAddress::sign_message(&signer.private_key, &bytes)
            .serialize_compact()
            .to_vec();
        Ok(P2PFrame { body, signature })
    }

    pub fn verify_bytes(bytes: &Vec<u8>) -> anyhow::Result<P2PFrame> {
        let frame: P2PFrame = Codec::decode(&bytes)?;
        P2PFrame::verify(frame)
    }

    pub fn verify(frame: P2PFrame) -> anyhow::Result<P2PFrame> {
        let bytes = Codec::encode(&frame.body);
        let bytes = bytes.as_slice();

        let public_key = FreeWebMovementAddress::to_public_key(&frame.body.public_key);
        let signature = FreeWebMovementAddress::to_signature(&frame.signature);

        if !FreeWebMovementAddress::verify_message(&public_key, bytes, &signature) {
            return Err(anyhow::anyhow!("Frame signature verification failed"));
        }
        Ok(frame)
    }

    pub async fn build(
        address: &FreeWebMovementAddress,
        cmd: P2PCommand,
        version: u8,
    ) -> anyhow::Result<Self> {
        let cmd_bytes = Codec::encode(&cmd);
        let body = FrameBody {
            address: address.to_string(),
            public_key: address.public_key.to_bytes().to_vec(),
            nonce: rand::thread_rng().r#gen(),
            data_length: cmd_bytes.len() as u32,
            version,
            data: cmd_bytes,
        };
        Ok(P2PFrame::sign(body, &address)?)
    }
}

impl Codec for P2PFrame {}

impl Frame for P2PFrame {
    fn validate(&self) -> bool {
        let bytes = Codec::encode(&self.body);
        let bytes = bytes.as_slice();

        let public_key = FreeWebMovementAddress::to_public_key(&self.body.public_key);
        let signature = FreeWebMovementAddress::to_signature(&self.signature);

        if !FreeWebMovementAddress::verify_message(&public_key, bytes, &signature) {
            return false;
        }
        true
    }

    fn sign<F>(&self, signer: F) -> Vec<u8>
    where
        F: FnOnce(&[u8]) -> Vec<u8>,
    {
        let raw_bytes = Codec::encode(&self.body); // 假设 Codec 提供 encode()
        signer(&raw_bytes)
    }

    fn payload(&self) -> Option<Vec<u8>> {
        Some(Codec::encode(&self.body))
    }

    fn command(&self) -> Option<&Vec<u8>> {
        Some(self.body.data.as_ref())
    }
    fn is_flat(&self) -> bool {
        false
    }
}

impl P2PFrame {
    pub async fn send<C: Codec>(
        address: &FreeWebMovementAddress,
        writer: &mut AexWriter,
        sub_command: &Option<C>,
        entity: Entity,
        action: Action,
        paired_session_key: Option<Arc<Mutex<PairedSessionKey>>>,
    ) -> anyhow::Result<()> {
        let data = match sub_command {
            Some(cmd) => Codec::encode(cmd),
            None => vec![],
        };

        let bytes = match paired_session_key {
            Some(psk) => {
                let encode = psk.lock().await;
                encode
                    .encrypt(&address.to_string().as_bytes().to_vec(), &data)
                    .await?
            }
            None => data,
        };

        let command = P2PCommand::new(entity, action, bytes);

        let frame = P2PFrame::build(address, command, 1).await.unwrap();

        let bytes = Codec::encode(&frame);
        let len = bytes.len() as u32;
        if let Err(e) = writer.write_all(&len.to_be_bytes()).await {
            eprintln!("Failed to send TCP bytes: {:?}", e);
        }
        if let Err(e) = writer.write_all(&bytes).await {
            eprintln!("Failed to send TCP bytes: {:?}", e);
        }
        Ok(())
    }

    pub async fn send_bytes(writer: &mut AexWriter, bytes: &[u8]) {
        if let Err(e) = writer.write_all(&bytes).await {
            eprintln!("Failed to send TCP bytes: {:?}", e);
        }
    }
}

pub async fn notify(frame: &P2PFrame, ctx: Arc<Mutex<Context>>) {
    // ⚠️ 重要安全原则：
    // - 不解密
    // - 不反序列化 Command
    // - 不修改 Frame
    // - 只做字节级转发

    // ===== 1️⃣ 查本地 clients ====
    {
        {
            let manager = {
                let guard = ctx.lock().await;
                guard.global.manager.clone()
            };
            let bytes = Codec::encode(frame);
            manager
                .forward(|entries| async {
                    for entry in entries {
                        // 1. 先把临时值固定到一个变量名上，延长它的生命周期
                        let writer_arc = entry.writer.clone().unwrap();

                        // 2. 在这个长期变量上获取锁
                        let mut writer_guard = writer_arc.lock().await;

                        // 3. 现在你可以安全地解引用了
                        let guard = &mut *writer_guard;
                        P2PFrame::send_bytes(guard, &bytes).await
                    }
                })
                .await;
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::command::{Action, Entity};
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
        let frame = P2PFrame::sign(body.clone(), &identity)?;
        assert!(!frame.signature.is_empty(), "签名不应该为空");

        // 4️⃣ 序列化 Frame
        let serialized = Codec::encode(&frame);

        // 5️⃣ 验证签名
        let frame1 = P2PFrame::verify_bytes(&serialized)?;

        assert_eq!(frame.signature.to_vec(), frame1.signature.to_vec());

        println!("Frame verified successfully!");

        let bytes = Codec::encode(&frame);
        let frame2: P2PFrame = Codec::decode(&bytes).unwrap();

        assert_eq!(frame1.signature.to_vec(), frame2.signature.to_vec());
        assert_eq!(frame1.body.data.to_vec(), frame2.body.data.to_vec());

        Ok(())
    }

    fn make_command() -> P2PCommand {
        P2PCommand::new(Entity::Node, Action::OnLine, vec![1, 2, 3, 4])
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
            vec![0xaa],
        );

        let frame = P2PFrame::new(body.clone(), vec![0xbb]);

        assert_eq!(frame.body.version, body.version);
        assert_eq!(frame.signature, vec![0xbb]);
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

        let frame = P2PFrame::sign(body.clone(), &identity)?;
        assert!(!frame.signature.is_empty());

        let encoded = Codec::encode(&frame);
        let verified = P2PFrame::verify_bytes(&encoded)?;

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

        let frame = P2PFrame::sign(body, &identity).unwrap();

        let bytes = Codec::encode(&frame.clone());
        let decoded: P2PFrame = Codec::decode(&bytes).unwrap();

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

        let mut frame = P2PFrame::sign(body.clone(), &identity).unwrap();

        // 🔥 篡改数据
        body.data = vec![9, 9, 9];
        frame.body = body;
        let encoded = Codec::encode(&frame);

        let res = P2PFrame::verify_bytes(&encoded);
        assert!(res.is_err(), "篡改后的签名应验证失败");
    }

    #[test]
    fn test_frame_config_consistency() {
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

        let bytes = Codec::encode(&body);

        let decoded: FrameBody = Codec::decode(&bytes).unwrap();

        assert_eq!(decoded.version, 1);
    }
}
