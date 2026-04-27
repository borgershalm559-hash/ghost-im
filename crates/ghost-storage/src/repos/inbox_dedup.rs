//! Inbox dedup: short-lived "we already saw this msg_uuid" set.

use crate::{Database, Result};
use rusqlite::params;

pub struct InboxDedupRepo<'a> {
    db: &'a Database,
}

impl<'a> InboxDedupRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Returns `true` if the msg_uuid was new, `false` if it was already present.
    pub fn try_insert(&self, msg_uuid: &[u8; 16], received_at: i64) -> Result<bool> {
        self.db.with_tx(|tx| {
            let n = tx.execute(
                "INSERT OR IGNORE INTO inbox_dedup (msg_uuid, received_at) VALUES (?1, ?2)",
                params![&msg_uuid[..], received_at],
            )?;
            Ok(n == 1)
        })
    }

    pub fn purge_older_than(&self, before: i64) -> Result<usize> {
        self.db.with_tx(|tx| {
            let n = tx.execute(
                "DELETE FROM inbox_dedup WHERE received_at < ?1",
                params![before],
            )?;
            Ok(n)
        })
    }
}

impl Database {
    pub fn inbox_dedup(&self) -> InboxDedupRepo<'_> {
        InboxDedupRepo::new(self)
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
    fn first_insert_returns_true_duplicate_returns_false() {
        let db = fresh_db();
        assert!(db.inbox_dedup().try_insert(&[1u8; 16], 100).unwrap());
        assert!(!db.inbox_dedup().try_insert(&[1u8; 16], 200).unwrap());
    }

    #[test]
    fn purge_removes_old_entries() {
        let db = fresh_db();
        db.inbox_dedup().try_insert(&[1u8; 16], 100).unwrap();
        db.inbox_dedup().try_insert(&[2u8; 16], 200).unwrap();
        let removed = db.inbox_dedup().purge_older_than(150).unwrap();
        assert_eq!(removed, 1);
        assert!(db.inbox_dedup().try_insert(&[1u8; 16], 999).unwrap());
        assert!(!db.inbox_dedup().try_insert(&[2u8; 16], 999).unwrap());
    }
}
