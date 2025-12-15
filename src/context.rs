use std::collections::HashMap;

use serde_json::Value;

use zz_account::address::FreeWebMovementAddress as Address;

use crate::defines::ProtocolType;


pub struct Context {
    pub(crate) address: Address,
    pub(crate) clients: HashMap<String, ProtocolType>,
    pub(crate) global: Value,
    pub(crate) local: Value,
}

impl Context {
    pub fn new(address: Address) -> Self {
        Self {
            address,
            clients: HashMap::new(),
            global: Value::Object(serde_json::Map::new()),
            local: Value::Object(serde_json::Map::new()),
        }
    }
}
