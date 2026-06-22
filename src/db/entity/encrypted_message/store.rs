use anyhow::{Result, anyhow};
use sea_orm::ConnectionTrait;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, ActiveValue::NotSet, QueryOrder, Set};
use sha2::{Digest, Sha256};

use super::entity::{ActiveModel, Column, Entity, Model};
use crate::consts::PRE_HASH;
use crate::db::defines::StoreFromConnection;

pub struct EncryptedMessageStore<'a, C: ConnectionTrait + Send + Sync> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait + Send + Sync> StoreFromConnection<'a, C>
    for EncryptedMessageStore<'a, C>
{
    fn new(db: &'a C) -> Self {
        Self { db }
    }
}

fn compute_hash(
    msg_id: &str,
    from: &str,
    to: &str,
    content: &[u8],
    created_at: i64,
    pre_hash: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(msg_id.as_bytes());
    hasher.update(from.as_bytes());
    hasher.update(to.as_bytes());
    hasher.update(content);
    hasher.update(created_at.to_le_bytes());
    hasher.update(pre_hash.as_bytes());
    format!("{:x}", hasher.finalize())
}

impl<'a, C: ConnectionTrait + Send + Sync> EncryptedMessageStore<'a, C> {
    pub async fn insert(
        &self,
        msg_id: &str,
        from_address: &str,
        to_address: &str,
        encrypted_content: Vec<u8>,
        nonce: Vec<u8>,
        ephemeral_pk: Vec<u8>,
    ) -> Result<Model> {
        let now = chrono::Utc::now().timestamp();
        let size_bytes = encrypted_content.len() as i64;

        let pre_hash_str = PRE_HASH.as_str();
        let hash = compute_hash(
            msg_id,
            from_address,
            to_address,
            &encrypted_content,
            now,
            pre_hash_str,
        );

        let am = ActiveModel {
            id: NotSet,
            msg_id: Set(msg_id.to_string()),
            from_address: Set(from_address.to_string()),
            to_address: Set(to_address.to_string()),
            encrypted_content: Set(encrypted_content),
            nonce: Set(nonce),
            ephemeral_pk: Set(ephemeral_pk),
            created_at: Set(now),
            size_bytes: Set(size_bytes),
            status: Set("unread".to_string()),
            deleted_by_sender: Set(false),
            deleted_by_receiver: Set(false),
            pre_hash: Set(pre_hash_str.to_string()),
            hash: Set(hash.clone()),
        };
        let model = am.insert(&*self.db).await?;
        Ok(model)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<Model>> {
        let record = Entity::find_by_id(id).one(&*self.db).await?;
        Ok(record)
    }

    pub async fn get_by_msg_id(&self, msg_id: &str) -> Result<Option<Model>> {
        let record = Entity::find()
            .filter(Column::MsgId.eq(msg_id))
            .one(&*self.db)
            .await?;
        Ok(record)
    }

    pub async fn get_for_recipient(&self, address: &str) -> Result<Vec<Model>> {
        let records = Entity::find()
            .filter(Column::ToAddress.eq(address))
            .filter(Column::DeletedByReceiver.eq(false))
            .order_by_asc(Column::CreatedAt)
            .all(&*self.db)
            .await?;
        Ok(records)
    }

    pub async fn get_sent(&self, address: &str) -> Result<Vec<Model>> {
        let records = Entity::find()
            .filter(Column::FromAddress.eq(address))
            .filter(Column::DeletedBySender.eq(false))
            .order_by_desc(Column::CreatedAt)
            .all(&*self.db)
            .await?;
        Ok(records)
    }

    pub async fn get_unread_count(&self, address: &str) -> Result<u64> {
        let count = Entity::find()
            .filter(Column::ToAddress.eq(address))
            .filter(Column::Status.eq("unread"))
            .count(&*self.db)
            .await?;
        Ok(count)
    }

    pub async fn mark_read(&self, msg_id: &str, recipient: &str) -> Result<()> {
        let record = Entity::find()
            .filter(Column::MsgId.eq(msg_id))
            .filter(Column::ToAddress.eq(recipient))
            .one(&*self.db)
            .await?
            .ok_or_else(|| anyhow!("message not found"))?;

        let mut am: ActiveModel = record.into();
        am.status = Set("read".to_string());
        am.update(&*self.db).await?;
        Ok(())
    }

    pub async fn mark_read_all(&self, address: &str) -> Result<()> {
        let records = Entity::find()
            .filter(Column::ToAddress.eq(address))
            .filter(Column::Status.eq("unread"))
            .all(&*self.db)
            .await?;
        for record in records {
            let mut am: ActiveModel = record.into();
            am.status = Set("read".to_string());
            am.update(&*self.db).await?;
        }
        Ok(())
    }

    pub async fn delete_by_sender(&self, msg_id: &str, sender: &str) -> Result<()> {
        let record = Entity::find()
            .filter(Column::MsgId.eq(msg_id))
            .filter(Column::FromAddress.eq(sender))
            .one(&*self.db)
            .await?
            .ok_or_else(|| anyhow!("message not found"))?;

        let mut am: ActiveModel = record.into();
        am.deleted_by_sender = Set(true);
        am.update(&*self.db).await?;
        Ok(())
    }

    pub async fn delete_by_receiver(&self, msg_id: &str, receiver: &str) -> Result<()> {
        let record = Entity::find()
            .filter(Column::MsgId.eq(msg_id))
            .filter(Column::ToAddress.eq(receiver))
            .one(&*self.db)
            .await?
            .ok_or_else(|| anyhow!("message not found"))?;

        let mut am: ActiveModel = record.into();
        am.deleted_by_receiver = Set(true);
        am.update(&*self.db).await?;
        Ok(())
    }

    pub async fn get_total_size_for_address(&self, address: &str) -> Result<i64> {
        let records = Entity::find()
            .filter(Column::ToAddress.eq(address))
            .all(&*self.db)
            .await?;
        let total: i64 = records.iter().map(|r| r.size_bytes).sum();
        Ok(total)
    }

    pub async fn get_message_count_for_epoch(
        &self,
        epoch: i64,
        address: &str,
    ) -> Result<(i64, i64)> {
        let start_ts = epoch * 86400;
        let end_ts = start_ts + 86400;

        let all = Entity::find()
            .filter(Column::FromAddress.eq(address))
            .filter(Column::CreatedAt.gte(start_ts))
            .filter(Column::CreatedAt.lt(end_ts))
            .all(&*self.db)
            .await?;

        let count = all.len() as i64;
        let total_size: i64 = all.iter().map(|r| r.size_bytes).sum();

        Ok((count, total_size))
    }

    pub async fn reset(&self) -> Result<()> {
        let backend = self.db.get_database_backend();
        let stmt = sea_orm::Statement::from_string(
            backend,
            r#"
            DELETE FROM "encrypted_message";
            DELETE FROM sqlite_sequence WHERE name='encrypted_message';
            "#
            .to_string(),
        );
        self.db.execute(stmt).await?;
        Ok(())
    }
}
