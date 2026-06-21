use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
struct EncryptedToken {
    nonce: String,
    ciphertext: String,
}

fn config_path() -> Result<PathBuf> {
    let base = dirs::config_dir()
        .context("cannot determine config directory")?;
    let dir = base.join("rdtool");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("config.json"))
}

fn derive_key() -> Result<[u8; 32]> {
    let machine_id = machine_uid::get().unwrap_or_else(|_| "rdtool-fallback".to_string());
    let seed = format!("{machine_id}rdtool-v1");
    let hash = blake3::hash(seed.as_bytes());
    Ok(*hash.as_bytes())
}

pub fn save_token(token: &str) -> Result<()> {
    let key_bytes = derive_key()?;
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, token.as_bytes())
        .map_err(|e| anyhow::anyhow!("encryption failed: {e}"))?;

    let stored = EncryptedToken {
        nonce: B64.encode(nonce),
        ciphertext: B64.encode(ciphertext),
    };
    let json = serde_json::to_string(&stored)?;
    std::fs::write(config_path()?, json)?;
    Ok(())
}

pub fn load_token() -> Result<String> {
    let path = config_path()?;
    let json = std::fs::read_to_string(&path)
        .context("no token stored")?;
    let stored: EncryptedToken = serde_json::from_str(&json)?;

    let key_bytes = derive_key()?;
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);

    let nonce_bytes = B64.decode(&stored.nonce)?;
    let ciphertext_bytes = B64.decode(&stored.ciphertext)?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext_bytes.as_ref())
        .map_err(|_| anyhow::anyhow!("decryption failed - token may be from a different machine"))?;

    Ok(String::from_utf8(plaintext)?)
}

pub fn clear_token() -> Result<()> {
    let path = config_path()?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}
