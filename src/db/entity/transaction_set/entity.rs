use sea_orm::entity::prelude::*;

use sea_orm::{ActiveModelBehavior, DeriveEntityModel, EnumIter, RelationDef};

use crate::db::entity::transaction::entity::TxType;

/// ----------------------
/// 交易集合表 tx_sets
/// ----------------------

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "transaction_set")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub epoch: i64,       // days pass after theom genesis day
    pub cycle: i64,       // tick round count number after 00:00
    pub set_type: TxType, // Genesis | Mint | Packed
    pub pre_hash: String,
    pub hash: String,
    pub tx_hashes: String, // ordered, deterministic tx_hashes, in JSON Array format
    pub tx_hash_count: u32,
    pub total_amount: String,
    pub shard_key: String, // YYYY-MM-DD
    pub packets: String,   // packet data
    pub packers: String, // addresses of nodes that as witnesses, should be hashed as well, in JSON Array format
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match *self {}
    }
}

impl ActiveModelBehavior for ActiveModel {}
