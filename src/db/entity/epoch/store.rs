use anyhow::{Result, anyhow};
use sea_orm::ActiveValue::NotSet;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set};

use crate::db::defines::StoreFromConnection;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "epoch")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub day: i64,
    pub start_time: i64,
    pub end_time: Option<i64>,
    pub tick_count: u8,
    pub total_reward: String,
    pub settled: bool,
    pub pre_hash: String,
    pub hash: String,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match *self {}
    }
}

impl ActiveModelBehavior for ActiveModel {}

pub struct EpochStore<'a, C: ConnectionTrait + Send + Sync + 'a> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait + Send + Sync + 'a> StoreFromConnection<'a, C> for EpochStore<'a, C> {
    fn new(db: &'a C) -> Self {
        Self { db }
    }
}

impl<'a, C: ConnectionTrait + Send + Sync + 'a> EpochStore<'a, C> {
    pub async fn get_by_index(&self, day: i64) -> Result<Option<Model>> {
        let record = Entity::find()
            .filter(Column::Day.eq(day))
            .one(&*self.db)
            .await?;
        Ok(record)
    }

    pub async fn get_latest(&self) -> Result<Option<Model>> {
        let records = Entity::find().all(&*self.db).await?;
        Ok(records.into_iter().max_by_key(|r| r.day))
    }

    pub async fn get_current(&self) -> Result<Option<Model>> {
        let latest = self.get_latest().await?;
        Ok(latest.and_then(|e| if !e.settled { Some(e) } else { None }))
    }

    pub async fn insert(&self, day: i64, start_time: i64) -> Result<Model> {
        let pre_hash = "0".repeat(32);
        let am = ActiveModel {
            id: NotSet,
            day: Set(day),
            start_time: Set(start_time),
            end_time: Set(None),
            tick_count: Set(0u8),
            total_reward: Set(String::new()),
            settled: Set(false),
            pre_hash: Set(pre_hash),
            hash: Set(String::new()),
        };
        am.insert(&*self.db).await.map_err(|e| anyhow!(e))
    }

    pub async fn create(&self, day: i64, pre_hash: &str) -> Result<Model> {
        let now = chrono::Utc::now().timestamp();
        let am = ActiveModel {
            id: NotSet,
            day: Set(day),
            start_time: Set(now),
            end_time: Set(None),
            tick_count: Set(0u8),
            total_reward: Set(String::new()),
            settled: Set(false),
            pre_hash: Set(pre_hash.to_string()),
            hash: Set(String::new()),
        };
        am.insert(&*self.db).await.map_err(|e| anyhow!(e))
    }

    pub async fn update_tick_count(&self, day: i64, tick_count: u8) -> Result<()> {
        let record = self
            .get_by_index(day)
            .await?
            .ok_or_else(|| anyhow!("Epoch not found"))?;

        let mut am: ActiveModel = record.into();
        am.tick_count = Set(tick_count);
        am.update(&*self.db).await?;
        Ok(())
    }

    pub async fn finalize(
        &self,
        day: i64,
        hash: &str,
        tick_count: u8,
        total_reward: &str,
    ) -> Result<()> {
        let record = self
            .get_by_index(day)
            .await?
            .ok_or_else(|| anyhow!("Epoch not found"))?;

        let mut am: ActiveModel = record.into();
        am.end_time = Set(Some(chrono::Utc::now().timestamp()));
        am.tick_count = Set(tick_count);
        am.total_reward = Set(total_reward.to_string());
        am.settled = Set(true);
        am.hash = Set(hash.to_string());
        am.update(&*self.db).await?;
        Ok(())
    }

    pub async fn get_current_or_create(&self, pre_hash: &str) -> Result<Model> {
        if let Some(latest) = self.get_latest().await? {
            if !latest.settled {
                return Ok(latest);
            }
            let next_day = latest.day + 1;
            return self.create(next_day, pre_hash).await;
        }
        self.create(0, pre_hash).await
    }
}
