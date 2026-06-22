use anyhow::{Result, anyhow};

use once_cell::sync::Lazy;

const GENESIS_APPLIED_KEY: &str = "genesis_applied";
const NETWORK_TYPE_KEY: &str = "network_type";

pub enum MetaKey {
    GenesisApplied = 0,
    NetworkType,
}

impl MetaKey {
    pub const fn as_str(&self) -> &'static str {
        match self {
            MetaKey::GenesisApplied => GENESIS_APPLIED_KEY,
            MetaKey::NetworkType => NETWORK_TYPE_KEY,
        }
    }

    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            GENESIS_APPLIED_KEY => Ok(MetaKey::GenesisApplied),
            _ => Err(anyhow!("unknown meta key: {}", s)),
        }
    }
}

pub static META_KEYS: Lazy<Vec<&'static str>> =
    Lazy::new(|| vec![MetaKey::GenesisApplied.as_str()]);
