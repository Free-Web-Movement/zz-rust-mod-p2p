// store/transaction_set_store.rs
use crate::{
    consts::PRE_HASH,
    db::{
        defines::StoreFromConnection,
        entity::{
            transaction::entity::TxType,
            transaction_set::entity::{ActiveModel, Column, Entity, Model},
        },
    },
};
use anyhow::Result;
use sea_orm::*;
use sha2::{Digest, Sha256};

pub struct TransactionSetStore<'a, C: ConnectionTrait + Send + Sync> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait + Send + Sync> StoreFromConnection<'a, C>
    for TransactionSetStore<'a, C>
{
    fn new(db: &'a C) -> Self {
        Self { db }
    }
}

impl<'a, C: ConnectionTrait + Send + Sync> TransactionSetStore<'a, C> {
    pub async fn reset(&self) -> Result<()> {
        let backend = self.db.get_database_backend();

        let stmt = Statement::from_string(
            backend,
            r#"
        DELETE FROM "transaction_set";
        DELETE FROM sqlite_sequence WHERE name='transaction_set';
        "#
            .to_string(),
        );

        // 使用泛型 ConnectionTrait，不限制 DatabaseConnection
        self.db.execute(stmt).await?;

        Ok(())
    }

    pub async fn get_all(&self) -> Result<Vec<Model>> {
        Ok(Entity::find()
            .order_by_asc(Column::Id)
            .all(&*self.db)
            .await?)
    }

    pub async fn last_hash(&self) -> Result<String> {
        let record = Entity::find()
            .order_by_desc(Column::Id)
            .one(&*self.db)
            .await?;
        Ok(record
            .map(|r| r.hash)
            .unwrap_or_else(|| PRE_HASH.to_string()))
    }

    /// 插入交易集，同时生成链式 hash
    pub async fn insert_with_next_hash(
        &self,
        epoch: i64,
        cycle: i64,
        set_type: TxType,
        shard_key: &str,
        packers: &str,
        total_amount: u64,
        tx_hashes: Vec<String>,
    ) -> Result<String> {
        let pre_hash = self.last_hash().await?;

        // 生成交易集 hash
        let mut hasher = Sha256::new();
        hasher.update(pre_hash.as_bytes());
        hasher.update(epoch.to_le_bytes());
        hasher.update(cycle.to_le_bytes());
        for tx_hash in &tx_hashes {
            hasher.update(tx_hash.as_bytes());
        }
        hasher.update(packers);
        hasher.update(set_type.clone().into_value());
        hasher.update(total_amount.to_le_bytes());

        let hash = format!("{:x}", hasher.finalize());

        let am = ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            epoch: Set(epoch),
            cycle: Set(cycle),
            pre_hash: sea_orm::ActiveValue::Set(pre_hash),
            set_type: sea_orm::ActiveValue::Set(set_type),
            hash: sea_orm::ActiveValue::Set(hash.clone()),
            tx_hashes: sea_orm::ActiveValue::Set(serde_json::to_string(&tx_hashes)?),
            tx_hash_count: sea_orm::ActiveValue::Set(tx_hashes.len() as u32),
            total_amount: sea_orm::ActiveValue::Set(total_amount.to_string()),
            shard_key: sea_orm::ActiveValue::Set(shard_key.to_string()),
            packets: sea_orm::ActiveValue::Set("[]".to_string()),
            packers: sea_orm::ActiveValue::Set(packers.to_string()),
        };

        am.insert(&*self.db).await?;
        Ok(hash)
    }
}
