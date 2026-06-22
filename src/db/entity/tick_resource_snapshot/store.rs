use anyhow::Result;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue::NotSet, ActiveValue::Set, QueryOrder};
use sha2::{Digest, Sha256};

use super::entity::{ActiveModel, Column, Entity, Model};
use crate::db::defines::StoreFromConnection;

pub struct TickResourceSnapshotStore<'a, C: ConnectionTrait + Send + Sync + 'a> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait + Send + Sync + 'a> StoreFromConnection<'a, C>
    for TickResourceSnapshotStore<'a, C>
{
    fn new(db: &'a C) -> Self {
        Self { db }
    }
}

impl<'a, C: ConnectionTrait + Send + Sync + 'a> TickResourceSnapshotStore<'a, C> {
    pub async fn create(
        &self,
        node_id: &str,
        epoch: i64,
        tick: i64,
        market_resource_id: i64,
        accumulated: i64,
    ) -> Result<Model> {
        let pre_hash = self.get_last_hash(node_id, epoch).await?;

        let mut hasher = Sha256::new();
        hasher.update(pre_hash.as_bytes());
        hasher.update(node_id.as_bytes());
        hasher.update(epoch.to_le_bytes());
        hasher.update(tick.to_le_bytes());
        hasher.update(market_resource_id.to_le_bytes());
        hasher.update(accumulated.to_le_bytes());
        let hash = format!("{:x}", hasher.finalize());

        let am = ActiveModel {
            id: NotSet,
            node_id: Set(node_id.to_string()),
            epoch: Set(epoch),
            tick: Set(tick),
            market_resource_id: Set(market_resource_id),
            accumulated: Set(accumulated),
            verified: Set(false),
            pre_hash: Set(pre_hash),
            hash: Set(hash),
        };

        am.insert(&*self.db).await.map_err(|e| anyhow::anyhow!(e))
    }

    async fn get_last_hash(&self, node_id: &str, epoch: i64) -> Result<String> {
        let record = Entity::find()
            .filter(Column::NodeId.eq(node_id))
            .filter(Column::Epoch.eq(epoch))
            .order_by_desc(Column::Tick)
            .one(&*self.db)
            .await?;

        Ok(record.map(|r| r.hash).unwrap_or_else(|| "0".repeat(64)))
    }

    pub async fn get_by_epoch(&self, epoch: i64) -> Result<Vec<Model>> {
        let records = Entity::find()
            .filter(Column::Epoch.eq(epoch))
            .all(&*self.db)
            .await?;
        Ok(records)
    }

    pub async fn get_by_node(&self, node_id: &str, epoch: i64) -> Result<Vec<Model>> {
        let records = Entity::find()
            .filter(Column::NodeId.eq(node_id))
            .filter(Column::Epoch.eq(epoch))
            .all(&*self.db)
            .await?;
        Ok(records)
    }

    pub async fn mark_verified(&self, id: i64) -> Result<()> {
        let record = Entity::find()
            .filter(Column::Id.eq(id))
            .one(&*self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Snapshot not found"))?;

        let mut am: ActiveModel = record.into();
        am.verified = Set(true);
        am.update(&*self.db).await?;
        Ok(())
    }

    pub async fn get_total_by_node(&self, node_id: &str, epoch: i64) -> Result<i64> {
        let snapshots = self.get_by_node(node_id, epoch).await?;
        let total: i64 = snapshots.iter().map(|s| s.accumulated).sum();
        Ok(total)
    }
}
