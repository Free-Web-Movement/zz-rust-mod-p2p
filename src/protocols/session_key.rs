use anyhow::{Result, anyhow};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, rand_core},
};
use rand_core::{OsRng, RngCore};
use serde_json::Value;

use std::collections::HashMap;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use x25519_dalek::{EphemeralSecret, PublicKey};
use zz_account::address::FreeWebMovementAddress as Address;

use crate::nodes::{connected_clients::ConnectedClients, servers::Servers};

pub struct SessionKey {
    pub key: Option<[u8; 32]>,                     // 对称 session_key
    pub ephemeral_secret: Option<EphemeralSecret>, // 一次性
    pub ephemeral_public: PublicKey,               // 可缓存
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
        }
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