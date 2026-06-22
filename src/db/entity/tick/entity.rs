use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "tick")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub epoch: i64,
    pub tick_index: i64,
    pub cycle: i64,
    pub timestamp: i64,
    pub status: String,
    pub pre_hash: String,
    pub hash: String,
    pub tx_hash: String,
    pub snapshot: Option<String>,
}
#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match *self {}
    }
}

impl ActiveModelBehavior for ActiveModel {}
