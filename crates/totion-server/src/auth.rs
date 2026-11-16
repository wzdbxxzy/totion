use anyhow::Result;
use chrono::{Duration, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use rand_core::{OsRng, RngCore};
use rusqlite::{params, Connection, OptionalExtension};
use totion_core::User;

use crate::db;

pub const SESSION_DAYS: i64 = 30;
pub const CHALLENGE_SECS: i64 = 300;

pub fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    OsRng.fill_bytes(&mut buf);
    hex::encode(buf)
}

pub fn generate_invite_code() -> String {
    let mut buf = [0u8; 4];
    OsRng.fill_bytes(&mut buf);
    hex::encode_upper(buf)
}

/// 确定邀请码：环境变量 > 数据库 > 新生成。结果写回数据库。
pub fn init_invite(conn: &Connection, env_code: Option<&str>) -> Result<String> {
    let code = match env_code.map(str::trim).filter(|c| !c.is_empty()) {
        Some(code) => code.to_string(),
        None => match db::get_meta(conn, "invite_code")? {
            Some(existing) => existing,
            None => generate_invite_code(),
        },
    };
    db::set_meta(conn, "invite_code", &code)?;
    db::ensure_invite(conn, &code, "初始邀请码", "server")?;
    Ok(code)
}

pub fn create_session(conn: &Connection, user_id: &str) -> Result<String> {
    let token = db::new_token();
    let expires_at = (Utc::now() + Duration::days(SESSION_DAYS)).to_rfc3339();
    conn.execute(
        "INSERT INTO sessions(token, user_id, expires_at) VALUES (?1, ?2, ?3)",
        params![token, user_id, expires_at],
    )?;
    Ok(token)
}

pub fn user_for_token(conn: &Connection, token: &str) -> Option<User> {
    conn.query_row(
        "SELECT u.id, u.name, u.public_key, u.created_at
         FROM sessions s JOIN users u ON u.id = s.user_id
         WHERE s.token = ?1 AND s.expires_at > ?2",
        params![token, db::now_str()],
        db::row_to_user,
    )
    .optional()
    .ok()
    .flatten()
}

pub fn find_user_by_key(conn: &Connection, public_key: &str) -> Option<User> {
    conn.query_row(
        "SELECT id, name, public_key, created_at FROM users WHERE public_key = ?1",
        params![public_key],
        db::row_to_user,
    )
    .optional()
    .ok()
    .flatten()
}

pub fn verify_signature(public_key_hex: &str, message: &[u8], signature_hex: &str) -> bool {
    let Ok(key_bytes) = hex::decode(public_key_hex) else {
        return false;
    };
    let key_array: [u8; 32] = match key_bytes.try_into() {
        Ok(array) => array,
        Err(_) => return false,
    };
    let Ok(verifying_key) = VerifyingKey::from_bytes(&key_array) else {
        return false;
    };
    let Ok(signature_bytes) = hex::decode(signature_hex) else {
        return false;
    };
    let Ok(signature) = Signature::from_slice(&signature_bytes) else {
        return false;
    };
    verifying_key.verify(message, &signature).is_ok()
}

pub fn cleanup_expired(conn: &Connection) {
    let now = db::now_str();
    let _ = conn.execute("DELETE FROM sessions WHERE expires_at <= ?1", params![now]);
    let _ = conn.execute(
        "DELETE FROM challenges WHERE expires_at <= ?1",
        params![now],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn signs_and_verifies() {
        let key = SigningKey::generate(&mut OsRng);
        let public_key = hex::encode(key.verifying_key().to_bytes());
        let message = b"totion-challenge";
        let signature = hex::encode(key.sign(message).to_bytes());

        assert!(verify_signature(&public_key, message, &signature));
        assert!(!verify_signature(
            &public_key,
            b"another-message",
            &signature
        ));
        assert!(!verify_signature("not-hex", message, &signature));
    }
}
