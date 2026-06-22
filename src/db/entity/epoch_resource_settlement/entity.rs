use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "epoch_resource_settlement")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    /* ---------- identity ---------- */
    pub node_id: String,

    /* ---------- epoch ---------- */
    pub epoch: i64, // day index

    /* ---------- resources ---------- */
    pub resources: String, // JSON Array of {resource_type, accumulated, weight}

    /* ---------- settlement ---------- */
    pub total_weight: u16, // 本 epoch 所有资源的总权值
    pub reward: u64,       // 本 epoch 分币奖励

    /* ---------- chain ---------- */
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
