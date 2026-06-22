use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "tick_resource_snapshot")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    /* ---------- identity ---------- */
    pub node_id: String, // 节点标识

    /* ---------- time ---------- */
    pub epoch: i64, // 当前 epoch（天索引）
    pub tick: i64,  // tick 索引（15 分钟一格，表示最后完成的 tick）

    /* ---------- resource ---------- */
    pub market_resource_id: i64, // 指向 market_resource_price 的资源 ID

    /* ---------- resource data ---------- */
    pub accumulated: i64, // epoch 内所有已完成 tick 的累积使用量

    /* ---------- verification ---------- */
    pub verified: bool, // 是否已通过全网共识

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
