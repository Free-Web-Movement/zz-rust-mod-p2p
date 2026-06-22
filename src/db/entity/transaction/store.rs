use anyhow::Result;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, EntityTrait, QueryOrder, QuerySelect, Set};
use sha2::{Digest, Sha256};

use crate::consts::PRE_HASH;
use crate::db::defines::StoreFromConnection;
use crate::db::entity::transaction::entity::{ActiveModel, Column, Entity, Model, TxType};

use sea_orm::Statement;

pub fn generate_tx_id() -> String {
    use uuid::Uuid;
    Uuid::new_v4().to_string()
}
/// ----------------------
/// Transaction Store
/// ----------------------

pub struct TransactionStore<'a, C: ConnectionTrait + Send + Sync> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait + Send + Sync> StoreFromConnection<'a, C> for TransactionStore<'a, C> {
    fn new(db: &'a C) -> Self {
        Self { db }
    }
}

impl<'a, C: ConnectionTrait + Send + Sync> TransactionStore<'a, C> {
    /// 获取最后一条交易的 hash
    pub async fn last_hash(&self) -> Result<String> {
        let record = Entity::find()
            .order_by_desc(Column::Id)
            .one(&*self.db)
            .await?;
        Ok(record
            .map(|r| r.hash)
            .unwrap_or_else(|| PRE_HASH.to_string()))
    }

    /// 查询某分片的所有交易
    pub async fn get_by_shard(&self, shard_key: &str) -> Result<Vec<Model>> {
        let records = Entity::find()
            .filter(Column::ShardKey.eq(shard_key))
            .order_by_asc(Column::Timestamp)
            .all(&*self.db)
            .await?;
        Ok(records)
    }

    pub async fn get_all(&self) -> Result<Vec<Model>> {
        Ok(Entity::find()
            .order_by_asc(Column::Id)
            .all(&*self.db)
            .await?)
    }

    /// 分页查询（使用 SQL LIMIT/OFFSET，不加载全表到内存）
    pub async fn get_page(&self, page: u64, per_page: u64) -> Result<Vec<Model>> {
        let offset = (page.saturating_sub(1)) * per_page;
        Ok(Entity::find()
            .order_by_desc(Column::Id)
            .offset(offset)
            .limit(per_page)
            .all(&*self.db)
            .await?)
    }

    /// 总交易数
    pub async fn count_all(&self) -> Result<u64> {
        Ok(Entity::find().count(&*self.db).await? as u64)
    }

    /// 重置交易表
    pub async fn reset(&self) -> Result<()> {
        let backend = self.db.get_database_backend();

        let stmt = Statement::from_string(
            backend,
            r#"
        DELETE FROM "transaction";
        DELETE FROM sqlite_sequence WHERE name='transaction';
        "#
            .to_string(),
        );

        // 使用泛型 ConnectionTrait，不限制 DatabaseConnection
        self.db.execute(stmt).await?;

        Ok(())
    }

    /// 插入交易，同时生成链式 hash
    pub async fn insert_with_next_hash(
        &self,
        epoch: i64,
        cycle: i64,
        tx_type: TxType,
        from: &str,
        to: &str,
        amount: i64,
        shard_key: &str,
    ) -> Result<String> {
        let prev_hash = self.last_hash().await?;
        let ts = chrono::Utc::now().timestamp();

        // 计算 hash
        let mut hasher = Sha256::new();
        hasher.update(prev_hash.as_bytes());
        hasher.update(epoch.to_le_bytes());
        hasher.update(cycle.to_le_bytes());
        hasher.update(tx_type.clone().into_value());
        hasher.update(from.as_bytes());
        hasher.update(to.as_bytes());
        hasher.update(&amount.to_string().into_bytes());
        let hash = format!("{:x}", hasher.finalize());

        let am = ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            pre_hash: Set(prev_hash),
            epoch: Set(epoch),
            cycle: Set(cycle),
            tx_type: Set(tx_type),
            from_address: Set(from.to_string()),
            to_address: Set(to.to_string()),
            amount: Set(amount.to_string()),
            timestamp: Set(ts),
            hash: Set(hash.clone()),
            shard_key: Set(shard_key.to_string()),
        };

        am.insert(&*self.db).await?;
        Ok(hash)
    }
}
