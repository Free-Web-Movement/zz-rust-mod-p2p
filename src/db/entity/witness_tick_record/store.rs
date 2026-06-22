use crate::db::defines::StoreFromConnection;
use crate::db::entity::witness_tick_record::entity::{ActiveModel, Column, Entity, Model};
use anyhow::Result;
use sea_orm::ConnectionTrait;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, ActiveValue, QueryOrder, Set};

pub struct WitnessTickRecordStore<'a, C: ConnectionTrait + Send + Sync> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait + Send + Sync> StoreFromConnection<'a, C>
    for WitnessTickRecordStore<'a, C>
{
    fn new(db: &'a C) -> Self {
        Self { db }
    }
}

impl<'a, C: ConnectionTrait + Send + Sync> WitnessTickRecordStore<'a, C> {
    /// Increment tick count for a given (epoch, address) pair.
    /// Creates a new record with tick_count=1 if none exists.
    pub async fn increment_tick(&self, epoch: i64, address: &str) -> Result<()> {
        let now = chrono::Utc::now().timestamp();

        if let Some(existing) = Entity::find()
            .filter(Column::Epoch.eq(epoch))
            .filter(Column::Address.eq(address))
            .one(&*self.db)
            .await?
        {
            let mut am: ActiveModel = existing.into();
            let current = match &am.tick_count {
                ActiveValue::Set(v) => *v,
                ActiveValue::Unchanged(v) => *v,
                ActiveValue::NotSet => 0,
            };
            am.tick_count = Set(current + 1);
            am.updated_at = Set(now);
            am.update(&*self.db).await?;
        } else {
            let am = ActiveModel {
                id: sea_orm::ActiveValue::NotSet,
                epoch: Set(epoch),
                address: Set(address.to_string()),
                tick_count: Set(1),
                updated_at: Set(now),
            };
            am.insert(&*self.db).await?;
        }

        Ok(())
    }

    /// Get all tick records for a given epoch, ordered by tick_count descending.
    pub async fn get_epoch_records(&self, epoch: i64) -> Result<Vec<Model>> {
        let records = Entity::find()
            .filter(Column::Epoch.eq(epoch))
            .order_by(Column::TickCount, sea_orm::Order::Desc)
            .all(&*self.db)
            .await?;
        Ok(records)
    }

    /// Get the maximum tick_count for a given epoch.
    pub async fn get_max_tick_count(&self, epoch: i64) -> Result<u8> {
        let result = Entity::find()
            .filter(Column::Epoch.eq(epoch))
            .order_by(Column::TickCount, sea_orm::Order::Desc)
            .one(&*self.db)
            .await?;
        Ok(result.map(|r| r.tick_count).unwrap_or(0))
    }

    /// Get addresses with tick_count >= threshold for a given epoch.
    pub async fn get_full_time_addresses(&self, epoch: i64, threshold: u8) -> Result<Vec<String>> {
        let records = Entity::find()
            .filter(Column::Epoch.eq(epoch))
            .filter(Column::TickCount.gte(threshold))
            .all(&*self.db)
            .await?;
        Ok(records.into_iter().map(|r| r.address).collect())
    }

    /// Reset table (for testing).
    pub async fn reset(&self) -> Result<()> {
        let backend = self.db.get_database_backend();
        let stmt = sea_orm::Statement::from_string(
            backend,
            r#"
            DELETE FROM "witness_tick_record";
            DELETE FROM sqlite_sequence WHERE name='witness_tick_record';
            "#
            .to_string(),
        );
        self.db.execute(stmt).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{Database, DatabaseConnection};

    use super::*;

    async fn setup_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let sql = "
            CREATE TABLE IF NOT EXISTS witness_tick_record (
                id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
                epoch BIGINT NOT NULL,
                address TEXT NOT NULL,
                tick_count TINYINTEGER NOT NULL,
                updated_at BIGINT NOT NULL
            )
        ";
        db.execute(sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            sql.to_string(),
        ))
        .await
        .unwrap();
        db
    }

    #[tokio::test]
    async fn test_increment_tick_new() {
        let db = setup_db().await;
        let store = WitnessTickRecordStore::new(&db);

        store.increment_tick(1, "node1").await.unwrap();

        let records = store.get_epoch_records(1).await.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].tick_count, 1);
    }

    #[tokio::test]
    async fn test_increment_tick_existing() {
        let db = setup_db().await;
        let store = WitnessTickRecordStore::new(&db);

        store.increment_tick(1, "node1").await.unwrap();
        store.increment_tick(1, "node1").await.unwrap();
        store.increment_tick(1, "node1").await.unwrap();

        let records = store.get_epoch_records(1).await.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].tick_count, 3);
    }

    #[tokio::test]
    async fn test_increment_tick_multiple_nodes() {
        let db = setup_db().await;
        let store = WitnessTickRecordStore::new(&db);

        store.increment_tick(1, "node1").await.unwrap();
        store.increment_tick(1, "node1").await.unwrap();
        store.increment_tick(1, "node2").await.unwrap();

        let records = store.get_epoch_records(1).await.unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].tick_count, 2); // node1 has 2, ordered desc
        assert_eq!(records[1].tick_count, 1); // node2 has 1
    }

    #[tokio::test]
    async fn test_separate_epochs() {
        let db = setup_db().await;
        let store = WitnessTickRecordStore::new(&db);

        store.increment_tick(1, "node1").await.unwrap();
        store.increment_tick(2, "node1").await.unwrap();

        let records_epoch1 = store.get_epoch_records(1).await.unwrap();
        let records_epoch2 = store.get_epoch_records(2).await.unwrap();
        assert_eq!(records_epoch1.len(), 1);
        assert_eq!(records_epoch2.len(), 1);
    }

    #[tokio::test]
    async fn test_get_full_time_addresses() {
        let db = setup_db().await;
        let store = WitnessTickRecordStore::new(&db);

        store.increment_tick(1, "node1").await.unwrap();
        store.increment_tick(1, "node1").await.unwrap();
        store.increment_tick(1, "node2").await.unwrap();

        let full_time = store.get_full_time_addresses(1, 2).await.unwrap();
        assert_eq!(full_time.len(), 1);
        assert_eq!(full_time[0], "node1");
    }

    #[tokio::test]
    async fn test_get_max_tick_count() {
        let db = setup_db().await;
        let store = WitnessTickRecordStore::new(&db);

        store.increment_tick(1, "node1").await.unwrap();
        store.increment_tick(1, "node1").await.unwrap();
        store.increment_tick(1, "node2").await.unwrap();

        let max = store.get_max_tick_count(1).await.unwrap();
        assert_eq!(max, 2);
    }
}
