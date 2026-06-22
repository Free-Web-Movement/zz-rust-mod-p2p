
use sea_orm::entity::prelude::*;
use chrono::Utc;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "global_price_model")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub resource: i16,
    pub url: String,
    pub js_handler: String,
    pub vendor: String,
    pub country: String,
    pub city: Option<String>,
    pub weight: i64,
    pub global_valid: bool,
    pub invalid: bool,
    pub last_success_timestamp: i64,
    pub consensus_count: i32,
    pub check_number: i32,
    pub check_success_number: i32,
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

