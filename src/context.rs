use serde_json::Value;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use zz_account::address::FreeWebMovementAddress as Address;

use crate::nodes::clients::Clients;

pub struct Context {
    pub ip: String,
    pub port: u16,
    pub address: Address,
    pub token: CancellationToken,
    pub clients: Mutex<Clients>,
    pub global: Value,
    pub local: Value,
}

impl Context {
    pub fn new(ip: String, port: u16, address: Address) -> Self {
        Self {
            ip,
            port,
            address,
            token: CancellationToken::new(),
            clients: Mutex::new(Clients::new()),
            global: Value::Object(serde_json::Map::new()),
            local: Value::Object(serde_json::Map::new()),
        }
    }
}
