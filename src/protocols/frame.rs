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
        self.data = Codec::encode(cmd)?;
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
        let bytes = Codec::encode(&body)?;
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
        let bytes = Codec::encode(&frame.body)?;
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
        let cmd_bytes = Codec::encode(&cmd)?;
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
        let Ok(bytes) = Codec::encode(&self.body) else {
            return false;
        };
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
        match Codec::encode(&self.body) {
            Ok(raw_bytes) => signer(&raw_bytes),
            Err(e) => {
                tracing::error!("Failed to encode frame for signing: {:?}", e);
                Vec::new()
            }
        }
    }

    fn payload(&self) -> Option<Vec<u8>> {
        Codec::encode(&self.body).ok()
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
            Some(cmd) => Codec::encode(cmd)?,
            None => vec![],
        };

        let (gctx, peer_sock) = {
            let guard = ctx.lock().await;
            (guard.global.clone(), guard.addr)
        };

        let gpsk = gctx.clone().paired_session_keys.clone();

        let address = match gctx.get::<FreeWebMovementAddress>().await {
            Some(a) => a,
            None => {
                tracing::error!("FreeWebMovementAddress not set in GlobalContext");
                return Err(anyhow::anyhow!("Address not set"));
            }
        };

        let addr_str = address.to_string();
        let bytes = if is_encrypt {
            match gpsk {
                Some(psk) => {
                    let encode = psk.lock().await;
                    // For MessageCommand (SendText), use the recipient's address as the encryption key.
                    // This ensures per-recipient session keys work correctly across multiple peers.
                    let key = if action == Action::SendText {
                        // Decode receiver from the serialized MessageCommand.
                        let decoded: anyhow::Result<
                            crate::protocols::commands::message::MessageCommand,
                        > = Codec::decode(&data);
                        match decoded {
                            Ok(msg) => msg.receiver.as_bytes().to_vec(),
                            _ => {
                                tracing::warn!(
                                    "⚠️ Failed to decode MessageCommand for key lookup, falling back to self address"
                                );
                                addr_str.as_bytes().to_vec()
                            }
                        }
                    } else if action == Action::MessageAck {
                        // For MessageAck, use the peer's address (the node at the other end of
                        // this connection) as the encryption key. The session key table stores
                        // keys per-peer under the peer's address, so looking up main[self_addr]
                        // (the old behaviour) would always fail.
                        // First try the peer address stored in the connection context during
                        // the online handshake, then fall back to registry lookup.
                        let peer_addr_from_ctx: Option<String> = {
                            let guard = ctx.lock().await;
                            let addr: Option<String> = guard.get();
                            tracing::info!(
                                "🔑 MessageAck key: ctx.addr={:?}, ctx.get::<String>()={:?}",
                                guard.addr,
                                addr
                            );
                            addr
                        };
                        if let Some(ref peer_addr) = peer_addr_from_ctx {
                            tracing::info!("🔑 MessageAck using ctx peer_addr='{}'", peer_addr);
                            peer_addr.as_bytes().to_vec()
                        } else if let Some(node) =
                            gctx.get::<std::sync::Arc<crate::node::Node>>().await
                        {
                            if let Some(ref peer_addr) =
                                node.registry.find_node_for_seed(&peer_sock)
                            {
                                tracing::info!(
                                    "🔑 MessageAck using registry peer_addr='{}'",
                                    peer_addr
                                );
                                peer_addr.as_bytes().to_vec()
                            } else {
                                tracing::warn!(
                                    "⚠️ MessageAck: peer {} not found in registry, falling back to self address '{}'",
                                    peer_sock,
                                    addr_str
                                );
                                addr_str.as_bytes().to_vec()
                            }
                        } else {
                            tracing::warn!(
                                "⚠️ MessageAck: Node not available in context, falling back to self address '{}'",
                                addr_str
                            );
                            addr_str.as_bytes().to_vec()
                        }
                    } else {
                        addr_str.as_bytes().to_vec()
                    };
                    match encode.encrypt(&key, &data).await {
                        Ok(ct) => ct,
                        Err(e) => {
                            tracing::error!(
                                "❌ ENCRYPT FAILED for address='{}' (action={:?}): {:?}",
                                addr_str,
                                action,
                                e
                            );
                            return Err(e);
                        }
                    }
                }
                None => data,
            }
        } else {
            data
        };

        tracing::info!(
            "📤 P2PFrame::send: {} {:?} encrypt={} data_len={}",
            addr_str,
            action,
            is_encrypt,
            bytes.len()
        );

        let command = P2PCommand::new(entity, action, bytes);

        let frame = match P2PFrame::build(&address, command, 1).await {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("Failed to build P2PFrame: {:?}", e);
                return Err(e);
            }
        };

        let bytes = match Codec::encode(&frame) {
            Ok(b) => b,
            Err(e) => {
                tracing::error!("Failed to encode frame for sending: {:?}", e);
                return Err(e);
            }
        };

        let mut guard = ctx.lock().await;
        if let Some(ref mut writer) = guard.writer {
            if let Err(e) = writer.write_all(&bytes).await {
                tracing::error!("Failed to send data: {:?}", e);
            }

            let _ = writer.flush().await;
        }
        Ok(())
    }

    pub async fn send_bytes(writer: &mut AexWriter, bytes: &[u8]) {
        if let Err(e) = writer.write_all(&bytes).await {
            tracing::error!("Failed to send TCP bytes: {:?}", e);
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
                let Ok(bytes) = Codec::encode(frame) else {
                    tracing::error!("Failed to encode frame for notify");
                    return;
                };
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
