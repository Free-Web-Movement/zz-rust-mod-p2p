use aex::{
    connection::context::{AexWriter, Context},
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
        ctx: Arc<Mutex<Context>>,
        command: &Option<C>,
        entity: Entity,
        action: Action,
        is_encrypt: bool,
    ) -> anyhow::Result<()> {
        let data = match command {
            Some(cmd) => Codec::encode(cmd),
            None => vec![],
        };

        let gctx = {
            let guard = ctx.lock().await;
            guard.global.clone()
        };

        let gpsk = gctx.clone().paired_session_keys.clone();

        let address = gctx.get::<FreeWebMovementAddress>().await.unwrap();

        let bytes = if is_encrypt {
            match gpsk {
                Some(psk) => {
                    let encode = psk.lock().await;
                    encode
                        .encrypt(&address.to_string().as_bytes().to_vec(), &data)
                        .await?
                }
                None => data,
            }
        } else {
            data
        };

        let command = P2PCommand::new(entity, action, bytes);

        let frame = P2PFrame::build(&address, command, 1).await.unwrap();

        let bytes = Codec::encode(&frame);

        let mut guard = ctx.lock().await;
        if let Some(ref mut writer) = guard.writer {
            if let Err(e) = writer.write_all(&bytes).await {
                eprintln!("Failed to send data: {:?}", e);
            }

            let _ = writer.flush().await;
        }
        Ok(())
    }

    pub async fn send_bytes(writer: &mut AexWriter, bytes: &[u8]) {
        if let Err(e) = writer.write_all(&bytes).await {
            eprintln!("Failed to send TCP bytes: {:?}", e);
        }
    }

    pub async fn notify(&self, ctx: Arc<Mutex<Context>>) {
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

                let frame: &P2PFrame = self;
                let bytes = Codec::encode(frame);
                manager
                    .forward(|entries| async {
                        for entry in entries {
                            if let Some(ctx) = &entry.context {
                                let mut guard = ctx.lock().await;
                                if let Some(writer) = &mut guard.writer {
                                    P2PFrame::send_bytes(writer, &bytes).await
                                }
                            }
                            continue;
                        }
                    })
                    .await;
            };
        }
    }
}
