use anyhow::Result;
use ed25519_dalek::{
    SigningKey,
    VerifyingKey,
    Signature,
    Signer,
    Verifier,
};
use rand::rngs::OsRng;
use sha2::{Sha256, Digest};

/// 本地私钥（不可 Clone）
pub struct IdentityKey {
    signing_key: SigningKey,
}

/// 可公开传播的身份
#[derive(Clone, Debug)]
pub struct IdentityPublic {
    pub verifying_key: VerifyingKey,
    pub address: [u8; 20],
}

impl IdentityKey {
    /// 生成新身份（2.2 正确方式）
    pub fn generate() -> Self {
        let mut rng = OsRng;
        let signing_key = SigningKey::generate(&mut rng);
        Self { signing_key }
    }

    /// 从 32 字节私钥恢复
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(bytes);
        Self { signing_key }
    }

    /// 导出私钥（用于磁盘）
    pub fn to_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    /// 获取公开身份
    pub fn public(&self) -> IdentityPublic {
        let verifying_key = self.signing_key.verifying_key();
        let address = derive_address(&verifying_key);
        IdentityPublic {
            verifying_key,
            address,
        }
    }

    /// 签名
    pub fn sign(&self, data: &[u8]) -> Signature {
        self.signing_key.sign(data)
    }
}

impl IdentityPublic {
    /// 验证签名
    pub fn verify(&self, data: &[u8], sig: &Signature) -> Result<()> {
        self.verifying_key.verify(data, sig)?;
        Ok(())
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }
}

/// 地址派生（pubkey → address）
fn derive_address(key: &VerifyingKey) -> [u8; 20] {
    let hash = Sha256::digest(key.to_bytes());
    let mut out = [0u8; 20];
    out.copy_from_slice(&hash[..20]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn test_identity_generate_and_public() {
        let identity = IdentityKey::generate();

        let bytes = identity.to_bytes();
        let sign_key = IdentityKey::from_bytes(&bytes);
        assert_eq!(bytes, sign_key.to_bytes());
        let public = identity.public();

        // 验证地址长度
        assert_eq!(public.address.len(), 20);

        // 公钥字节长度固定
        let pk_bytes = public.to_bytes();
        assert_eq!(pk_bytes.len(), 32);
    }

    #[test]
    fn test_sign_and_verify_success() {
        let identity = IdentityKey::generate();
        let public = identity.public();

        let msg = b"hello free web movement";
        let sig = identity.sign(msg);

        // 正确数据 + 正确签名 → 必须通过
        assert!(public.verify(msg, &sig).is_ok());
    }

    #[test]
    fn test_verify_failure_on_modified_message() {
        let identity = IdentityKey::generate();
        let public = identity.public();

        let msg = b"original message";
        let sig = identity.sign(msg);

        // 修改数据
        let bad_msg = b"modified message";

        // 必须失败
        assert!(public.verify(bad_msg, &sig).is_err());
    }

    #[test]
    fn test_verify_failure_on_wrong_key() {
        let identity1 = IdentityKey::generate();
        let identity2 = IdentityKey::generate();

        let public2 = identity2.public();

        let msg = b"cross key test";
        let sig1 = identity1.sign(msg);

        // 用错误的公钥验证
        assert!(public2.verify(msg, &sig1).is_err());
    }

    #[test]
    fn test_address_derivation_is_deterministic() {
        let mut rng = OsRng;
        let signing_key = SigningKey::generate(&mut rng);
        let verifying_key = signing_key.verifying_key();

        let addr1 = super::derive_address(&verifying_key);
        let addr2 = super::derive_address(&verifying_key);

        // 同一个公钥 → 地址必须一致
        assert_eq!(addr1, addr2);
    }

    #[test]
    fn test_different_keys_produce_different_addresses() {
        let mut rng = OsRng;

        let key1 = SigningKey::generate(&mut rng).verifying_key();
        let key2 = SigningKey::generate(&mut rng).verifying_key();

        let addr1 = super::derive_address(&key1);
        let addr2 = super::derive_address(&key2);

        assert_ne!(addr1, addr2);
    }
}

