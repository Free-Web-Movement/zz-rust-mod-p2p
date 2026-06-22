use anyhow::Result;
use sea_orm::ConnectionTrait;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, Order, QueryOrder, Set};
use sha2::{Digest, Sha256};

use super::entity::{ActiveModel, Column, Entity, Model};
use crate::db::defines::StoreFromConnection;

pub struct EpochAllocationStore<'a, C: ConnectionTrait + Send + Sync> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait + Send + Sync> StoreFromConnection<'a, C>
    for EpochAllocationStore<'a, C>
{
    fn new(db: &'a C) -> Self {
        Self { db }
    }
}

impl<'a, C: ConnectionTrait + Send + Sync> EpochAllocationStore<'a, C> {
    pub async fn last_hash(&self, epoch: i64) -> Result<String> {
        let last = Entity::find()
            .filter(Column::Epoch.eq(epoch))
            .order_by(Column::Id, Order::Desc)
            .one(&*self.db)
            .await?;

        Ok(last.map(|r| r.hash).unwrap_or_else(|| "0".repeat(64)))
    }

    pub async fn insert(
        &self,
        epoch: i64,
        round: i64,
        address: &str,
        weight: &str,
        amount: &str,
    ) -> Result<String> {
        let prev_hash = self.last_hash(epoch).await?;
        let ts = chrono::Utc::now().timestamp();

        let mut hasher = Sha256::new();
        hasher.update(prev_hash.as_bytes());
        hasher.update(epoch.to_le_bytes());
        hasher.update(round.to_le_bytes());
        hasher.update(address.as_bytes());
        hasher.update(weight.as_bytes());
        hasher.update(amount.as_bytes());
        hasher.update(ts.to_le_bytes());
        let hash = format!("{:x}", hasher.finalize());

        let am = ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            epoch: Set(epoch),
            round: Set(round),
            address: Set(address.to_string()),
            weight: Set(weight.to_string()),
            amount: Set(amount.to_string()),
            pre_hash: Set(prev_hash),
            hash: Set(hash.clone()),
            created_at: Set(ts),
        };

        am.insert(&*self.db).await?;
        Ok(hash)
    }

    pub async fn get_by_epoch(&self, epoch: i64) -> Result<Vec<Model>> {
        let records = Entity::find()
            .filter(Column::Epoch.eq(epoch))
            .order_by(Column::Id, Order::Asc)
            .all(&*self.db)
            .await?;
        Ok(records)
    }

    pub async fn verify_chain(&self, epoch: i64) -> Result<bool> {
        let records = self.get_by_epoch(epoch).await?;
        if records.is_empty() {
            return Ok(true);
        }

        if records[0].pre_hash != "0".repeat(64) {
            return Ok(false);
        }

        for i in 1..records.len() {
            if records[i].pre_hash != records[i - 1].hash {
                return Ok(false);
            }
        }

        Ok(true)
    }
}
