use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "node")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub ipv4: String,
    pub ipv6: String,
    pub max_bandwidth: u64,
    pub actual_bandwidth: u64,
    pub min_ping: u32,
    pub online_seconds: i64,
    pub trust_score: u32,
    pub daily_online_rate: u8,
    pub monthly_online_rate: u8,
    pub yearly_online_rate: u8,
    pub ip_changed: bool,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match *self {}
    }
}

impl ActiveModelBehavior for ActiveModel {}
