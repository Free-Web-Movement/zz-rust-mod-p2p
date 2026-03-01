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

    // pub async fn create_temp_session(&self) -> [u8; 16] {
    //     let mut session_id = [0u8; 16];
    //     OsRng.fill_bytes(&mut session_id);

    //     let session_key = SessionKey::new();

    //     self.temp_sessions
    //         .lock()
    //         .await
    //         .insert(session_id, session_key);

    //     session_id
    // }

    // pub async fn move_temp_to_permanent(
    //     &self,
    //     session_id: [u8; 16],
    //     address: String,
    // ) -> Result<()> {
    //     let mut temp_sessions = self.temp_sessions.lock().await;

    //     let mut session_key = temp_sessions
    //         .remove(&session_id)
    //         .ok_or_else(|| anyhow!("temp session not found"))?;

    //     // 迁移本身视为一次有效使用
    //     session_key.touch();

    //     self.session_keys.lock().await.insert(address, session_key);

    //     Ok(())
    // }

    // pub async fn cleanup_temp_sessions(&self, ttl_ms: u128) {
    //     self.temp_sessions
    //         .lock()
    //         .await
    //         .retain(|_, sk| !SystemTime::is_expired(sk.updated_at, ttl_ms));
    // }

    // pub async fn cleanup_sessions(&self, ttl_ms: u128) {
    //     self.session_keys
    //         .lock()
    //         .await
    //         .retain(|_, sk| !SystemTime::is_expired(sk.updated_at, ttl_ms));
    // }

    // pub async fn with_session<R>(
    //     &self,
    //     address: &str,
    //     f: impl FnOnce(&mut SessionKey) -> Result<R>,
    // ) -> Result<R> {
    //     let mut sessions = self.session_keys.lock().await;

    //     let sk = sessions
    //         .get_mut(address)
    //         .ok_or_else(|| anyhow!("session not found for address"))?;

    //     // 每次合法使用都 touch
    //     sk.touch();

    //     f(sk)
    // }

    // /// 完成 session 握手（ACK 阶段）
    // pub async fn session_establish(&self, address: &str, peer_public: &PublicKey) -> Result<()> {
    //     let mut sessions = self.session_keys.lock().await;

    //     let sk = sessions
    //         .get_mut(address)
    //         .ok_or_else(|| anyhow!("session not found for address"))?;

    //     sk.establish(peer_public)?;
    //     sk.touch();

    //     Ok(())
    // }

    // /// 使用 session 加密
    // pub async fn session_enc(&self, address: &str, plaintext: &[u8]) -> Result<Vec<u8>> {
    //     let mut sessions = self.session_keys.lock().await;

    //     let sk = sessions
    //         .get_mut(address)
    //         .ok_or_else(|| anyhow!("session not found for address"))?;

    //     let ct = sk.encrypt(plaintext)?;
    //     sk.touch();

    //     Ok(ct)
    // }

    // /// 使用 session 解密
    // pub async fn session_dec(&self, address: &str, data: &[u8]) -> Result<Vec<u8>> {
    //     let mut sessions = self.session_keys.lock().await;

    //     let sk = sessions
    //         .get_mut(address)
    //         .ok_or_else(|| anyhow!("session not found for address"))?;

    //     let pt = sk.decrypt(data)?;
    //     sk.touch();

    //     Ok(pt)
    // }
}