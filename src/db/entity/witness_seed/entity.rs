use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "witness_seed")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub ip: String,
    pub port: i32,
    pub node_id: String,
    pub added_at: i64,
    pub last_seen: i64,
    pub is_active: bool,
    pub success_count: i32,
    pub failure_count: i32,
    pub online_days: i64,
    pub offline_days: i64,
    pub online_rate: f64,
    pub is_connectable: bool,
    pub inactive_days: i64,
    pub is_intranet: bool,
    pub metadata: String,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match *self {}
    }
}

impl ActiveModelBehavior for ActiveModel {}
