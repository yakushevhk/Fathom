use anyhow::{anyhow, bail, Result};
use base64::Engine as _;
use ring::{aead::{self, Aad, LessSafeKey, Nonce, UnboundKey}, rand::{SecureRandom, SystemRandom}};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const MAX_NAME: usize = 128;
const MAX_KIND: usize = 64;
const MAX_SECRET: usize = 65_536;
const NONCE_LEN: usize = 12;
const KEY_ENV: &str = "FATHOM_CREDENTIAL_KEY";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CredentialRow { pub id: String, pub name: String, pub kind: String, pub created_at: String, pub updated_at: String }

fn ensure_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS credentials (id TEXT PRIMARY KEY,name TEXT NOT NULL UNIQUE,kind TEXT NOT NULL,ciphertext BLOB NOT NULL,created_at TEXT NOT NULL,updated_at TEXT NOT NULL); CREATE INDEX IF NOT EXISTS idx_credentials_updated_at ON credentials(updated_at);")
}
fn bounded<'a>(value: &'a str, max: usize, field: &str) -> Result<&'a str> {
    if value.trim().is_empty() { bail!("credential {field} must not be empty") }
    if value.as_bytes().len() > max { bail!("credential {field} exceeds maximum length") }
    Ok(value)
}
fn key_bytes() -> Result<[u8; 32]> {
    let raw = std::env::var(KEY_ENV).map_err(|_| anyhow!("credential encryption key is not configured"))?;
    if raw.is_empty() { bail!("credential encryption key is invalid") }
    let mut out = [0u8; 32];
    if raw.len() == 64 && raw.as_bytes().iter().all(|b| b.is_ascii_hexdigit()) {
        for (i, pair) in raw.as_bytes().chunks_exact(2).enumerate() {
            let high = (pair[0] as char).to_digit(16).unwrap() as u8;
            let low = (pair[1] as char).to_digit(16).unwrap() as u8;
            out[i] = (high << 4) | low;
        }
        return Ok(out);
    }
    for decoded in [base64::engine::general_purpose::STANDARD.decode(raw.as_bytes()), base64::engine::general_purpose::STANDARD_NO_PAD.decode(raw.as_bytes()), base64::engine::general_purpose::URL_SAFE.decode(raw.as_bytes()), base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(raw.as_bytes())] {
        if let Ok(decoded) = decoded { if decoded.len() == 32 { out.copy_from_slice(&decoded); return Ok(out); } }
    }
    bail!("credential encryption key is invalid")
}
fn key() -> Result<LessSafeKey> {
    let bytes = key_bytes()?;
    let unbound = UnboundKey::new(&aead::AES_256_GCM, &bytes).map_err(|_| anyhow!("credential encryption key is invalid"))?;
    Ok(LessSafeKey::new(unbound))
}
fn encrypt(secret: &str, key: &LessSafeKey) -> Result<Vec<u8>> {
    let mut nonce_bytes = [0u8; NONCE_LEN];
    SystemRandom::new().fill(&mut nonce_bytes).map_err(|_| anyhow!("credential encryption failed"))?;
    let mut data = secret.as_bytes().to_vec();
    key.seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce_bytes), Aad::empty(), &mut data).map_err(|_| anyhow!("credential encryption failed"))?;
    let mut result = Vec::with_capacity(NONCE_LEN + data.len()); result.extend_from_slice(&nonce_bytes); result.extend_from_slice(&data); Ok(result)
}
fn decrypt(mut data: Vec<u8>, key: &LessSafeKey) -> Result<String> {
    if data.len() < NONCE_LEN + 16 { bail!("credential ciphertext is invalid") }
    let mut nonce_bytes = [0u8; NONCE_LEN]; nonce_bytes.copy_from_slice(&data[..NONCE_LEN]);
    let plain = key.open_in_place(Nonce::assume_unique_for_key(nonce_bytes), Aad::empty(), &mut data[NONCE_LEN..]).map_err(|_| anyhow!("credential decryption failed"))?;
    String::from_utf8(plain.to_vec()).map_err(|_| anyhow!("credential plaintext is invalid"))
}
fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CredentialRow> { Ok(CredentialRow { id: row.get(0)?, name: row.get(1)?, kind: row.get(2)?, created_at: row.get(3)?, updated_at: row.get(4)? }) }

impl crate::Persistence {
    pub fn store_credential(&self, name: &str, kind: &str, secret: &str) -> Result<CredentialRow> {
        let name = bounded(name, MAX_NAME, "name")?; let kind = bounded(kind, MAX_KIND, "kind")?;
        if secret.is_empty() { bail!("credential secret must not be empty") } if secret.as_bytes().len() > MAX_SECRET { bail!("credential secret exceeds maximum length") }
        let key = key()?; let ciphertext = encrypt(secret, &key)?; let id = Uuid::now_v7().to_string(); let now = chrono::Utc::now().to_rfc3339();
        let mut conn = self.conn.lock(); ensure_schema(&conn)?; let tx = conn.transaction()?;
        tx.execute("INSERT INTO credentials (id,name,kind,ciphertext,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?5) ON CONFLICT(name) DO UPDATE SET kind=excluded.kind,ciphertext=excluded.ciphertext,updated_at=excluded.updated_at", params![id,name,kind,ciphertext,now])?;
        let result = tx.query_row("SELECT id,name,kind,created_at,updated_at FROM credentials WHERE name=?1", params![name], row)?; tx.commit()?; Ok(result)
    }
    pub fn list_credentials(&self) -> Result<Vec<CredentialRow>> {
        let conn = self.conn.lock(); ensure_schema(&conn)?;
        let mut stmt = conn.prepare("SELECT id,name,kind,created_at,updated_at FROM credentials ORDER BY updated_at DESC,id")?;
        let rows = stmt.query_map([], row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
    pub fn delete_credential(&self, id: &str) -> Result<bool> { let id = bounded(id, 128, "id")?; let conn = self.conn.lock(); ensure_schema(&conn)?; Ok(conn.execute("DELETE FROM credentials WHERE id=?1", params![id])? != 0) }
    pub fn resolve_secret(&self, id: &str) -> Result<Option<String>> { let id = bounded(id, 128, "id")?; let key = key()?; let conn = self.conn.lock(); ensure_schema(&conn)?; let ciphertext: Option<Vec<u8>> = conn.query_row("SELECT ciphertext FROM credentials WHERE id=?1", params![id], |r| r.get(0)).optional()?; ciphertext.map(|v| decrypt(v, &key)).transpose() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn missing_key_refuses_storage() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var(KEY_ENV);
        let db = crate::Persistence::in_memory().unwrap();
        assert!(db.store_credential("mail", "password", "secret-value").is_err());
        assert!(db.list_credentials().unwrap().is_empty());
    }

    #[test]
    fn rows_hide_and_encrypt_secret() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var(KEY_ENV, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=");
        let db = crate::Persistence::in_memory().unwrap();
        let first = db.store_credential("mail", "password", "secret-one").unwrap();
        let second = db.store_credential("mail", "password", "secret-two").unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(db.resolve_secret(&second.id).unwrap().as_deref(), Some("secret-two"));
        assert!(!serde_json::to_string(&second).unwrap().contains("secret-two"));
        let conn = db.conn.lock();
        let ciphertext: Vec<u8> = conn.query_row("SELECT ciphertext FROM credentials WHERE id=?1", params![second.id], |r| r.get(0)).unwrap();
        assert!(!String::from_utf8_lossy(&ciphertext).contains("secret-two"));
        std::env::remove_var(KEY_ENV);
    }
}
