use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction, DbErr, EntityTrait,
    Schema, Statement, TransactionTrait,
    sea_query::{MysqlQueryBuilder, PostgresQueryBuilder, SqliteQueryBuilder},
};
use std::sync::Arc;

use crate::{create_entities, db::defines::StoreFromConnection};

/// 多线程安全的数据库总 Store
#[derive(Clone)]
pub struct DatabaseStore<'a, C: ConnectionTrait + Send + Sync> {
    pub conn: &'a C,
}

pub type DbConn = Arc<dyn ConnectionTrait + Send + Sync>;

impl<'a, C: ConnectionTrait + Send + Sync> DatabaseStore<'a, C> {
    /// 使用 Arc<DatabaseConnection> 初始化
    pub fn new(conn: &'a C) -> Self {
        Self { conn }
    }

    pub async fn with_transaction<F, T>(db: &'a C, f: F) -> Result<(), anyhow::Error>
    where
        F: FnOnce(&DatabaseTransaction) -> futures::future::BoxFuture<anyhow::Result<T>>,
        C: TransactionTrait,
    {
        let txn = db.begin().await?;
        let res: Result<T, anyhow::Error> = f(&txn).await;

        match res {
            Ok(_) => {
                txn.commit().await?;
                Ok(())
            }
            Err(e) => {
                txn.rollback().await?;
                Err(e)
            }
        }
    }

    pub async fn store<S>(db: &'a C) -> S
    where
        S: StoreFromConnection<'a, C>,
        C: ConnectionTrait + Send + Sync,
    {
        S::new(db)
    }

    /// 根据 Entity 自动创建表

    pub async fn create_table<E>(db: &DatabaseConnection, entity: E) -> Result<(), DbErr>
    where
        E: EntityTrait,
    {
        let backend = db.get_database_backend();
        let schema = Schema::new(backend);

        // ✅ 第一步：先生成 TableCreateStatement（拥有值）
        let mut table_stmt = schema.create_table_from_entity(entity);

        // ✅ 第二步：再调用 builder 方法
        table_stmt.if_not_exists();

        // ✅ 第三步：转 SQL
        let sql = match backend {
            DatabaseBackend::Sqlite => table_stmt.to_string(SqliteQueryBuilder),
            DatabaseBackend::MySql => table_stmt.to_string(MysqlQueryBuilder),
            DatabaseBackend::Postgres => table_stmt.to_string(PostgresQueryBuilder),
        };

        let stmt = Statement::from_string(backend, sql);
        db.execute(stmt).await?;

        Ok(())
    }

    /// 批量创建你项目里所有表
    pub async fn create_all_table(db: &DatabaseConnection) -> Result<(), DbErr> {
        create_entities!(
            db,
            (
                crate::db::entity::meta::entity::Entity,
                crate::db::entity::balance::entity::Entity,
                crate::db::entity::transaction::entity::Entity,
                crate::db::entity::transaction_set::entity::Entity,
                crate::db::entity::public_server::entity::Entity,
                crate::db::entity::witness_tick_record::entity::Entity,
                crate::db::entity::epoch::entity::Entity,
                crate::db::entity::epoch_reward::entity::Entity,
                crate::db::entity::epoch_allocation::entity::Entity,
            )
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::*;
    use sea_orm::{ActiveModelTrait, ColumnTrait, Database, EntityTrait, QueryFilter, Set};
    use tempfile::TempDir;

    use crate::db;
    use crate::db::entity::balance::entity as balance_entity;
    use crate::db::entity::meta::entity as meta_entity;
    use crate::db::entity::transaction::entity as tx_entity;
    use crate::db::entity::transaction_set::entity as txset_entity;
    use chrono::Utc;

    #[tokio::test]
    async fn test_sqlite_create_and_crud() -> anyhow::Result<()> {
        // 1️⃣ 创建临时 SQLite 文件
        let tmp_dir = TempDir::new()?;
        let db_path = tmp_dir.path().join("test.db");
        // let db_url = format!("sqlite://{}", db_path.to_str().unwrap());
        // ⚠️ SQLite 必须加 ?mode=rwc，否则临时文件可能无法写入
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());

        let conn = Database::connect(&db_url).await?;

        // 2️⃣ 创建所有表
        db::store::DatabaseStore::<DatabaseConnection>::create_all_table(&conn).await?;

        // 3️⃣ 测试写入 meta 表
        let meta_am = meta_entity::ActiveModel {
            key: Set("test_key".to_string()),
            value: Set("test_value".to_string()),
            ..Default::default()
        };
        meta_am.insert(&conn).await?;

        let meta_rows = meta_entity::Entity::find().all(&conn).await?;
        assert_eq!(meta_rows.len(), 1);
        assert_eq!(meta_rows[0].key, "test_key");

        // 4️⃣ 测试写入 balance 表
        let balance_am = balance_entity::ActiveModel {
            address: Set("addr1".to_string()),
            balance: Set("1000".to_string()),
            updated_at: Set(chrono::Utc::now().timestamp()),
            ..Default::default()
        };
        balance_am.insert(&conn).await?;

        let balance_rows = balance_entity::Entity::find().all(&conn).await?;
        assert_eq!(balance_rows.len(), 1);
        assert_eq!(balance_rows[0].balance, "1000".to_string());

        // 5️⃣ 测试写入 transaction 表
        let tx_am = tx_entity::ActiveModel {
            epoch: Set(0),
            cycle: Set(0),
            from_address: Set("addr1".to_string()),
            to_address: Set("addr2".to_string()),
            amount: Set("500".to_string()),
            tx_type: Set(tx_entity::TxType::Mint),
            timestamp: Set(Utc::now().timestamp()),
            hash: Set("hash1".to_string()),
            pre_hash: Set("pre_hash".to_string()),
            shard_key: Set("shard1".to_string()),
            ..Default::default()
        };
        tx_am.insert(&conn).await?;

        let tx_rows = tx_entity::Entity::find().all(&conn).await?;
        assert_eq!(tx_rows.len(), 1);
        assert_eq!(tx_rows[0].tx_type, tx_entity::TxType::Mint);

        // 6️⃣ 测试写入 transaction_set 表
        let txset_am = txset_entity::ActiveModel {
            epoch: Set(0),
            cycle: Set(0),
            pre_hash: Set("pre_hash".to_string()),
            hash: Set("hash_set".to_string()),
            tx_hashes: Set(serde_json::to_string(&vec!["tx1".to_string()])?),
            tx_hash_count: Set(1),
            total_amount: Set("500".to_string()),
            set_type: Set(tx_entity::TxType::Mint),
            shard_key: Set("shard1".to_string()),
            packets: Set("[]".to_string()),
            packers: Set("addr1".to_string()),
            ..Default::default()
        };
        txset_am.insert(&conn).await?;

        let txset_rows = txset_entity::Entity::find().all(&conn).await?;
        assert_eq!(txset_rows.len(), 1);
        assert_eq!(txset_rows[0].set_type, tx_entity::TxType::Mint);

        println!("SQLite test DB located at: {}", db_path.display());

        // =============================
        // 7️⃣ 测试 with_transaction —— commit
        // =============================
        DatabaseStore::<DatabaseConnection>::with_transaction(&conn, |txn| {
            Box::pin(async move {
                (meta_entity::ActiveModel {
                    key: Set("tx_commit_key".to_string()),
                    value: Set("tx_commit_value".to_string()),
                    ..Default::default()
                })
                .insert(txn)
                .await?;

                Ok(())
            })
        })
        .await?;

        // =============================
        // 8️⃣ 测试 with_transaction —— rollback
        // =============================
        let result: Result<(), _> =
            DatabaseStore::<DatabaseConnection>::with_transaction(&conn, |txn| {
                Box::pin(async move {
                    (meta_entity::ActiveModel {
                        key: Set("tx_rollback_key".to_string()),
                        value: Set("tx_rollback_value".to_string()),
                        ..Default::default()
                    })
                    .insert(txn)
                    .await?;

                    // ❌ 主动制造错误
                    Err::<(), anyhow::Error>(anyhow::anyhow!("force rollback"))
                })
            })
            .await;

        assert!(result.is_err());

        let rolled_back = meta_entity::Entity::find()
            .filter(meta_entity::Column::Key.eq("tx_rollback_key"))
            .all(&conn)
            .await?;
        assert!(rolled_back.is_empty());

        Ok(())
    }
}
