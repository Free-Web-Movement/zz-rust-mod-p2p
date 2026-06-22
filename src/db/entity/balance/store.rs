use anyhow::{Result, anyhow};
use sea_orm::ActiveValue::NotSet;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set, TransactionTrait};

use crate::db::defines::StoreFromConnection;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "balance")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub address: String,
    pub balance: String,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match *self {}
    }
}

impl ActiveModelBehavior for ActiveModel {}

pub struct BalanceStore<'a, C: ConnectionTrait + Send + Sync + 'a> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait + Send + Sync + 'a> StoreFromConnection<'a, C> for BalanceStore<'a, C> {
    fn new(db: &'a C) -> Self {
        Self { db }
    }
}

impl<'a, C: ConnectionTrait + Send + Sync + 'a> BalanceStore<'a, C> {
    /// 获取余额
    pub async fn get(&self, address: &str) -> Result<u64> {
        let record = Entity::find()
            .filter(Column::Address.eq(address.to_string()))
            .one(&*self.db)
            .await?;
        Ok(match record {
            Some(m) => m.balance.parse::<u64>()?,
            None => 0,
        })
    }

    /// 设置余额（覆盖）
    pub async fn set(&self, address: &str, amount: u64) -> Result<()> {
        let record = Entity::find()
            .filter(Column::Address.eq(address.to_string()))
            .one(&*self.db)
            .await?;

        match record {
            Some(existing) => {
                let mut am: ActiveModel = existing.into();
                am.balance = Set(amount.to_string());
                am.update(&*self.db).await?;
            }
            None => {
                let am = ActiveModel {
                    id: NotSet,
                    address: Set(address.to_string()),
                    balance: Set(amount.to_string()),
                    updated_at: Set(chrono::Utc::now().timestamp()),
                };
                am.insert(&*self.db).await?;
            }
        }
        Ok(())
    }

    /// 转账（使用事务保证原子性，避免竞态条件）
    pub async fn transfer(&self, from_addr: &str, to_addr: &str, amount: u64) -> Result<()>
    where
        C: TransactionTrait,
    {
        if amount == 0 {
            return Err(anyhow!("transfer amount must > 0"));
        }

        let tx = self.db.begin().await?;
        {
            let store = BalanceStore::new(&tx);

            let from_balance = store.get(from_addr).await?;
            if from_balance < amount {
                return Err(anyhow!("insufficient funds"));
            }

            let to_balance = store.get(to_addr).await?;
            store.set(from_addr, from_balance - amount).await?;
            store.set(to_addr, to_balance + amount).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_all(&self) -> anyhow::Result<Vec<(String, u64)>> {
        let rows = Entity::find().all(&*self.db).await?;

        Ok(rows
            .into_iter()
            .map(|m| (m.address, m.balance.parse::<u64>().unwrap_or(0)))
            .collect())
    }
}

/// ----------------------
/// 内置单元测试
/// ----------------------
#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, Schema};

    async fn setup_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();

        // 创建表
        let schema = Schema::new(sea_orm::DatabaseBackend::Sqlite);
        let mut entity = schema.create_table_from_entity(Entity);
        let stmt = entity.if_not_exists();
        db.execute(db.get_database_backend().build(stmt))
            .await
            .unwrap();

        db
    }

    #[tokio::test]
    async fn test_set_and_get() {
        let db = setup_db().await;
        let store = BalanceStore::new(&db);

        store.set("addr1", 1000).await.unwrap();
        let bal = store.get("addr1").await.unwrap();
        assert_eq!(bal, 1000);
    }

    #[tokio::test]
    async fn test_transfer_success() {
        let db = setup_db().await;
        let store = BalanceStore::new(&db);

        store.set("addr1", 1000).await.unwrap();
        store.set("addr2", 500).await.unwrap();

        store.transfer("addr1", "addr2", 300).await.unwrap();

        assert_eq!(store.get("addr1").await.unwrap(), 700);
        assert_eq!(store.get("addr2").await.unwrap(), 800);
    }

    #[tokio::test]
    async fn test_transfer_fail_insufficient() {
        let db = setup_db().await;
        let store = BalanceStore::new(&db);

        store.set("addr1", 100).await.unwrap();
        store.set("addr2", 50).await.unwrap();

        let res = store.transfer("addr1", "addr2", 200).await;
        assert!(res.is_err());

        assert_eq!(store.get("addr1").await.unwrap(), 100);
        assert_eq!(store.get("addr2").await.unwrap(), 50);
    }

    #[tokio::test]
    async fn test_upsert() {
        let db = setup_db().await;
        let store = BalanceStore::new(&db);

        store.set("addr1", 100).await.unwrap();
        store.set("addr1", 300).await.unwrap();

        let bal = store.get("addr1").await.unwrap();
        assert_eq!(bal, 300);
    }
}
