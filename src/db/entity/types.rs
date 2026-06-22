use sea_orm::{DeriveActiveEnum, EnumIter};
use serde::{Deserialize, Serialize};
use std::hash::Hash;

#[repr(u8)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumIter, DeriveActiveEnum,
)]
#[sea_orm(rs_type = "i16", db_type = "SmallInteger")]
pub enum ResourceType {
    Cpu = 1,
    Bandwidth = 2,
    Ip = 3,
    Storage = 4,
    Api = 5,
    Memory = 6,
    HotStorage = 7,
    ColdStorage = 8,
    ApiRequest = 9,
    Unknown = 0,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "i8", db_type = "TinyInteger")]
pub enum PricingUnit {
    TimeBased = 1, // IP / Storage
    QuantityBased = 2, // Bandwidth / API
                   // Hybrid = 3,         // CPU / GPU
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "i8", db_type = "TinyInteger")]
pub enum ResourceUnit {
    Ip = 1,
    Gb = 2,
    Tb = 3,
    Mbps = 4,
    Core = 5,
    Ghz = 6,
    Request = 7,
}
