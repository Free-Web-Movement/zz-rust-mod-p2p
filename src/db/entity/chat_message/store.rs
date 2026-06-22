use anyhow::Result;
use sea_orm::ConnectionTrait;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, ActiveValue::NotSet, QueryOrder, QuerySelect, Set};

use super::entity::{ActiveModel, Column, Entity, Model};
use crate::db::defines::StoreFromConnection;

pub struct ChatMessageStore<'a, C: ConnectionTrait + Send + Sync> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait + Send + Sync> StoreFromConnection<'a, C> for ChatMessageStore<'a, C> {
    fn new(db: &'a C) -> Self {
        Self { db }
    }
}

impl<'a, C: ConnectionTrait + Send + Sync> ChatMessageStore<'a, C> {
    pub async fn add(
        &self,
        contact_address: &str,
        content: &str,
        is_sent: bool,
        status: &str,
    ) -> Result<Model> {
        let now = chrono::Utc::now().timestamp();
        let am = ActiveModel {
            id: NotSet,
            contact_address: Set(contact_address.to_string()),
            content: Set(content.to_string()),
            is_sent: Set(is_sent),
            status: Set(status.to_string()),
            timestamp: Set(now),
        };
        let model = am.insert(&*self.db).await?;
        Ok(model)
    }

    pub async fn get_by_contact(&self, contact_address: &str) -> Result<Vec<Model>> {
        let records = Entity::find()
            .filter(Column::ContactAddress.eq(contact_address))
            .order_by_asc(Column::Timestamp)
            .all(&*self.db)
            .await?;
        Ok(records)
    }

    pub async fn get_recent(&self, limit: u64) -> Result<Vec<Model>> {
        let records = Entity::find()
            .order_by_desc(Column::Timestamp)
            .limit(limit)
            .all(&*self.db)
            .await?;
        Ok(records)
    }

    pub async fn get_unread_count(&self, contact_address: &str) -> Result<u64> {
        let count = Entity::find()
            .filter(Column::ContactAddress.eq(contact_address))
            .filter(Column::IsSent.eq(false))
            .filter(Column::Status.eq("received"))
            .count(&*self.db)
            .await?;
        Ok(count)
    }

    pub async fn mark_as_read(&self, contact_address: &str) -> Result<()> {
        let records = Entity::find()
            .filter(Column::ContactAddress.eq(contact_address))
            .filter(Column::IsSent.eq(false))
            .filter(Column::Status.eq("received"))
            .all(&*self.db)
            .await?;
        for record in records {
            let mut am: ActiveModel = record.into();
            am.status = Set("read".to_string());
            am.update(&*self.db).await?;
        }
        Ok(())
    }

    pub async fn get_all_contacts_with_unread(&self) -> Result<Vec<(String, u64)>> {
        let records = Entity::find()
            .filter(Column::IsSent.eq(false))
            .filter(Column::Status.eq("received"))
            .all(&*self.db)
            .await?;
        let mut map: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
        for r in records {
            *map.entry(r.contact_address).or_insert(0) += 1;
        }
        let mut result: Vec<(String, u64)> = map.into_iter().collect();
        result.sort_by(|a, b| b.1.cmp(&a.1));
        Ok(result)
    }

    /// 获取所有会话列表，按地址分组，包含最后一条消息和时间
    pub async fn get_conversations(&self) -> Result<Vec<ConversationSummary>> {
        let records = Entity::find()
            .order_by_desc(Column::Timestamp)
            .all(&*self.db)
            .await?;
        let mut groups: std::collections::HashMap<String, Vec<Model>> =
            std::collections::HashMap::new();
        for r in records {
            groups.entry(r.contact_address.clone()).or_default().push(r);
        }
        let mut result: Vec<ConversationSummary> = groups
            .into_iter()
            .map(|(addr, msgs)| {
                let unread = msgs
                    .iter()
                    .filter(|m| !m.is_sent && m.status == "received")
                    .count() as u64;
                let last = msgs.first().unwrap(); // sorted desc, so first is latest
                ConversationSummary {
                    address: addr,
                    last_message: last.content.clone(),
                    last_timestamp: last.timestamp,
                    unread_count: unread,
                }
            })
            .collect();
        result.sort_by(|a, b| b.last_timestamp.cmp(&a.last_timestamp));
        Ok(result)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConversationSummary {
    pub address: String,
    pub last_message: String,
    pub last_timestamp: i64,
    pub unread_count: u64,
}
