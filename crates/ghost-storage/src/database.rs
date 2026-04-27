//! Database wrapper around rusqlite::Connection with SQLCipher PRAGMA key.

use crate::master_key::{master_key_pragma, MASTER_KEY_LEN};
use crate::{Result, StorageError};
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database").finish_non_exhaustive()
    }
}

impl Database {
    /// Open or create an encrypted SQLite database at `path` using the supplied
    /// 32-byte master key.
    pub fn open_encrypted(path: &Path, master_key: &[u8; MASTER_KEY_LEN]) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;

        // Apply key BEFORE any other statement (SQLCipher requirement).
        // SQLCipher raw-key syntax: PRAGMA key = "x'<hex>'";
        // The outer double-quotes make it a string literal; SQLCipher then
        // interprets the x'...' prefix as a pre-derived raw key, bypassing PBKDF2.
        let pragma = master_key_pragma(master_key);
        conn.execute_batch(&format!("PRAGMA key = \"{pragma}\";"))?;

        // Pin SQLCipher format options for forward stability.
        conn.execute_batch(
            "PRAGMA cipher_page_size = 4096;
             PRAGMA cipher_kdf_iter = 256000;
             PRAGMA cipher_default_kdf_iter = 256000;
             PRAGMA foreign_keys = ON;",
        )?;

        // Verify the key is correct by trying a no-op read.
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
            row.get::<_, i64>(0)
        })?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Open an in-memory encrypted database — used by tests.
    pub fn open_in_memory(master_key: &[u8; MASTER_KEY_LEN]) -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let pragma = master_key_pragma(master_key);
        conn.execute_batch(&format!("PRAGMA key = \"{pragma}\";"))?;
        conn.execute_batch(
            "PRAGMA cipher_page_size = 4096;
             PRAGMA cipher_kdf_iter = 256000;
             PRAGMA foreign_keys = ON;",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Run `f` with the held connection lock. Internal helper for repos.
    pub(crate) fn with_conn<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&Connection) -> Result<R>,
    {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Invalid(format!("connection lock poisoned: {e}")))?;
        f(&conn)
    }

    /// Run `f` with a mutable transaction. Internal helper for repos.
    pub(crate) fn with_tx<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&rusqlite::Transaction<'_>) -> Result<R>,
    {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Invalid(format!("connection lock poisoned: {e}")))?;
        let tx = conn.transaction()?;
        let result = f(&tx)?;
        tx.commit()?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master_key::{derive_master_key, MASTER_KEY_LEN};
    use ghost_identity::IdentityKey;
    use tempfile::tempdir;

    fn fresh_master_key() -> [u8; MASTER_KEY_LEN] {
        derive_master_key(&IdentityKey::generate())
    }

    #[test]
    fn open_in_memory_succeeds() {
        let key = fresh_master_key();
        let db = Database::open_in_memory(&key).unwrap();
        let count: i64 = db
            .with_conn(|c| {
                c.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get(0))
                    .map_err(Into::into)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn open_encrypted_persists_across_handles_with_same_key() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("ghost.db");
        let key = fresh_master_key();

        {
            let db = Database::open_encrypted(&path, &key).unwrap();
            db.with_conn(|c| {
                c.execute("CREATE TABLE t (v INTEGER)", [])
                    .map_err(Into::into)
            })
            .unwrap();
            db.with_tx(|tx| {
                tx.execute("INSERT INTO t (v) VALUES (?1)", [42_i64])
                    .map_err(Into::into)
            })
            .unwrap();
        }

        {
            let db = Database::open_encrypted(&path, &key).unwrap();
            let v: i64 = db
                .with_conn(|c| {
                    c.query_row("SELECT v FROM t", [], |r| r.get(0))
                        .map_err(Into::into)
                })
                .unwrap();
            assert_eq!(v, 42);
        }
    }

    #[test]
    fn open_encrypted_fails_with_wrong_key() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("ghost.db");
        let right = fresh_master_key();
        let wrong = fresh_master_key();
        assert_ne!(right, wrong);

        {
            let db = Database::open_encrypted(&path, &right).unwrap();
            db.with_conn(|c| {
                c.execute("CREATE TABLE t (v INTEGER)", [])
                    .map(|_| ())
                    .map_err(Into::into)
            })
            .unwrap();
        }

        let err = Database::open_encrypted(&path, &wrong).unwrap_err();
        assert!(matches!(err, StorageError::Sqlite(_)));
    }

    #[test]
    fn foreign_keys_pragma_is_on() {
        let key = fresh_master_key();
        let db = Database::open_in_memory(&key).unwrap();
        let on: i64 = db
            .with_conn(|c| {
                c.query_row("PRAGMA foreign_keys", [], |r| r.get(0))
                    .map_err(Into::into)
            })
            .unwrap();
        assert_eq!(on, 1);
    }
}
