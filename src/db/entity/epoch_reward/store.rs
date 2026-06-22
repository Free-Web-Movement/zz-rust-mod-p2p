use anyhow::Result;
use sea_orm::ActiveValue::NotSet;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set};

use crate::db::defines::StoreFromConnection;

pub struct EpochRewardStore<'a, C: ConnectionTrait + Send + Sync + 'a> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait + Send + Sync + 'a> StoreFromConnection<'a, C>
    for EpochRewardStore<'a, C>
{
    fn new(db: &'a C) -> Self {
        Self { db }
    }
}

impl<'a, C: ConnectionTrait + Send + Sync + 'a> EpochRewardStore<'a, C> {
    pub async fn insert(
        &self,
        epoch_id: i64,
        address: &str,
        amount: &str,
    ) -> Result<super::entity::Model> {
        let am = super::entity::ActiveModel {
            id: NotSet,
            epoch_id: Set(epoch_id),
            address: Set(address.to_string()),
            amount: Set(amount.to_string()),
        };
        am.insert(&*self.db).await.map_err(anyhow::Error::from)
    }

    pub async fn get_by_epoch(&self, epoch_id: i64) -> Result<Vec<super::entity::Model>> {
        let records = super::entity::Entity::find()
            .filter(super::entity::Column::EpochId.eq(epoch_id))
            .all(&*self.db)
            .await?;
        Ok(records)
    }

    pub async fn get_by_address_and_epoch(
        &self,
        epoch_id: i64,
        address: &str,
    ) -> Result<Option<super::entity::Model>> {
        let record = super::entity::Entity::find()
            .filter(super::entity::Column::EpochId.eq(epoch_id))
            .filter(super::entity::Column::Address.eq(address))
            .one(&*self.db)
            .await?;
        Ok(record)
    }

    pub async fn get_total_reward_for_epoch(&self, epoch_id: i64) -> Result<u64> {
        let records = self.get_by_epoch(epoch_id).await?;
        let total = records
            .iter()
            .filter_map(|r| r.amount.parse::<u64>().ok())
            .sum();
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::Database;
    use tempfile::TempDir;

    use super::*;
    use crate::db::store::DatabaseStore;

    #[tokio::test]
    async fn test_insert_and_query() {
        let dir = TempDir::new().expect("temp dir");
        let db_path = dir.path().join("test.db");
        let url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let db = Database::connect(&url).await.expect("connect db");
        DatabaseStore::<DatabaseConnection>::create_all_table(&db)
            .await
            .expect("create tables");

        let store = DatabaseStore::store::<EpochRewardStore<DatabaseConnection>>(&db).await;

        // Insert rewards for two addresses in epoch 0
        store.insert(0, "addr1", "1000").await.unwrap();
        store.insert(0, "addr2", "2000").await.unwrap();
        store.insert(1, "addr1", "500").await.unwrap();

        // Query by epoch
        let epoch0 = store.get_by_epoch(0).await.unwrap();
        assert_eq!(epoch0.len(), 2);

        let epoch1 = store.get_by_epoch(1).await.unwrap();
        assert_eq!(epoch1.len(), 1);

        // Query by address + epoch
        let r = store
            .get_by_address_and_epoch(0, "addr1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(r.amount, "1000");

        let r = store
            .get_by_address_and_epoch(0, "addr2")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(r.amount, "2000");

        // Address not in epoch
        let r = store
            .get_by_address_and_epoch(0, "nonexistent")
            .await
            .unwrap();
        assert!(r.is_none());

        // Total reward for epoch 0
        let total = store.get_total_reward_for_epoch(0).await.unwrap();
        assert_eq!(total, 3000);

        let total = store.get_total_reward_for_epoch(1).await.unwrap();
        assert_eq!(total, 500);

        // Empty epoch
        let total = store.get_total_reward_for_epoch(99).await.unwrap();
        assert_eq!(total, 0);
    }
}
