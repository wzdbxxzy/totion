use std::path::PathBuf;

use anyhow::{Context, Result};
use ed25519_dalek::SigningKey;
use rand_core::OsRng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub server_url: String,
    pub name: String,
    pub public_key: String,
    pub secret_key: String,
    pub user_id: String,
    #[serde(default)]
    pub token: Option<String>,
}

impl Identity {
    pub fn generate(server_url: &str, name: &str) -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self {
            server_url: server_url.trim_end_matches('/').to_string(),
            name: name.trim().to_string(),
            public_key: hex::encode(signing_key.verifying_key().to_bytes()),
            secret_key: hex::encode(signing_key.to_bytes()),
            user_id: uuid::Uuid::new_v4().to_string(),
            token: None,
        }
    }

    pub fn signing_key(&self) -> Result<SigningKey> {
        let bytes = hex::decode(&self.secret_key).context("私钥 hex 解码失败")?;
        let array: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("私钥长度不是 32 字节"))?;
        Ok(SigningKey::from_bytes(&array))
    }

    pub fn sign(&self, message: &[u8]) -> Result<String> {
        use ed25519_dalek::Signer;
        Ok(hex::encode(self.signing_key()?.sign(message).to_bytes()))
    }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("totion")
        .join("identity.json")
}

pub fn load_identity() -> Result<Option<Identity>> {
    let path = config_path();
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("读取身份文件失败: {}", path.display()))?;
    let identity = serde_json::from_str(&raw).context("解析身份文件失败")?;
    Ok(Some(identity))
}

pub fn save_identity(identity: &Identity) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(identity)?;
    std::fs::write(&path, raw).with_context(|| format!("写入身份文件失败: {}", path.display()))?;
    Ok(())
}
