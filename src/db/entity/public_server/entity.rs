use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "public_server")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub address: String,
    pub port: u16,
    pub historical_addresses: Option<String>,
    pub protocol: String,
    pub is_active: bool,
    pub last_seen: i64,
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match *self {}
    }
}

impl ActiveModelBehavior for ActiveModel {}
