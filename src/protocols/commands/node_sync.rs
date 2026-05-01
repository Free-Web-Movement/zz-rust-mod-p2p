use tokio::sync::oneshot;
use tokio::sync::Mutex;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

use aex::tcp::types::Codec;
use aex::connection::context::Context;

use crate::protocols::{
    command::{Action, Entity, P2PCommand},
    frame::P2PFrame,
};

// ================== 数据结构定义 ==================

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct NodeSyncRequest {
    pub request_id: String,
    pub node_id: String,
    pub sync_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct NodeSyncResponse {
    pub request_id: String,
    pub success: bool,
    pub transactions: Vec<TransactionData>,
    pub balances: Vec<BalanceData>,
    pub epochs: Vec<EpochData>,
    pub seeds: Vec<SeedData>,
    pub meta_entries: Vec<MetaEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct TransactionData {
    pub id: i64,
    pub epoch: i64,
    pub cycle: i64,
    pub tx_type: String,
    pub from_address: String,
    pub to_address: String,
    pub amount: String,
    pub timestamp: i64,
    pub pre_hash: String,
    pub hash: String,
    pub shard_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct BalanceData {
    pub id: i64,
    pub address: String,
    pub balance: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct EpochData {
    pub id: i64,
    pub day: i64,
    pub start_time: i64,
    pub end_time: i64,
    pub tick_count: u32,
    pub total_reward: String,
    pub settled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct SeedData {
    pub id: i64,
    pub ip: String,
    pub port: i32,
    pub node_id: String,
    pub is_intranet: bool,
    pub is_active: bool,
    pub success_count: i32,
    pub failure_count: i32,
    pub online_days: i64,
    pub offline_days: i64,
    pub online_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct MetaEntry {
    pub key: String,
    pub value: String,
}

impl Codec for NodeSyncRequest {}
impl Codec for NodeSyncResponse {}

// ================== 通道机制 ==================

use tokio::sync::mpsc;

/// 发给主项目的请求包，包含 request_id 和 oneshot 回传通道
pub struct SyncRequest {
    pub request_id: String,
    pub response_tx: oneshot::Sender<NodeSyncResponse>,
}

type RequestSender = mpsc::Sender<SyncRequest>;
type ResponseSender = mpsc::Sender<NodeSyncResponse>;

static REQUEST_SENDER: StdMutex<Option<RequestSender>> = StdMutex::new(None);
static RESPONSE_SENDER: StdMutex<Option<ResponseSender>> = StdMutex::new(None);

// ================== 处理器函数 ==================

/// 处理节点同步请求（老节点/已同步节点）
pub async fn node_sync_handler(ctx: Arc<Mutex<Context>>, _frame: P2PFrame, cmd: P2PCommand) {
    println!("🔄 Received node sync request");

    let request: NodeSyncRequest = match Codec::decode(&cmd.data) {
        Ok(req) => req,
        Err(e) => {
            eprintln!("❌ Failed to decode NodeSyncRequest: {}", e);
            return;
        }
    };

    println!("  Request ID: {}, Node: {}, Type: {}", 
             request.request_id, request.node_id, request.sync_type);

    // 通过通道请求主项目提供数据
    let response = {
        let tx_option = {
            let guard = REQUEST_SENDER.lock().unwrap();
            guard.clone()
        };

        match tx_option {
            Some(tx) => {
                let (resp_tx, resp_rx) = oneshot::channel();
                let sync_req = SyncRequest {
                    request_id: request.request_id.clone(),
                    response_tx: resp_tx,
                };
                if let Err(e) = tx.send(sync_req).await {
                    eprintln!("❌ Failed to send request to main: {}", e);
                    NodeSyncResponse {
                        request_id: request.request_id,
                        success: false,
                        transactions: vec![],
                        balances: vec![],
                        epochs: vec![],
                        seeds: vec![],
                        meta_entries: vec![],
                    }
                } else {
                    match resp_rx.await {
                        Ok(resp) => resp,
                        Err(e) => {
                            eprintln!("❌ Failed to receive response from main: {}", e);
                            NodeSyncResponse {
                                request_id: request.request_id,
                                success: false,
                                transactions: vec![],
                                balances: vec![],
                                epochs: vec![],
                                seeds: vec![],
                                meta_entries: vec![],
                            }
                        }
                    }
                }
            }
            None => {
                eprintln!("⚠️  No request sender registered");
                NodeSyncResponse {
                    request_id: request.request_id,
                    success: false,
                    transactions: vec![],
                    balances: vec![],
                    epochs: vec![],
                    seeds: vec![],
                    meta_entries: vec![],
                }
            }
        }
    };

    // 发送响应
    if let Err(e) = P2PFrame::send::<NodeSyncResponse>(
        ctx.clone(),
        &Some(response),
        Entity::Node,
        Action::NodeSyncResponse,
        false,
    ).await {
        eprintln!("❌ Failed to send NodeSyncResponse: {}", e);
    } else {
        println!("✅ Sent node sync response");
    }
}

/// 处理节点同步响应（新节点/待同步节点）
pub async fn node_sync_response_handler(
    _ctx: Arc<Mutex<Context>>,
    _frame: P2PFrame,
    cmd: P2PCommand,
) {
    println!("✅ Received node sync response");

    let response: NodeSyncResponse = match Codec::decode(&cmd.data) {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("❌ Failed to decode NodeSyncResponse: {}", e);
            return;
        }
    };

    println!("  Request ID: {}, Success: {}, Seeds: {}", 
             response.request_id, response.success, response.seeds.len());

    if !response.success {
        eprintln!("❌ Node sync failed on remote side");
        return;
    }

    // 通过通道将响应传回主项目
    let tx_option = {
        let guard = RESPONSE_SENDER.lock().unwrap();
        guard.clone()
    };

    if let Some(tx) = tx_option {
        if let Err(e) = tx.send(response).await {
            eprintln!("❌ Failed to send response to main: {}", e);
        } else {
            println!("📦 Node sync data forwarded to main project");
        }
    } else {
        println!("⚠️  No response sender registered");
    }
}

// ================== 触发同步的公共函数 ==================

/// 向指定节点请求节点数据同步
pub async fn request_node_sync(
    ctx: Arc<Mutex<Context>>,
    node_id: String,
    sync_type: String,
) -> anyhow::Result<()> {
    let request_id = uuid::Uuid::new_v4().to_string();
    let request = NodeSyncRequest {
        request_id: request_id.clone(),
        node_id,
        sync_type,
    };

    println!("🔄 Requesting node sync (ID: {})", request_id);

    P2PFrame::send::<NodeSyncRequest>(
        ctx.clone(),
        &Some(request),
        Entity::Node,
        Action::NodeSyncRequest,
        false,
    ).await?;

    println!("✅ Node sync request sent");
    Ok(())
}

/// 设置请求通道（在主项目中调用，用于接收数据请求）
pub fn set_request_sender(tx: RequestSender) {
    let mut guard = REQUEST_SENDER.lock().unwrap();
    *guard = Some(tx);
}

/// 设置响应通道（在主项目中调用，用于发送响应数据）
pub fn set_response_sender(tx: ResponseSender) {
    let mut guard = RESPONSE_SENDER.lock().unwrap();
    *guard = Some(tx);
}

/// 获取请求接收器（在主项目中使用，用于提供数据）
pub fn get_request_receiver() -> (RequestSender, mpsc::Receiver<SyncRequest>) {
    let (tx, rx) = mpsc::channel(1);
    (tx, rx)
}

/// 获取响应接收器（在主项目中使用，用于接收同步数据）
pub fn get_response_receiver() -> (ResponseSender, mpsc::Receiver<NodeSyncResponse>) {
    let (tx, rx) = mpsc::channel(1);
    (tx, rx)
}
