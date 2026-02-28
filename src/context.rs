use aex::{crypto::zero_trust_session_key::SessionKey, time::SystemTime};
use anyhow::{Result, anyhow};
use rand::{RngCore, rngs::OsRng};
use serde_json::Value;
use x25519_dalek::PublicKey;

use std::collections::HashMap;
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
    pub session_keys: Mutex<HashMap<String, SessionKey>>,
    pub temp_sessions: Mutex<HashMap<[u8; 16], SessionKey>>, // 临时 session_id → SessionKey
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
            session_keys: Mutex::new(HashMap::new()),
            temp_sessions: Mutex::new(HashMap::new()),
        }
    }

    pub async fn create_temp_session(&self) -> [u8; 16] {
        let mut session_id = [0u8; 16];
        OsRng.fill_bytes(&mut session_id);

        let session_key = SessionKey::new();

        self.temp_sessions
            .lock()
            .await
            .insert(session_id, session_key);

        session_id
    }

    pub async fn move_temp_to_permanent(
        &self,
        session_id: [u8; 16],
        address: String,
    ) -> Result<()> {
        let mut temp_sessions = self.temp_sessions.lock().await;

        let mut session_key = temp_sessions
            .remove(&session_id)
            .ok_or_else(|| anyhow!("temp session not found"))?;

        // 迁移本身视为一次有效使用
        session_key.touch();

        self.session_keys.lock().await.insert(address, session_key);

        Ok(())
    }

    pub async fn cleanup_temp_sessions(&self, ttl_ms: u128) {
        self.temp_sessions
            .lock()
            .await
            .retain(|_, sk| !SystemTime::is_expired(sk.updated_at, ttl_ms));
    }

    pub async fn cleanup_sessions(&self, ttl_ms: u128) {
        self.session_keys
            .lock()
            .await
            .retain(|_, sk| !SystemTime::is_expired(sk.updated_at, ttl_ms));
    }

    pub async fn with_session<R>(
        &self,
        address: &str,
        f: impl FnOnce(&mut SessionKey) -> Result<R>,
    ) -> Result<R> {
        let mut sessions = self.session_keys.lock().await;

        let sk = sessions
            .get_mut(address)
            .ok_or_else(|| anyhow!("session not found for address"))?;

        // 每次合法使用都 touch
        sk.touch();

        f(sk)
    }

    /// 完成 session 握手（ACK 阶段）
    pub async fn session_establish(&self, address: &str, peer_public: &PublicKey) -> Result<()> {
        let mut sessions = self.session_keys.lock().await;

        let sk = sessions
            .get_mut(address)
            .ok_or_else(|| anyhow!("session not found for address"))?;

        sk.establish(peer_public)?;
        sk.touch();

        Ok(())
    }

    /// 使用 session 加密
    pub async fn session_enc(&self, address: &str, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut sessions = self.session_keys.lock().await;

        let sk = sessions
            .get_mut(address)
            .ok_or_else(|| anyhow!("session not found for address"))?;

        let ct = sk.encrypt(plaintext)?;
        sk.touch();

        Ok(ct)
    }

    /// 使用 session 解密
    pub async fn session_dec(&self, address: &str, data: &[u8]) -> Result<Vec<u8>> {
        let mut sessions = self.session_keys.lock().await;

        let sk = sessions
            .get_mut(address)
            .ok_or_else(|| anyhow!("session not found for address"))?;

        let pt = sk.decrypt(data)?;
        sk.touch();

        Ok(pt)
    }
}

#[cfg(test)]
mod tests {
    use crate::nodes::{net_info::NetInfo, storage::Storeage};

    use super::*;
    use tokio::time::{Duration, sleep};
    use x25519_dalek::{EphemeralSecret, PublicKey};

    pub fn dummy_context() -> Context {
        let address = Address::random(); // 如果没有 default，换成你真实构造方式

        // 1️⃣ 初始化 storage
        let storage = Storeage::new(None, None, None, None);

        // 2️⃣ 初始化 Servers（内部完成 external list 的 merge + persist）
        let servers = Servers::new(address.clone(), storage, NetInfo::new(8080));

        Context::new("127.0.0.1".to_string(), 8080, address, servers)
    }

    fn gen_keypair() -> (EphemeralSecret, PublicKey) {
        let secret = EphemeralSecret::random_from_rng(rand::rngs::OsRng);
        let public = PublicKey::from(&secret);
        (secret, public)
    }

    #[tokio::test]
    async fn test_create_and_move_temp_session() {
        let ctx = dummy_context();

        let session_id = ctx.create_temp_session().await;

        assert_eq!(session_id.len(), 16);

        let addr = "peer-1".to_string();

        ctx.move_temp_to_permanent(session_id, addr.clone())
            .await
            .expect("move temp -> permanent");

        let sessions = ctx.session_keys.lock().await;
        assert!(sessions.contains_key(&addr));
    }

    #[tokio::test]
    async fn test_move_temp_session_not_found() {
        let ctx = dummy_context();

        let fake_id = [1u8; 16];

        let err = ctx
            .move_temp_to_permanent(fake_id, "peer-x".to_string())
            .await
            .unwrap_err();

        assert!(err.to_string().contains("temp session not found"));
    }

    #[tokio::test]
    async fn test_session_establish_encrypt_decrypt() {
        let ctx = dummy_context();

        // temp session
        let session_id = ctx.create_temp_session().await;
        let addr = "peer-crypto".to_string();

        ctx.move_temp_to_permanent(session_id, addr.clone())
            .await
            .unwrap();

        // peer keypair
        let (_peer_secret, peer_public) = gen_keypair();

        // establish
        ctx.session_establish(&addr, &peer_public)
            .await
            .expect("establish");

        // encrypt
        let plaintext = b"hello secure world";
        let ciphertext = ctx.session_enc(&addr, plaintext).await.expect("encrypt");

        assert_ne!(ciphertext, plaintext);

        // decrypt
        let decrypted = ctx.session_dec(&addr, &ciphertext).await.expect("decrypt");

        assert_eq!(decrypted, plaintext);
    }

    #[tokio::test]
    async fn test_session_not_found_paths() {
        let ctx = dummy_context();

        let err = ctx.session_enc("missing", b"x").await.unwrap_err();
        assert!(err.to_string().contains("session not found"));

        let err = ctx.session_dec("missing", b"x").await.unwrap_err();
        assert!(err.to_string().contains("session not found"));

        let (_s, p) = gen_keypair();
        let err = ctx.session_establish("missing", &p).await.unwrap_err();
        assert!(err.to_string().contains("session not found"));
    }

    #[tokio::test]
    async fn test_cleanup_temp_sessions() {
        let ctx = dummy_context();

        let id = ctx.create_temp_session().await;

        // 等一会儿，确保 updated_at < now
        sleep(Duration::from_millis(2)).await;

        ctx.cleanup_temp_sessions(0).await;

        let map = ctx.temp_sessions.lock().await;
        assert!(!map.contains_key(&id));
    }

    #[tokio::test]
    async fn test_cleanup_permanent_sessions() {
        let ctx = dummy_context();

        let id = ctx.create_temp_session().await;
        let addr = "peer-expire".to_string();

        ctx.move_temp_to_permanent(id, addr.clone()).await.unwrap();

        sleep(Duration::from_millis(2)).await;

        ctx.cleanup_sessions(0).await;

        let map = ctx.session_keys.lock().await;
        assert!(!map.contains_key(&addr));
    }

    #[tokio::test]
    async fn test_with_session() {
        let ctx = dummy_context();

        let id = ctx.create_temp_session().await;
        let addr = "peer-with".to_string();

        ctx.move_temp_to_permanent(id, addr.clone()).await.unwrap();

        ctx.with_session(&addr, |sk| {
            // 尚未 establish，key 应为空
            assert!(sk.key.is_none());
            Ok(())
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_with_session_not_found() {
        let ctx = dummy_context();

        let err = ctx.with_session("nope", |_| Ok(())).await.unwrap_err();

        assert!(err.to_string().contains("session not found"));
    }

    #[tokio::test]
    async fn test_online_ack_with_session_id_binding() -> Result<()> {
        // A = 发起方，S = 接收方（服务器 / 对端）
        let ctx_a = dummy_context();
        let ctx_s = dummy_context();

        let addr_a = "addr-A".to_string();
        let addr_s = "addr-S".to_string();

        /* ---------------- A: create temp session ---------------- */

        let session_id = ctx_a.create_temp_session().await;

        let a_public = {
            let map = ctx_a.temp_sessions.lock().await;
            map.get(&session_id)
                .expect("A temp session exists")
                .ephemeral_public
        };

        /* ---------------- S: receive ONLINE ---------------- */
        /*
            关键点：
            - S 不能直接 move 到 permanent
            - S 必须也用 session_id 暂存
        */
        let sk_s = SessionKey::new();
        ctx_s.temp_sessions.lock().await.insert(session_id, sk_s);

        let s_public = {
            let map = ctx_s.temp_sessions.lock().await;
            map.get(&session_id)
                .expect("S temp session exists")
                .ephemeral_public
        };

        /* ---------------- S -> A : ONLINE_ACK ---------------- */
        /*
            ACK 内容语义上等价于：
            {
                session_id,
                address: S,
                public_key: s_public
            }
        */

        /* ---------------- A: receive ACK ---------------- */

        ctx_a
            .move_temp_to_permanent(session_id, addr_s.clone())
            .await
            .expect("A move temp -> permanent");

        ctx_a
            .session_establish(&addr_s, &s_public)
            .await
            .expect("A establish");

        /* ---------------- S: finalize after ACK ---------------- */

        ctx_s
            .move_temp_to_permanent(session_id, addr_a.clone())
            .await
            .expect("S move temp -> permanent");

        ctx_s
            .session_establish(&addr_a, &a_public)
            .await
            .expect("S establish");

        /* ---------------- verify secure channel ---------------- */

        let msg = b"hello via established session";
        let ct = ctx_a.session_enc(&addr_s, msg).await?;
        let pt = ctx_s.session_dec(&addr_a, &ct).await?;

        assert_eq!(pt, msg);

        Ok(())
    }
}
