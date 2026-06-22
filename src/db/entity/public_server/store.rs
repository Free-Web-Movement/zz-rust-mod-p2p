use anyhow::Result;
use sea_orm::ActiveValue::NotSet;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, QueryOrder, Set};

use crate::db::defines::StoreFromConnection;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "public_server")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub address: String,
    pub port: u16,
    pub historical_addresses: Option<String>,
    pub protocol: String,
    pub is_active: bool,
    pub last_seen: i64,
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match *self {}
    }
}

impl ActiveModelBehavior for ActiveModel {}

pub struct PublicServerStore<'a, C: ConnectionTrait + Send + Sync + 'a> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait + Send + Sync + 'a> StoreFromConnection<'a, C>
    for PublicServerStore<'a, C>
{
    fn new(db: &'a C) -> Self {
        Self { db }
    }
}

impl<'a, C: ConnectionTrait + Send + Sync + 'a> PublicServerStore<'a, C> {
    pub async fn get_all(&self) -> Result<Vec<Model>> {
        Ok(Entity::find()
            .order_by_desc(Column::LastSeen)
            .all(&*self.db)
            .await?)
    }

    pub async fn get_active(&self) -> Result<Vec<Model>> {
        Ok(Entity::find()
            .filter(Column::IsActive.eq(true))
            .order_by_desc(Column::LastSeen)
            .all(&*self.db)
            .await?)
    }

    pub async fn upsert(&self, address: &str, port: u16, protocol: &str) -> Result<Model> {
        let now = chrono::Utc::now().timestamp();

        let existing = Entity::find()
            .filter(Column::Address.eq(address.to_string()))
            .filter(Column::Port.eq(port))
            .one(&*self.db)
            .await?;

        match existing {
            Some(record) => {
                let was_active = record.is_active;
                let mut am: ActiveModel = record.into();
                // Only update last_seen when node comes online from offline
                if !was_active {
                    am.last_seen = Set(now);
                }
                am.is_active = Set(true);
                am.update(&*self.db).await.map_err(|e| anyhow::anyhow!(e))
            }
            None => {
                let am = ActiveModel {
                    id: NotSet,
                    address: Set(address.to_string()),
                    port: Set(port),
                    historical_addresses: Set(serde_json::to_string(&Vec::<String>::new()).ok()),
                    protocol: Set(protocol.to_string()),
                    is_active: Set(true),
                    last_seen: Set(now),
                    created_at: Set(now),
                };
                am.insert(&*self.db).await.map_err(|e| anyhow::anyhow!(e))
            }
        }
    }

    pub async fn update_with_historical(
        &self,
        address: &str,
        port: u16,
        protocol: &str,
        historical: Vec<String>,
    ) -> Result<Model> {
        let now = chrono::Utc::now().timestamp();
        let historical_json = serde_json::to_string(&historical).ok();

        let existing = Entity::find()
            .filter(Column::Address.eq(address.to_string()))
            .filter(Column::Port.eq(port))
            .one(&*self.db)
            .await?;

        match existing {
            Some(record) => {
                let mut am: ActiveModel = record.into();
                am.last_seen = Set(now);
                am.is_active = Set(true);
                am.historical_addresses = Set(historical_json.clone());
                am.update(&*self.db).await.map_err(|e| anyhow::anyhow!(e))
            }
            None => {
                let am = ActiveModel {
                    id: NotSet,
                    address: Set(address.to_string()),
                    port: Set(port),
                    historical_addresses: Set(historical_json.clone()),
                    protocol: Set(protocol.to_string()),
                    is_active: Set(true),
                    last_seen: Set(now),
                    created_at: Set(now),
                };
                am.insert(&*self.db).await.map_err(|e| anyhow::anyhow!(e))
            }
        }
    }

    pub async fn mark_inactive(&self, id: i64) -> Result<()> {
        let record = Entity::find()
            .filter(Column::Id.eq(id))
            .one(&*self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Public server not found"))?;

        let mut am: ActiveModel = record.into();
        am.is_active = Set(false);
        am.update(&*self.db).await?;
        Ok(())
    }

    pub async fn remove(&self, id: i64) -> Result<()> {
        let record = Entity::find()
            .filter(Column::Id.eq(id))
            .one(&*self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Public server not found"))?;

        record.delete(&*self.db).await?;
        Ok(())
    }
}
