use anyhow::{Result, anyhow};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, rand_core},
};
use rand_core::{OsRng, RngCore};
use x25519_dalek::{EphemeralSecret, PublicKey};

use crate::util::time::timestamp;

pub struct SessionKey {
    pub key: Option<[u8; 32]>,                     // 对称 session_key
    pub ephemeral_secret: Option<EphemeralSecret>, // 一次性
    pub ephemeral_public: PublicKey,               // 可缓存
    pub created_at: u128,
    pub updated_at: u128,
}

impl SessionKey {
    pub fn new() -> Self {
        let mut rng = OsRng;
        let secret = EphemeralSecret::random_from_rng(&mut rng);
        let public = PublicKey::from(&secret);

        Self {
            key: None,
            ephemeral_secret: Some(secret),
            ephemeral_public: public,
            created_at: timestamp(),
            updated_at: timestamp()
        }
    }

        #[inline]
    pub fn touch(&mut self) {
        self.updated_at = timestamp();
    }

    #[inline]
    pub fn is_expired(&self, ttl_ms: u128) -> bool {
        timestamp().saturating_sub(self.updated_at) > ttl_ms
    }

    pub fn establish(&mut self, peer_public: &PublicKey) -> Result<()> {
        let secret = self
            .ephemeral_secret
            .take()
            .ok_or_else(|| anyhow!("session already established"))?;

        let shared = secret.diffie_hellman(peer_public);

        let mut key = [0u8; 32];
        key.copy_from_slice(&shared.as_bytes()[..32]);
        self.key = Some(key);

        Ok(())
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let key = self.key.ok_or_else(|| anyhow!("session not established"))?;
        let cipher = XChaCha20Poly1305::new(&key.into());

        let mut nonce_bytes = [0u8; 24];
        OsRng.fill_bytes(&mut nonce_bytes);

        let nonce = XNonce::from_slice(&nonce_bytes);
        let ct = cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| anyhow!("encrypt failed"))?;

        Ok([nonce_bytes.to_vec(), ct].concat())
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        let key = self.key.ok_or_else(|| anyhow!("session not established"))?;

        if data.len() < 24 {
            return Err(anyhow!("ciphertext too short"));
        }

        let (nonce_bytes, ct) = data.split_at(24);
        let cipher = XChaCha20Poly1305::new(&key.into());

        let nonce = XNonce::from_slice(nonce_bytes);
        cipher
            .decrypt(nonce, ct)
            .map_err(|_| anyhow!("decrypt failed"))
    }
}