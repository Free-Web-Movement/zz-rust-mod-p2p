use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "tick_witness_check")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub tick_id: i64,
    pub witness_id: i64,
    pub peer_witness_id: i64,
    pub result: bool,
    pub pre_hash: String,
    pub hash: String,
}
#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match *self {}
    }
}

impl ActiveModelBehavior for ActiveModel {}
