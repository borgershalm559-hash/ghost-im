//! Settings: simple key-value strings.

use crate::{Database, Result};
use rusqlite::params;

pub struct SettingsRepo<'a> {
    db: &'a Database,
}

impl<'a> SettingsRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        self.db.with_tx(|tx| {
            tx.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )?;
            Ok(())
        })
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        self.db.with_conn(|c| {
            let mut stmt = c.prepare("SELECT value FROM settings WHERE key = ?1")?;
            let mut rows = stmt.query(params![key])?;
            match rows.next()? {
                Some(row) => Ok(Some(row.get::<_, String>(0)?)),
                None => Ok(None),
            }
        })
    }

    pub fn delete(&self, key: &str) -> Result<bool> {
        self.db.with_tx(|tx| {
            let n = tx.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
            Ok(n > 0)
        })
    }
}

impl Database {
    pub fn settings(&self) -> SettingsRepo<'_> {
        SettingsRepo::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master_key::derive_master_key;
    use ghost_identity::IdentityKey;

    fn fresh_db() -> Database {
        let db = Database::open_in_memory(&derive_master_key(&IdentityKey::generate())).unwrap();
        db.migrate().unwrap();
        db
    }

    #[test]
    fn set_then_get() {
        let db = fresh_db();
        db.settings().set("retention", "30d").unwrap();
        assert_eq!(db.settings().get("retention").unwrap().as_deref(), Some("30d"));
    }

    #[test]
    fn set_overwrites() {
        let db = fresh_db();
        db.settings().set("k", "v1").unwrap();
        db.settings().set("k", "v2").unwrap();
        assert_eq!(db.settings().get("k").unwrap().as_deref(), Some("v2"));
    }

    #[test]
    fn delete_removes_entry() {
        let db = fresh_db();
        db.settings().set("k", "v").unwrap();
        assert!(db.settings().delete("k").unwrap());
        assert!(db.settings().get("k").unwrap().is_none());
    }

    #[test]
    fn get_missing_returns_none() {
        let db = fresh_db();
        assert!(db.settings().get("nope").unwrap().is_none());
    }
}
