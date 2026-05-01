use std::sync::Arc;
use tokio::sync::Mutex;

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

use aex::tcp::types::Codec;

use crate::protocols::{
    command::{Action, Entity, P2PCommand},
    frame::P2PFrame,
};

// ==================== 数据结构定义 ====================

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct NodeSyncRequest {
    pub request_id: String,
    pub node_id: String,
    pub sync_type: String, // "full" 或 "incremental"
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

// ==================== 处理器函数 ====================

/// 处理节点同步请求（老节点/已同步节点）
/// 从数据库读取所有相关数据并返回
pub async fn node_sync_handler(ctx: Arc<Mutex<aex::connection::context::Context>>, frame: P2PFrame, cmd: P2PCommand) {
    println!("🔄 Received node sync request from {}", frame.body.address);

    let request: NodeSyncRequest = match Codec::decode(&cmd.data) {
        Ok(req) => req,
        Err(e) => {
            eprintln!("❌ Failed to decode NodeSyncRequest: {}", e);
            return;
        }
    };

    println!(
        "  Request ID: {}, Node: {}, Type: {}",
        request.request_id, request.node_id, request.sync_type
    );

    // TODO: 从数据库读取数据
    // 这里需要先获取 db 连接，再查询各表数据
    // 暂时返回空数据作为框架
    let response = NodeSyncResponse {
        request_id: request.request_id,
        success: true,
        transactions: vec![],
        balances: vec![],
        epochs: vec![],
        seeds: vec![],
        meta_entries: vec![],
    };

    // 发送响应 - 让 P2PFrame::send 处理编码
    if let Err(e) = P2PFrame::send::<NodeSyncResponse>(
        ctx.clone(),
        &Some(response),
        Entity::Node,
        Action::NodeSyncResponse,
        false,
    )
    .await
    {
        eprintln!("❌ Failed to send NodeSyncResponse: {}", e);
    } else {
        println!("✅ Sent node sync response");
    }
}

/// 处理节点同步响应（新节点/待同步节点）
/// 接收数据并写入本地数据库
pub async fn node_sync_response_handler(
    ctx: Arc<Mutex<aex::connection::context::Context>>,
    frame: P2PFrame,
    cmd: P2PCommand,
) {
    println!(
        "✅ Received node sync response from {}",
        frame.body.address
    );

    let response: NodeSyncResponse = match Codec::decode(&cmd.data) {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("❌ Failed to decode NodeSyncResponse: {}", e);
            return;
        }
    };

    println!(
        "  Request ID: {}, Success: {}, Data: tx={}, bal={}, epoch={}, seed={}, meta={}",
        response.request_id,
        response.success,
        response.transactions.len(),
        response.balances.len(),
        response.epochs.len(),
        response.seeds.len(),
        response.meta_entries.len()
    );

    if !response.success {
        eprintln!("❌ Node sync failed on remote side");
        return;
    }

    // TODO: 将数据写入本地数据库
    // 需要在调用处传入 db 连接并执行写入
    println!("📦 Node sync data received (writing to DB not yet implemented)");
}

// ==================== 触发同步的公共函数 ====================

/// 向指定节点请求节点数据同步
pub async fn request_node_sync(
    ctx: Arc<Mutex<aex::connection::context::Context>>,
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
    )
    .await?;

    println!("✅ Node sync request sent");
    Ok(())
}
