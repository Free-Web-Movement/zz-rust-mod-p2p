use crate::db::defines::StoreFromConnection;
use crate::db::entity::witness_seed::entity::{ActiveModel, Column, Entity, Model};
use anyhow::Result;
use sea_orm::ConnectionTrait;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, QueryOrder, Set};

pub struct WitnessSeedStore<'a, C: ConnectionTrait + Send + Sync> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait + Send + Sync> StoreFromConnection<'a, C> for WitnessSeedStore<'a, C> {
    fn new(db: &'a C) -> Self {
        Self { db }
    }
}

impl<'a, C: ConnectionTrait + Send + Sync> WitnessSeedStore<'a, C> {
    /// 根据 ip + port 查找 seed
    pub async fn find_by_ip_port(&self, ip: &str, port: i32) -> Result<Option<Model>> {
        let record = Entity::find()
            .filter(Column::Ip.eq(ip))
            .filter(Column::Port.eq(port))
            .one(&*self.db)
            .await?;
        Ok(record)
    }

    /// 插入或更新 seed 记录
    pub async fn upsert(
        &self,
        ip: &str,
        port: i32,
        node_id: &str,
        is_intranet: bool,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp();

        if let Some(existing) = self.find_by_ip_port(ip, port).await? {
            let was_active = existing.is_active;
            let success_count = existing.success_count;
            let online_days = existing.online_days;
            let offline_days = existing.offline_days;

            let mut am: ActiveModel = existing.into();
            // Only update last_seen when node comes online from offline
            if !was_active {
                am.last_seen = Set(now);
            }
            am.is_active = Set(true);
            am.success_count = Set(success_count + 1);
            am.online_days = Set(online_days + 1);
            am.online_rate = Set((online_days + 1) as f64 / (online_days + offline_days) as f64);
            am.update(&*self.db).await?;
        } else {
            // 插入新记录
            let am = ActiveModel {
                id: sea_orm::ActiveValue::NotSet,
                ip: Set(ip.to_string()),
                port: Set(port),
                node_id: Set(node_id.to_string()),
                added_at: Set(now),
                last_seen: Set(now),
                is_active: Set(true),
                success_count: Set(1),
                failure_count: Set(0),
                online_days: Set(1),
                offline_days: Set(0),
                online_rate: Set(1.0),
                is_connectable: Set(true),
                inactive_days: Set(0),
                is_intranet: Set(is_intranet),
                metadata: Set("{}".to_string()),
            };
            am.insert(&*self.db).await?;
        }

        Ok(())
    }

    /// 获取所有活跃的 seeds
    pub async fn get_active_seeds(&self) -> Result<Vec<Model>> {
        let records = Entity::find()
            .filter(Column::IsActive.eq(true))
            .order_by_asc(Column::AddedAt)
            .all(&*self.db)
            .await?;
        Ok(records)
    }

    /// 清空所有 seeds（用于全量同步替换）
    pub async fn clear_all(&self) -> Result<()> {
        let backend = self.db.get_database_backend();
        let sql = format!("DELETE FROM \"{}\"", "witness_seed");
        let stmt = sea_orm::Statement::from_string(backend, sql);
        self.db.execute(stmt).await?;
        Ok(())
    }

    /// 重置表（测试用）
    pub async fn reset(&self) -> Result<()> {
        let backend = self.db.get_database_backend();

        let stmt = sea_orm::Statement::from_string(
            backend,
            r#"
        DELETE FROM "witness_seed";
        DELETE FROM sqlite_sequence WHERE name='witness_seed';
        "#
            .to_string(),
        );

        self.db.execute(stmt).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::db::defines::StoreFromConnection;
    use crate::db::entity::witness_seed::store::WitnessSeedStore;
    use sea_orm::{ConnectionTrait, Database, DatabaseConnection};

    async fn setup_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let create_sql = "
            CREATE TABLE IF NOT EXISTS witness_seed (
                id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
                ip TEXT NOT NULL,
                port INTEGER NOT NULL,
                node_id TEXT NOT NULL,
                added_at BIGINT NOT NULL,
                last_seen BIGINT NOT NULL,
                is_active BOOLEAN NOT NULL,
                success_count INTEGER NOT NULL,
                failure_count INTEGER NOT NULL,
                online_days BIGINT NOT NULL,
                offline_days BIGINT NOT NULL,
                online_rate REAL NOT NULL,
                is_connectable BOOLEAN NOT NULL,
                inactive_days BIGINT NOT NULL,
                is_intranet BOOLEAN NOT NULL,
                metadata TEXT NOT NULL
            )
        ";
        db.execute(sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            create_sql.to_string(),
        ))
        .await
        .unwrap();
        db
    }

    #[tokio::test]
    async fn test_upsert_and_get_active() {
        let db = setup_db().await;
        let store = WitnessSeedStore::new(&db);

        // 插入
        store
            .upsert("192.168.1.100", 9000, "node1", true)
            .await
            .unwrap();

        // 查询活跃 seeds
        let seeds = store.get_active_seeds().await.unwrap();
        assert_eq!(seeds.len(), 1);
        assert_eq!(seeds[0].ip, "192.168.1.100");
        assert_eq!(seeds[0].port, 9000);
        assert!(seeds[0].is_intranet);
    }
}
