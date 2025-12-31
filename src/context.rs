use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce, aead::Aead};
use rand::{RngCore, rngs::OsRng};
use serde_json::Value;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use x25519_dalek::{ EphemeralSecret, PublicKey };
use zz_account::address::FreeWebMovementAddress as Address;
use std::collections::HashMap;

use crate::nodes::{ connected_clients::ConnectedClients, servers::Servers };

pub struct SessionKey {
    pub key: [u8; 32], // 对称 session_key，用于消息加解密
    pub ephemeral_secret: EphemeralSecret, // 临时私钥，只用于 KeyExchange
    pub ephemeral_public: PublicKey, // 临时公钥，避免重复生成
}

impl SessionKey {
    pub fn new(key: [u8; 32]) -> Self {
        let mut rng = OsRng;
        let ephemeral_secret = EphemeralSecret::random_from_rng(&mut rng);
        let ephemeral_public = PublicKey::from(&ephemeral_secret);
        Self { key, ephemeral_secret, ephemeral_public }
    }

    /// 使用对称 session_key 加密消息
    pub fn encrypt(&self, plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
        let cipher = XChaCha20Poly1305::new(&self.key.into());
        let mut nonce_bytes = [0u8; 24];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = XNonce::from_slice(&nonce_bytes);
        let mut ciphertext = cipher.encrypt(nonce, plaintext).unwrap();
        // 将 nonce 拼接在前面，方便解密
        let mut out = nonce_bytes.to_vec();
        out.append(&mut ciphertext);
        Ok(out)
    }

    /// 使用对称 session_key 解密消息
    pub fn decrypt(&self, ciphertext: &[u8]) -> anyhow::Result<Vec<u8>> {
        if ciphertext.len() < 24 {
            anyhow::bail!("Ciphertext too short");
        }
        let (nonce_bytes, ciphertext) = ciphertext.split_at(24);
        let cipher = XChaCha20Poly1305::new(&self.key.into());
        let nonce = XNonce::from_slice(nonce_bytes);
        let plaintext = cipher.decrypt(nonce, ciphertext).unwrap();
        Ok(plaintext)
    }
}

pub struct Context {
    pub ip: String,
    pub port: u16,
    pub address: Address,
    pub token: CancellationToken,
    pub clients: Mutex<ConnectedClients>,
    pub global: Value,
    pub local: Value,
    pub servers: Mutex<Servers>,
    /// session_keys: key = peer_addr / 节点地址
    /// 每个 session 包含对称密钥 + 临时私钥 + 临时公钥
    pub session_keys: Mutex<HashMap<String, SessionKey>>,
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
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_key_encrypt_decrypt() {
        let key_bytes = [1u8; 32];
        let session = SessionKey::new(key_bytes);

        let plaintext = b"Hello, this is a test message!";
        let ciphertext = session.encrypt(plaintext).unwrap();
        let decrypted = session.decrypt(&ciphertext).unwrap();

        assert_eq!(plaintext.to_vec(), decrypted);
        println!("✅ Encryption/Decryption successful!");
    }

    #[tokio::test]
    async fn test_ephemeral_keys_unique() {
        let key_bytes = [2u8; 32];
        let session1 = SessionKey::new(key_bytes);
        let session2 = SessionKey::new(key_bytes);

        assert_ne!(session1.ephemeral_public.to_bytes(), session2.ephemeral_public.to_bytes());
        println!("✅ Ephemeral keys are unique per session");
    }
}
