use serde_json::Value;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use zz_account::address::FreeWebMovementAddress as Address;

use crate::nodes::connected_clients::ConnectedClients;

pub struct Context {
    pub ip: String,
    pub port: u16,
    pub address: Address,
    pub token: CancellationToken,
    pub clients: Mutex<ConnectedClients>,
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
            clients: Mutex::new(ConnectedClients::new()),
            global: Value::Object(serde_json::Map::new()),
            local: Value::Object(serde_json::Map::new()),
        }
    }
}
