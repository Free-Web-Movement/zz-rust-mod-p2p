use aex::crypto::session_key_manager::PairedSessionKey;
use serde_json::Value;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use zz_account::address::FreeWebMovementAddress as Address;

use crate::{
    nodes::{connected_clients::ConnectedClients, servers::Servers},
};

pub struct Context {
    pub ip: String,
    pub port: u16,
    pub address: Address,
    pub token: CancellationToken,
    pub clients: Mutex<ConnectedClients>,
    pub global: Value,
    pub local: Value,
    pub servers: Mutex<Servers>,
    pub paired_session_keys: PairedSessionKey,
}

impl Context {
    pub fn new(ip: String, port: u16, address: Address, servers: Servers) -> Self {
        Self {
            ip,
            port,
            address,
            token: CancellationToken::new(),
            clients: Mutex::new(ConnectedClients::new()),
            global: Value::Object(serde_json::Map::new()),
            local: Value::Object(serde_json::Map::new()),
            servers: Mutex::new(servers),
            paired_session_keys: PairedSessionKey::new(16)
        }
    }

}