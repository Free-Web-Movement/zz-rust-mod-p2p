
use sea_orm::entity::prelude::*;
use chrono::Utc;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "local_price_model")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub resource: i16,
    pub url: String,
    pub js_handler: String,
    pub vendor: String,
    pub country: String,
    pub city: Option<String>,
    pub last_success_timestamp: i64,
    pub weight: i64,
    pub valid: bool,
    pub fail_count: i32,
    pub created_at: i64,
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

