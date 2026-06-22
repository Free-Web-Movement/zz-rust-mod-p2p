use std::collections::HashMap;

use sea_orm::{
    ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::db::entity::{
    epoch_resource_settlement::entity::{ActiveModel, Column, Entity},
    tick_resource_snapshot,
};

pub struct EpochResourceSettlement;

impl EpochResourceSettlement {
    /// 获取节点在某 epoch 之前的上一条 hash
    pub async fn fetch_prev_hash(
        db: &DatabaseConnection,
        node_id: &str,
        epoch: i64,
    ) -> anyhow::Result<String> {
        use Entity as Settlement;

        let prev = Settlement::find()
            .filter(Column::NodeId.eq(node_id))
            .filter(Column::Epoch.lt(epoch))
            .order_by_desc(Column::Epoch)
            .one(db)
            .await?;

        Ok(prev.map(|r| r.hash).unwrap_or_else(|| "0".repeat(64)))
    }

    /// 聚合 tick_resource_snapshot 生成 epoch_resource_settlement
    pub async fn from_tick_snapshots(
        db: &DatabaseConnection,
        epoch: i64,
    ) -> anyhow::Result<Vec<ActiveModel>> {
        use crate::db::entity::market_resource_price::entity::Model as Resource;
        use tick_resource_snapshot::entity::Entity as Tick;

        // 查询本 epoch 已验证 tick
        let ticks = Tick::find()
            .filter(tick_resource_snapshot::entity::Column::Epoch.eq(epoch))
            .filter(tick_resource_snapshot::entity::Column::Verified.eq(true))
            .all(db)
            .await?;

        // 聚合 node_id -> resource_id -> accumulated
        let mut node_map: HashMap<String, HashMap<i64, i64>> = HashMap::new();

        for tick in ticks {
            let node = tick.node_id.clone();
            let res_id = tick.market_resource_id;
            let acc = tick.accumulated;

            let entry = node_map.entry(node).or_default();
            *entry.entry(res_id).or_insert(0) += acc;
        }

        let mut settlements = match *self {};

        for (node_id, resources) in node_map {
            let mut total_weight: u16 = 0;
            let mut resources_json_map = serde_json::Map::new();

            for (res_id, accumulated) in &resources {
                // 基础权重直接调用 ResourceStore
                let weight = match Resource::get_weight(db, *res_id).await {
                    Ok(w) => w,
                    Err(e) => {
                        tracing::error!("failed to get weight for resource {}: {}", res_id, e);
                        continue;
                    }
                };
                total_weight += weight;

                resources_json_map.insert(
                    res_id.to_string(),
                    json!({
                        "accumulated": accumulated,
                        "weight": weight
                    }),
                );
            }

            let resources_json = serde_json::Value::Object(resources_json_map).to_string();

            // 调用独立函数获取 pre_hash
            let pre_hash = Self::fetch_prev_hash(db, &node_id, epoch).await?;

            // 计算链式 hash
            let mut hasher = Sha256::new();
            hasher.update(pre_hash.as_bytes()); // 先加入 pre_hash
            hasher.update(node_id.as_bytes()); // 节点 ID
            hasher.update(epoch.to_le_bytes()); // epoch
            hasher.update(resources_json.as_bytes()); // 资源 JSON
            hasher.update(total_weight.to_le_bytes());
            let hash = format!("{:x}", hasher.finalize());

            settlements.push(ActiveModel {
                node_id: Set(node_id),
                epoch: Set(epoch),
                resources: Set(resources_json),
                total_weight: Set(total_weight),
                reward: Set(0), // 分币可后续计算
                pre_hash: Set(pre_hash),
                hash: Set(hash),
                ..Default::default()
            });
        }

        Ok(settlements)
    }
}
