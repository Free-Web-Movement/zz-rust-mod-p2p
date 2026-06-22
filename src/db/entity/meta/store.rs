use std::str::FromStr;

use crate::{db::defines::StoreFromConnection, network_type::NetworkType};

use super::entity as meta;
use super::key::MetaKey;
use anyhow::{Result, anyhow};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};

pub struct MetaStore<'a, C: ConnectionTrait + Send + Sync> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait + Send + Sync> StoreFromConnection<'a, C> for MetaStore<'a, C> {
    fn new(db: &'a C) -> Self {
        Self { db }
    }
}

impl<'a, C: ConnectionTrait + Send + Sync> MetaStore<'a, C> {
    /// 设置 meta（覆盖式）
    pub async fn set(&self, key: MetaKey, value: &str) -> Result<()> {
        // 先尝试查找已有记录
        if let Some(existing) = meta::Entity::find()
            .filter(meta::Column::Key.eq(key.as_str().to_string()))
            .one(&*self.db)
            .await?
        {
            // 覆盖旧值
            let mut am: meta::ActiveModel = existing.into();
            am.value = Set(value.to_string());
            am.update(&*self.db).await?;
        } else {
            // 新增
            let model = meta::ActiveModel {
                key: Set(key.as_str().to_string()),
                value: Set(value.to_string()),
                ..Default::default() // id 会自增
            };
            model.insert(&*self.db).await?;
        }

        Ok(())
    }

    /// 读取 meta
    pub async fn get(&self, key: MetaKey) -> Result<Option<String>> {
        let record = meta::Entity::find()
            .filter(meta::Column::Key.eq(key.as_str().to_string()))
            .one(&*self.db)
            .await?;

        Ok(record.map(|m| m.value))
    }

    /// 判断某个 meta 是否存在
    pub async fn exists(&self, key: MetaKey) -> Result<bool> {
        Ok(self.get(key).await?.is_some())
    }

    /// ⚠️ 创世专用：检查并标记（不使用事务）
    pub async fn mark_genesis_applied(&self) -> Result<()> {
        // 原子查找
        if let Some(_) = meta::Entity::find()
            .filter(meta::Column::Key.eq(MetaKey::GenesisApplied.as_str().to_string()))
            .one(&*self.db)
            .await?
        {
            return Err(anyhow!("genesis already applied"));
        }

        // 插入创世标记
        let model = meta::ActiveModel {
            key: Set(MetaKey::GenesisApplied.as_str().to_string()),
            value: Set("true".to_string()),
            ..Default::default() // id 会自增
        };

        model.insert(&*self.db).await?;

        Ok(())
    }

    /// ⚠️ 只能调用一次：设置网络状态
    pub async fn set_network(&self, network: NetworkType) -> Result<()> {
        // 已存在则拒绝
        if let Some(_) = meta::Entity::find()
            .filter(meta::Column::Key.eq(MetaKey::NetworkType.as_str().to_string()))
            .one(&*self.db)
            .await?
        {
            return Err(anyhow!("network already set"));
        }

        let model = meta::ActiveModel {
            key: Set(MetaKey::NetworkType.as_str().to_string()),
            value: Set(network.as_str().to_string()),
            ..Default::default()
        };

        model.insert(&*self.db).await?;

        Ok(())
    }
    pub async fn get_network(&self) -> Result<NetworkType> {
        let record = meta::Entity::find()
            .filter(meta::Column::Key.eq(MetaKey::NetworkType.as_str().to_string()))
            .one(&*self.db)
            .await?
            .ok_or_else(|| anyhow!("network not set"))?;

        NetworkType::from_str(&record.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use sea_orm::ConnectionTrait;
    use sea_orm::{Database, DatabaseConnection, Schema};

    use crate::db::entity::meta::entity::Entity as MetaEntity;

    pub async fn setup_test_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();

        let schema = Schema::new(sea_orm::DatabaseBackend::Sqlite);
        let stmt = schema.create_table_from_entity(MetaEntity);

        db.execute(db.get_database_backend().build(&stmt))
            .await
            .unwrap();

        db
    }

    async fn setup() -> DatabaseConnection {
        setup_test_db().await
    }

    #[tokio::test]
    async fn test_set_and_get_meta() {
        let db = setup().await;
        let store = MetaStore::new(&db);

        store.set(MetaKey::GenesisApplied, "true").await.unwrap();

        let value = store.get(MetaKey::GenesisApplied).await.unwrap();
        assert_eq!(value, Some("true".to_string()));
    }

    #[tokio::test]
    async fn test_exists_meta() {
        let db = setup().await;
        let store = MetaStore::new(&db);

        assert!(!store.exists(MetaKey::GenesisApplied).await.unwrap());

        store.set(MetaKey::GenesisApplied, "true").await.unwrap();

        assert!(store.exists(MetaKey::GenesisApplied).await.unwrap());
    }

    #[tokio::test]
    async fn test_mark_genesis_applied_once() {
        let db = setup().await;
        let store = MetaStore::new(&db);

        // 第一次：成功
        store.mark_genesis_applied().await.unwrap();

        // 确认写入
        let value = store.get(MetaKey::GenesisApplied).await.unwrap();
        assert_eq!(value, Some("true".to_string()));
    }

    #[tokio::test]
    async fn test_mark_genesis_applied_twice_fails() {
        let db = setup().await;
        let store = MetaStore::new(&db);

        store.mark_genesis_applied().await.unwrap();

        // 第二次：必须失败
        let err = store.mark_genesis_applied().await.unwrap_err();

        assert!(
            err.to_string().contains("genesis already applied"),
            "unexpected error: {err}"
        );
    }
}
