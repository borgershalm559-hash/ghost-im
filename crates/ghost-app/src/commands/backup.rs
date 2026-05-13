//! Backup / Restore commands.
//!
//! Exports a single passphrase-encrypted file containing the user's
//! `identity.encrypted` + `ghost.db`. Restore decrypts and writes both back
//! to the standard data dir. The app must be restarted after restore for the
//! Client to pick up the new files.
//!
//! Format (`*.ghost-backup`):
//!   magic         8 bytes  = b"GHBKP_v1"
//!   salt         16 bytes  (Argon2 salt)
//!   nonce        24 bytes  (XChaCha20-Poly1305 nonce)
//!   ciphertext   N bytes   (encrypts: u32 LE identity_len || identity_bytes || u32 LE db_len || db_bytes)

use crate::error::{CommandError, CommandResult};
use ghost_identity::crypto::{aead_decrypt, aead_encrypt, derive_key, NONCE_LEN, SALT_LEN};
use ghost_identity::{database_file, identity_file};
use rand::RngCore;
use std::fs;
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 8] = b"GHBKP_v1";
const AAD: &[u8] = b"ghost.backup.v1.aad";

#[tauri::command]
pub async fn export_backup(path: String, passphrase: String) -> CommandResult<u64> {
    if passphrase.len() < 6 {
        return Err(CommandError(
            "passphrase must be at least 6 characters".into(),
        ));
    }
    let identity_path = identity_file()
        .map_err(|e| CommandError(format!("resolve identity path: {e}")))?;
    let db_path =
        database_file().map_err(|e| CommandError(format!("resolve db path: {e}")))?;
    if !identity_path.exists() {
        return Err(CommandError("no identity to back up".into()));
    }

    let identity_bytes = fs::read(&identity_path)
        .map_err(|e| CommandError(format!("read identity: {e}")))?;
    let db_bytes = fs::read(&db_path).unwrap_or_default();

    // plaintext = LE-u32(identity_len) || identity || LE-u32(db_len) || db
    let mut plaintext = Vec::with_capacity(8 + identity_bytes.len() + db_bytes.len());
    plaintext.extend_from_slice(&(identity_bytes.len() as u32).to_le_bytes());
    plaintext.extend_from_slice(&identity_bytes);
    plaintext.extend_from_slice(&(db_bytes.len() as u32).to_le_bytes());
    plaintext.extend_from_slice(&db_bytes);

    let mut salt = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    let key = derive_key(passphrase.as_bytes(), &salt)
        .map_err(|e| CommandError(format!("KDF: {e}")))?;
    let (nonce, ciphertext) = aead_encrypt(&key, &plaintext, AAD)
        .map_err(|e| CommandError(format!("encrypt: {e}")))?;

    let mut file_bytes = Vec::with_capacity(8 + SALT_LEN + NONCE_LEN + ciphertext.len());
    file_bytes.extend_from_slice(MAGIC);
    file_bytes.extend_from_slice(&salt);
    file_bytes.extend_from_slice(&nonce);
    file_bytes.extend_from_slice(&ciphertext);

    let out = PathBuf::from(&path);
    if let Some(parent) = out.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(&out, &file_bytes).map_err(|e| CommandError(format!("write backup: {e}")))?;
    Ok(file_bytes.len() as u64)
}

#[tauri::command]
pub async fn import_backup(path: String, passphrase: String) -> CommandResult<()> {
    let file_bytes =
        fs::read(Path::new(&path)).map_err(|e| CommandError(format!("read backup: {e}")))?;
    if file_bytes.len() < 8 + SALT_LEN + NONCE_LEN + 16 {
        return Err(CommandError("backup file too small".into()));
    }
    if &file_bytes[..8] != MAGIC {
        return Err(CommandError("bad magic — not a Ghost backup".into()));
    }
    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(&file_bytes[8..8 + SALT_LEN]);
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&file_bytes[8 + SALT_LEN..8 + SALT_LEN + NONCE_LEN]);
    let ciphertext = &file_bytes[8 + SALT_LEN + NONCE_LEN..];

    let key = derive_key(passphrase.as_bytes(), &salt)
        .map_err(|e| CommandError(format!("KDF: {e}")))?;
    let plaintext = aead_decrypt(&key, &nonce, ciphertext, AAD)
        .map_err(|e| CommandError(format!("decrypt (wrong passphrase?): {e}")))?;

    let (identity_bytes, db_bytes) = parse_payload(&plaintext)
        .ok_or_else(|| CommandError("malformed payload".into()))?;

    let identity_path = identity_file()
        .map_err(|e| CommandError(format!("resolve identity path: {e}")))?;
    let db_path =
        database_file().map_err(|e| CommandError(format!("resolve db path: {e}")))?;
    if let Some(parent) = identity_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(&identity_path, identity_bytes)
        .map_err(|e| CommandError(format!("write identity: {e}")))?;
    if !db_bytes.is_empty() {
        fs::write(&db_path, db_bytes).map_err(|e| CommandError(format!("write db: {e}")))?;
    }
    Ok(())
}

fn parse_payload(pt: &[u8]) -> Option<(&[u8], &[u8])> {
    if pt.len() < 8 {
        return None;
    }
    let id_len = u32::from_le_bytes([pt[0], pt[1], pt[2], pt[3]]) as usize;
    let after_id = 4 + id_len;
    if pt.len() < after_id + 4 {
        return None;
    }
    let identity = &pt[4..after_id];
    let db_len = u32::from_le_bytes([
        pt[after_id],
        pt[after_id + 1],
        pt[after_id + 2],
        pt[after_id + 3],
    ]) as usize;
    let db_start = after_id + 4;
    if pt.len() < db_start + db_len {
        return None;
    }
    Some((identity, &pt[db_start..db_start + db_len]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_roundtrip() {
        let identity = b"identity bytes";
        let db = b"some database bytes here";
        let mut pt = Vec::new();
        pt.extend_from_slice(&(identity.len() as u32).to_le_bytes());
        pt.extend_from_slice(identity);
        pt.extend_from_slice(&(db.len() as u32).to_le_bytes());
        pt.extend_from_slice(db);
        let (i, d) = parse_payload(&pt).unwrap();
        assert_eq!(i, identity);
        assert_eq!(d, db);
    }

    #[test]
    fn payload_with_empty_db() {
        let identity = b"only identity";
        let mut pt = Vec::new();
        pt.extend_from_slice(&(identity.len() as u32).to_le_bytes());
        pt.extend_from_slice(identity);
        pt.extend_from_slice(&0u32.to_le_bytes());
        let (i, d) = parse_payload(&pt).unwrap();
        assert_eq!(i, identity);
        assert!(d.is_empty());
    }

    #[test]
    fn parse_rejects_truncated() {
        assert!(parse_payload(b"").is_none());
        assert!(parse_payload(b"\x05\x00\x00\x00aa").is_none()); // claims 5 bytes but only 2
    }
}
