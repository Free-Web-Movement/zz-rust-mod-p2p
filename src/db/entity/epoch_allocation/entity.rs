use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "epoch_allocation")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub epoch: i64,
    pub round: i64,
    pub address: String,
    pub weight: String,
    pub amount: String,
    pub pre_hash: String,
    pub hash: String,
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
