use anyhow::Result;
use sea_orm::ConnectionTrait;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, ActiveValue::NotSet, QueryOrder, Set};

use super::entity::{ActiveModel, Column, Entity, Model};
use crate::db::defines::StoreFromConnection;

pub struct ContactStore<'a, C: ConnectionTrait + Send + Sync> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait + Send + Sync> StoreFromConnection<'a, C> for ContactStore<'a, C> {
    fn new(db: &'a C) -> Self {
        Self { db }
    }
}

impl<'a, C: ConnectionTrait + Send + Sync> ContactStore<'a, C> {
    pub async fn add(&self, name: &str, address: &str) -> Result<Model> {
        let now = chrono::Utc::now().timestamp();
        let am = ActiveModel {
            id: NotSet,
            name: Set(name.to_string()),
            address: Set(address.to_string()),
            created_at: Set(now),
            updated_at: Set(now),
        };
        let model = am.insert(&*self.db).await?;
        Ok(model)
    }

    pub async fn get_all(&self) -> Result<Vec<Model>> {
        let records = Entity::find()
            .order_by_asc(Column::Name)
            .all(&*self.db)
            .await?;
        Ok(records)
    }

    pub async fn find_by_address(&self, address: &str) -> Result<Option<Model>> {
        let record = Entity::find()
            .filter(Column::Address.eq(address))
            .one(&*self.db)
            .await?;
        Ok(record)
    }

    pub async fn update_name(&self, address: &str, name: &str) -> Result<()> {
        if let Some(existing) = self.find_by_address(address).await? {
            let mut am: ActiveModel = existing.into();
            am.name = Set(name.to_string());
            am.updated_at = Set(chrono::Utc::now().timestamp());
            am.update(&*self.db).await?;
        }
        Ok(())
    }

    pub async fn delete(&self, address: &str) -> Result<()> {
        Entity::delete_many()
            .filter(Column::Address.eq(address))
            .exec(&*self.db)
            .await?;
        Ok(())
    }
}
