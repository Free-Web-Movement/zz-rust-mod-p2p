use anyhow::{Result, anyhow};
use sea_orm::ActiveValue::NotSet;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set};

use crate::db::defines::StoreFromConnection;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "witness")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub node_id: i64,
    pub address: String,
    pub online_seconds: i64,
    pub weight: i64,
    pub status: String,
    pub last_tick_ts: i64,
    pub resources: Option<String>,
    pub is_public_ip: bool,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match *self {}
    }
}

impl ActiveModelBehavior for ActiveModel {}

pub struct WitnessStore<'a, C: ConnectionTrait + Send + Sync + 'a> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait + Send + Sync + 'a> StoreFromConnection<'a, C> for WitnessStore<'a, C> {
    fn new(db: &'a C) -> Self {
        Self { db }
    }
}

impl<'a, C: ConnectionTrait + Send + Sync + 'a> WitnessStore<'a, C> {
    pub async fn get_by_address(&self, address: &str) -> Result<Option<Model>> {
        let record = Entity::find()
            .filter(Column::Address.eq(address.to_string()))
            .one(&*self.db)
            .await?;
        Ok(record)
    }

    pub async fn upsert(
        &self,
        node_id: i64,
        address: &str,
        online_seconds: i64,
        weight: i64,
        resources: Option<String>,
        is_public_ip: bool,
    ) -> Result<Model> {
        let record = self.get_by_address(address).await?;

        match record {
            Some(existing) => {
                let mut am: ActiveModel = existing.into();
                am.online_seconds = Set(online_seconds);
                am.weight = Set(weight);
                am.last_tick_ts = Set(chrono::Utc::now().timestamp());
                am.resources = Set(resources);
                am.is_public_ip = Set(is_public_ip);
                am.update(&*self.db).await.map_err(|e| anyhow!(e))
            }
            None => {
                let am = ActiveModel {
                    id: NotSet,
                    node_id: Set(node_id),
                    address: Set(address.to_string()),
                    online_seconds: Set(online_seconds),
                    weight: Set(weight),
                    status: Set("Active".to_string()),
                    last_tick_ts: Set(chrono::Utc::now().timestamp()),
                    resources: Set(resources),
                    is_public_ip: Set(is_public_ip),
                };
                am.insert(&*self.db).await.map_err(|e| anyhow!(e))
            }
        }
    }

    pub async fn update_online_seconds(&self, address: &str, seconds: i64) -> Result<()> {
        let record = self
            .get_by_address(address)
            .await?
            .ok_or_else(|| anyhow!("Witness not found"))?;

        let mut am: ActiveModel = record.into();
        am.online_seconds = Set(seconds);
        am.last_tick_ts = Set(chrono::Utc::now().timestamp());
        am.update(&*self.db).await?;
        Ok(())
    }

    pub async fn get_active_witnesses(&self) -> Result<Vec<Model>> {
        let records = Entity::find()
            .filter(Column::Status.eq("Active"))
            .all(&*self.db)
            .await?;
        Ok(records)
    }

    pub async fn get_all(&self) -> Result<Vec<Model>> {
        let records = Entity::find().all(&*self.db).await?;
        Ok(records)
    }
}
