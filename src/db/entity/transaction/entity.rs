use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum TxType {
    #[sea_orm(string_value = "transfer")]
    Transfer,

    #[sea_orm(string_value = "mint")]
    Mint,

    #[sea_orm(string_value = "burn")]
    Burn,
}

/// ----------------------
/// Transaction Entity
/// ----------------------
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "transaction")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub epoch: i64, // days pass after theom genesis day
    pub cycle: i64, // tick round count number after 00:00
    pub tx_type: TxType,
    pub from_address: String,
    pub to_address: String,
    pub amount: String,    // u64 以字符串存储
    pub timestamp: i64,    // UNIX 时间戳
    pub pre_hash: String,  // 上次交易的哈希
    pub hash: String,      // 本交易的哈希
    pub shard_key: String, // 分片标识
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match *self {}
    }
}

impl ActiveModelBehavior for ActiveModel {}
