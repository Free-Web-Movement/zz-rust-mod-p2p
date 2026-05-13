use std::sync::Arc;

use crate::protocols::command::{Action, Entity, P2PCommand};
use crate::protocols::frame::P2PFrame;
use aex::connection::context::Context;
use aex::tcp::types::Codec;
use aex::time::SystemTime;

use bincode::{Decode, Encode};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use zz_account::address::FreeWebMovementAddress;

/// 已处理消息的去重集合（存储消息内容的 SHA-256 十六进制摘要）
pub type SeenMessages = Arc<std::sync::Mutex<std::collections::HashSet<String>>>;

/// 待确认的发送请求：request_id → oneshot (true=已送达)
pub type PendingAcks = Arc<Mutex<std::collections::HashMap<u64, tokio::sync::oneshot::Sender<bool>>>>;

const SEEN_MESSAGES_MAX: usize = 10_000;

fn dedup_key(sender: &str, receiver: &str, message: &str, timestamp: u128) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(sender.as_bytes());
    hasher.update(b"|");
    hasher.update(receiver.as_bytes());
    hasher.update(b"|");
    hasher.update(message.as_bytes());
    hasher.update(b"|");
    hasher.update(timestamp.to_le_bytes());
    format!("{:x}", hasher.finalize())
}

static NEXT_REQUEST_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub fn next_request_id() -> u64 {
    NEXT_REQUEST_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Encode, Decode)]
pub struct MessageCommand {
    pub sender: String,
    pub receiver: String,
    pub request_id: u64,
    pub timestamp: u128,
    pub message: String,
}

impl Codec for MessageCommand {}

/// 消息送达确认
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Encode, Decode)]
pub struct MessageAckCommand {
    pub request_id: u64,
}

impl Codec for MessageAckCommand {}

/// 收到的消息，用于通过 channel 通知上层应用
#[derive(Debug, Clone)]
pub struct IncomingMessage {
    pub from: String,
    pub content: String,
    pub timestamp: u128,
}

/// 向指定连接发送文本消息（不广播）
pub async fn send_text_message(
    sender: String,
    receiver: String,
    request_id: u64,
    ctx: Arc<Mutex<Context>>,
    message: &str,
) -> anyhow::Result<()> {
    let command = MessageCommand {
        sender,
        receiver,
        request_id,
        timestamp: SystemTime::timestamp(),
        message: message.to_string(),
    };

    P2PFrame::send(ctx, &Some(command), Entity::Message, Action::SendText, true).await
}

/// 发送消息确认回执
pub async fn send_message_ack(
    _receiver_addr: String,
    request_id: u64,
    ctx: Arc<Mutex<Context>>,
) -> anyhow::Result<()> {
    let cmd = MessageAckCommand { request_id };
    P2PFrame::send(ctx, &Some(cmd), Entity::Message, Action::MessageAck, true).await
}

/// 消息送达确认处理
pub async fn message_ack_handler(ctx: Arc<Mutex<Context>>, frame: P2PFrame, cmd: P2PCommand) {
    let from = &frame.body.address;
    let psk = {
        let guard = ctx.lock().await;
        guard.global.paired_session_keys.clone().unwrap()
    };

    let plaintext: Vec<u8> = {
        let guard = psk.lock().await;
        guard
            .decrypt(&from.as_bytes().to_vec(), &cmd.data)
            .await
            .expect("Wrong encrypted data!")
    };

    let ack: MessageAckCommand = match Codec::decode(&plaintext) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("❌ Invalid MessageAckCommand from {}: {:?}", from, e);
            return;
        }
    };

    tracing::info!("📬 ACK received from {} for request_id={}", from, ack.request_id);

    let gctx = {
        let guard = ctx.lock().await;
        guard.global.clone()
    };

    // 去重检查（用 ack:<request_id> 做 key）
    {
        let key = format!("ack:{}", ack.request_id);
        if let Some(seen) = gctx.get::<SeenMessages>().await {
            let mut guard = seen.lock().unwrap();
            if !guard.insert(key) {
                tracing::info!("  ⏭️  Duplicate ACK request_id={}, skipping", ack.request_id);
                return;
            }
            if guard.len() > SEEN_MESSAGES_MAX {
                guard.clear();
            }
        }
    }

    let is_for_us = {
        if let Some(pending) = gctx.get::<PendingAcks>().await {
            let mut guard = pending.lock().await;
            if let Some(tx) = guard.remove(&ack.request_id) {
                let _ = tx.send(true);
                tracing::info!("  ✅ ACK matched pending request_id={}, delivery confirmed", ack.request_id);
                true
            } else {
                false
            }
        } else {
            tracing::warn!("  ⚠️  No PendingAcks in GlobalContext");
            false
        }
    };

    if !is_for_us {
        // 不是发给我们的回执，转发给所有 peer
        tracing::info!("  🔄 Forwarding ACK request_id={} to peers", ack.request_id);
        let manager = gctx.manager.clone();
        let ack_cmd = ack.clone();
        manager
            .forward(|entries| async move {
                for entry in entries {
                    if let Some(ctx) = &entry.context {
                        let _ = P2PFrame::send(
                            ctx.clone(),
                            &Some(ack_cmd.clone()),
                            Entity::Message,
                            Action::MessageAck,
                            true,
                        )
                        .await;
                    }
                }
            })
            .await;
    }
}

pub async fn message_handler(ctx: Arc<Mutex<Context>>, frame: P2PFrame, cmd: P2PCommand) {
    let from = &frame.body.address;
    let psk = {
        let guard = ctx.lock().await;
        guard.global.paired_session_keys.clone().unwrap()
    };

    let plaintext: Vec<u8> = {
        let guard = psk.lock().await;
        guard
            .decrypt(&from.as_bytes().to_vec(), &cmd.data)
            .await
            .expect("Wrong encrypted data!")
    };

    let message: MessageCommand = match Codec::decode(&plaintext) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("❌ Invalid MessageCommand from {}: {:?}", from, e);
            return;
        }
    };

    tracing::info!("📨 message_handler: received from {}, sender={}, receiver={}, msg_len={}",
        from, message.sender, message.receiver, message.message.len());

    // 去重检查
    {
        let gctx = { ctx.lock().await.global.clone() };
        let key = dedup_key(&message.sender, &message.receiver, &message.message, message.timestamp);
        if let Some(seen) = gctx.get::<SeenMessages>().await {
            let mut guard = seen.lock().unwrap();
            if !guard.insert(key) {
                tracing::info!("  ⏭️  Duplicate message (receiver={}), skipping", message.receiver);
                return;
            }
            if guard.len() > SEEN_MESSAGES_MAX {
                guard.clear();
            }
        }
    }

    let receiver = message.receiver.clone();
    let sender_addr = message.sender.clone();
    let request_id = message.request_id;
    let address: FreeWebMovementAddress = {
        let guard = ctx.lock().await;
        guard.get::<FreeWebMovementAddress>().unwrap()
    };

    // 通知上层应用收到消息
    if receiver == address.to_string() {
        tracing::info!("  ✅ Message IS for us ({}), delivering to app channel", address);

        // 发送回执给原始发送者
        let gctx = {
            let guard = ctx.lock().await;
            guard.global.clone()
        };

        // 查找发送者的连接并发送回执
        if let Some(node) = gctx.get::<Arc<crate::node::Node>>().await {
            let seeds = node.registry.get_seeds_for_node(&sender_addr);
            if !seeds.is_empty() {
                let manager = gctx.manager.clone();
                let seeds_clone = seeds.clone();
                let req_id = request_id;
                tokio::spawn(async move {
                    manager.forward(|entries| async move {
                        for entry in entries {
                            if seeds_clone.contains(&entry.addr) {
                                if let Some(ctx) = &entry.context {
                                    let _ = send_message_ack(sender_addr.clone(), req_id, ctx.clone()).await;
                                }
                            }
                        }
                    }).await;
                });
            } else {
                // 发送者不在直接连接中，广播回执
                tracing::warn!("  ⚠️  Sender {} not in NodeRegistry, broadcasting ACK", sender_addr);
                let gctx_clone = gctx.clone();
                let req_id = request_id;
                let sender_clone = sender_addr.clone();
                tokio::spawn(async move {
                    let manager = gctx_clone.manager.clone();
                    manager.forward(|entries| async move {
                        for entry in entries {
                            if let Some(ctx) = &entry.context {
                                let _ = send_message_ack(sender_clone.clone(), req_id, ctx.clone()).await;
                            }
                        }
                    }).await;
                });
            }
        }

        if let Some(tx) = gctx
            .get::<tokio::sync::mpsc::UnboundedSender<IncomingMessage>>()
            .await
        {
            let _ = tx.send(IncomingMessage {
                from: from.clone(),
                content: message.message.clone(),
                timestamp: message.timestamp,
            });
            tracing::info!("  ✅ Message delivered to app channel");
        } else {
            tracing::warn!("  ⚠️  No app channel found for incoming message!");
        }
        return;
    }
    tracing::info!("  🔄 Message NOT for us (us={}, receiver={}), forwarding...", address, receiver);

    let manager = {
        let guard = ctx.lock().await;
        guard.global.manager.clone()
    };

    manager
        .forward(|entries| async move {
            for entry in entries {
                if let Some(ctx) = &entry.context {
                    let _ = P2PFrame::send(
                        ctx.clone(),
                        &Some(message.clone()),
                        Entity::Message,
                        Action::SendText,
                        true,
                    )
                    .await;
                }
            }
        })
        .await
}
