use std::collections::HashMap;

use serde_json::Value;

use tokio_util::sync::CancellationToken;
use zz_account::address::FreeWebMovementAddress as Address;

use crate::defines::ProtocolType;

pub struct Context {
    pub(crate) ip: String,
    pub(crate) port: u16,
    pub(crate) address: Address,
    pub(crate) token: CancellationToken,
    pub(crate) clients: HashMap<String, ProtocolType>,
    pub(crate) global: Value,
    pub(crate) local: Value,
}

impl Context {
    pub fn new(ip: String, port: u16, address: Address) -> Self {
        Self {
            ip,
            port,
            address,
            token: CancellationToken::new(),
            clients: HashMap::new(),
            global: Value::Object(serde_json::Map::new()),
            local: Value::Object(serde_json::Map::new()),
        }
    }
}
