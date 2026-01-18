use std::sync::Arc;

use anyhow::Result;
use rand::Rng;
use serde::{ Deserialize, Serialize };
use zz_account::address::FreeWebMovementAddress;

use bincode::{ Decode, Encode, config };
use bincode::serde::{ decode_from_slice, encode_to_vec };

use crate::context::Context;
use crate::protocols::client_type::{ ClientType, send_bytes };
use crate::protocols::codec::Codec;
use crate::protocols::command::{ Action, Command, Entity };
use crate::protocols::commands::message::on_text_message;
use crate::protocols::commands::offline::on_node_offline;
use crate::protocols::registry::frame_handler_registry;

/// ⚠️ 不要写返回类型！
#[inline]
pub fn frame_config() -> impl bincode::config::Config {
    config::standard().with_fixed_int_encoding().with_big_endian()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CryptoState {
    /// 明文（仅用于 Online / Offline / KeyExchange）
    Plain,

    /// 使用地址绑定的临时会话密钥
    Encrypted {
        nonce: [u8; 12], // AEAD nonce
    },
}

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

impl FrameBody {
    pub fn new(
        version: u8,
        address: String,
        public_key: Vec<u8>,
        nonce: u64,
        data_length: u32,
        data: Vec<u8>
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

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
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
    
    pub async fn build(context: Arc<Context>, cmd: Command, version: u8) -> anyhow::Result<Self> {
        let cmd_bytes = cmd.serialize().unwrap();
        let body = FrameBody {
            address: context.address.to_string(),
            public_key: context.address.public_key.to_bytes().to_vec(),
            nonce: rand::thread_rng().r#gen(),
            data_length: cmd_bytes.len() as u32,
            version,
            data: cmd_bytes,
        };
        Ok(Frame::sign(body, &context.address)?)
    }

    pub async fn on(frame: &Frame, context: Arc<Context>, client_type: &ClientType) {
        println!("inside on data!");
        // 1️⃣ 解 Command
        let cmd = match frame.body.command_from_data() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("❌ Command decode failed: {:?}", e);
                return;
            }
        };

        // 2️⃣ 只处理 Node Online / Offline
        match (cmd.entity as Entity, cmd.action as Action) {
            // (Entity::Node, Action::OnLine) => {
            //     on_node_online(&cmd, frame, context, client_type).await;
            // }

            // (Entity::Node, Action::OnLineAck) => {
            //     on_node_online_ack(&cmd, frame, context, client_type).await;
            // }

            (Entity::Message, Action::SendText) => {
                on_text_message(&cmd, frame, context).await;
            }

            (Entity::Node, Action::OffLine) => {
                println!(
                    "⚠️ Node Offline: addr={}, nonce={}",
                    frame.body.address,
                    frame.body.nonce
                );
                on_node_offline(frame, context, client_type).await;
                // 这里你以后可以做 remove
            }

            _ => {
                println!(
                    "ℹ️ Unsupported command: entity={:?}, action={:?}",
                    cmd.entity,
                    cmd.action
                );

                frame_handler_registry.handle(frame.clone(), context, Arc::new(client_type.clone())).await;
            }
        }
    }
}

impl Codec for Frame {}

pub async fn forward_frame(receiver: String, frame: &Frame, context: Arc<Context>) {
    // ⚠️ 重要安全原则：
    // - 不解密
    // - 不反序列化 Command
    // - 不修改 Frame
    // - 只做字节级转发

    // ===== 1️⃣ 查本地 clients =====
    {
        let clients = context.clients.lock().await;

        // true = 包含直连 & 已认证连接
        let conns = clients.get_connections(&receiver, true);

        if !conns.is_empty() {
            let bytes = Frame::to_bytes(&frame.clone());

            for ct in conns {
                // ⚠️ 只发 bytes，不传 Frame
                send_bytes(&ct, &bytes).await;
            }

            // 🚨 非常重要：找到就必须 return
            // 否则会造成多路径重复转发
            return;
        }
    }

    // ===== 2️⃣ 查 servers，向其它服务器转发 =====
    let servers_guard = context.servers.lock().await;
    let bytes = Frame::to_bytes(&frame.clone());

    if let Some(servers) = &servers_guard.connected_servers {
        let all = servers.inner.iter().chain(servers.external.iter());

        for server in all {
            // ⚠️ server.client_type 本质也是一条连接
            send_bytes(&server.client_type, &bytes).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        nodes::{ net_info::NetInfo, servers::Servers, storage::Storeage },
        protocols::command::{ Action, Entity },
    };
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

        let bytes = Frame::to_bytes(&frame);
        let frame2 = Frame::from_bytes(&bytes).unwrap();

        assert_eq!(frame1.signature.to_vec(), frame2.signature.to_vec());
        assert_eq!(frame1.body.data.to_vec(), frame2.body.data.to_vec());

        Ok(())
    }

    fn make_command() -> Command {
        Command::new(Entity::Node, Action::OnLine, Some(vec![1, 2, 3, 4]))
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
            vec![9, 8, 7, 6]
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
            vec![]
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
            vec![0xaa]
        );

        let frame = Frame::new(body.clone(), vec![0xbb]);

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
            b"hello".to_vec()
        );

        let frame = Frame::sign(body.clone(), &identity)?;
        assert!(!frame.signature.is_empty());

        let encoded = bincode::serde::encode_to_vec(&frame, frame_config())?;
        let verified = Frame::verify_bytes(&encoded)?;

        assert_eq!(frame.signature, verified.signature);
        assert_eq!(frame.body.address.to_string(), verified.body.address.to_string());

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
            vec![1, 2, 3]
        );

        let frame = Frame::sign(body, &identity).unwrap();

        let bytes = Frame::to_bytes(&frame.clone());
        let decoded = Frame::from_bytes(&bytes).unwrap();

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
            vec![1, 2, 3]
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

        let body = FrameBody::new(1, addr.to_string(), addr.public_key.to_bytes(), 0, 0, vec![]);

        let bytes = encode_to_vec(&body, cfg1).unwrap();
        let (decoded, _): (FrameBody, usize) = decode_from_slice(&bytes, cfg2).unwrap();

        assert_eq!(decoded.version, 1);
    }

    use std::sync::Arc;

    use crate::context::Context;
    use crate::protocols::command::{ Command };
    use crate::protocols::frame::Frame;

    fn dummy_context() -> Context {
        let address = FreeWebMovementAddress::random(); // 如果没有 default，换成你真实构造方式

        // 1️⃣ 初始化 storage
        let storage = Storeage::new(None, None, None, None);

        // 2️⃣ 初始化 Servers（内部完成 external list 的 merge + persist）
        let servers = Servers::new(address.clone(), storage, NetInfo::new(8080));

        Context::new("127.0.0.1".to_string(), 8080, address, servers)
    }

    #[tokio::test]
    async fn test_frame_executor() -> Result<()> {
        use crate::protocols::command::{ Action, Command, Entity };

        let context = dummy_context();

        let context = Arc::new(context);
        let version = 1;

        // 调用 frame_executor 并传入回调
        Frame::executor(
            context.clone(),
            version,
            || async {
                // 这里是业务回调，生成 Command
                let cmd = Command::new(Entity::Node, Action::OnLine, Some(vec![1, 2, 3, 4]));
                Frame::build(context.clone(), cmd, version).await
            },
            None
        ).await?;

        Ok(())
    }
}
