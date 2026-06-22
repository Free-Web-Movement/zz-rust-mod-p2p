use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "epoch")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub day: i64,
    pub start_time: i64,
    pub end_time: Option<i64>,
    pub tick_count: u8,
    pub total_reward: String,
    pub settled: bool,
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
