use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub const NETWORK_TEST: &str = "test";
pub const NETWORK_GRAY: &str = "gray";
pub const NETWORK_MAIN: &str = "main";
pub const NETWORK_FUTURE: &str = "future";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkType {
    Test,
    Gray,
    Main,
    Future,
}

impl NetworkType {
    pub fn as_str(&self) -> &'static str {
        match self {
            NetworkType::Test => NETWORK_TEST,
            NetworkType::Gray => NETWORK_GRAY,
            NetworkType::Main => NETWORK_MAIN,
            NetworkType::Future => NETWORK_FUTURE,
        }
    }

    pub fn is(&self, v: NetworkType) -> bool {
        *self == v
    }
}

impl FromStr for NetworkType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            NETWORK_TEST => Ok(NetworkType::Test),
            NETWORK_GRAY => Ok(NetworkType::Gray),
            NETWORK_MAIN => Ok(NetworkType::Main),
            NETWORK_FUTURE => Ok(NetworkType::Future),
            _ => Err(anyhow!("unknown network state: {}", s)),
        }
    }
}
