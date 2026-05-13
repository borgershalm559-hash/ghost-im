//! Contacts repository.

use crate::{Database, Result, StorageError};
use ghost_core::GhostId;
use rusqlite::params;

/// Verification status of a contact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verification {
    Unverified = 0,
    Verified = 1,
}

impl Verification {
    pub fn from_i64(v: i64) -> Result<Self> {
        match v {
            0 => Ok(Self::Unverified),
            1 => Ok(Self::Verified),
            other => Err(StorageError::Invalid(format!(
                "unknown verification value {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Contact {
    pub ghost_id: GhostId,
    pub display_name: Option<String>,
    pub local_alias: Option<String>,
    pub fingerprint: String,
    pub added_at: i64,
    pub last_endpoint: Option<String>,
    pub verification: Verification,
    pub notes: Option<String>,
    pub blocked: bool,
    pub dk_pub: Option<[u8; 32]>,
    /// UNIX seconds — messages with received_at > this are unread (default 0).
    pub last_read_at: i64,
    /// Pinned chats sort first in the UI list.
    pub pinned: bool,
    /// Mutes notifications (UI-only until OS notifications land).
    pub muted: bool,
    /// `None` = forever. Otherwise: new messages get `expires_at = now + seconds`.
    pub retention_seconds: Option<i64>,
}

pub struct ContactsRepo<'a> {
    db: &'a Database,
}

impl<'a> ContactsRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Insert a new contact. Errors if the GhostId already exists.
    pub fn insert(&self, contact: &Contact) -> Result<()> {
        self.db.with_tx(|tx| {
            tx.execute(
                "INSERT INTO contacts (
                    ghost_id, display_name, local_alias, fingerprint, added_at,
                    last_endpoint, verification, notes, blocked, dk_pub,
                    last_read_at, pinned, muted, retention_seconds
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    contact.ghost_id.as_bytes(),
                    contact.display_name,
                    contact.local_alias,
                    contact.fingerprint,
                    contact.added_at,
                    contact.last_endpoint,
                    contact.verification as i64,
                    contact.notes,
                    contact.blocked as i64,
                    contact.dk_pub.as_ref().map(|b| &b[..]),
                    contact.last_read_at,
                    contact.pinned as i64,
                    contact.muted as i64,
                    contact.retention_seconds,
                ],
            )?;
            Ok(())
        })
    }

    /// Fetch a contact by GhostId. Returns `Ok(None)` if absent.
    pub fn get(&self, id: &GhostId) -> Result<Option<Contact>> {
        self.db.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT ghost_id, display_name, local_alias, fingerprint, added_at,
                        last_endpoint, verification, notes, blocked, dk_pub,
                        last_read_at, pinned, muted, retention_seconds
                   FROM contacts WHERE ghost_id = ?1",
            )?;
            let mut rows = stmt.query(params![id.as_bytes()])?;
            match rows.next()? {
                Some(row) => Ok(Some(Self::row_to_contact(row)?)),
                None => Ok(None),
            }
        })
    }

    /// List all contacts, ordered by `added_at` ascending.
    pub fn list(&self) -> Result<Vec<Contact>> {
        self.db.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT ghost_id, display_name, local_alias, fingerprint, added_at,
                        last_endpoint, verification, notes, blocked, dk_pub,
                        last_read_at, pinned, muted, retention_seconds
                   FROM contacts ORDER BY added_at ASC",
            )?;
            let rows = stmt
                .query_map([], |row| Ok(Self::row_to_contact(row)))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows.into_iter().collect()
        })
    }

    /// Update mutable fields. The GhostId and fingerprint are immutable.
    pub fn update(&self, contact: &Contact) -> Result<()> {
        self.db.with_tx(|tx| {
            let n = tx.execute(
                "UPDATE contacts SET
                    display_name = ?2,
                    local_alias = ?3,
                    last_endpoint = ?4,
                    verification = ?5,
                    notes = ?6,
                    blocked = ?7,
                    dk_pub = ?8
                 WHERE ghost_id = ?1",
                params![
                    contact.ghost_id.as_bytes(),
                    contact.display_name,
                    contact.local_alias,
                    contact.last_endpoint,
                    contact.verification as i64,
                    contact.notes,
                    contact.blocked as i64,
                    contact.dk_pub.as_ref().map(|b| &b[..]),
                ],
            )?;
            if n == 0 {
                return Err(StorageError::NotFound(format!(
                    "contact {}",
                    contact.ghost_id
                )));
            }
            Ok(())
        })
    }

    /// Delete a contact by GhostId. Returns true iff a row was deleted.
    pub fn delete(&self, id: &GhostId) -> Result<bool> {
        self.db.with_tx(|tx| {
            let n = tx.execute(
                "DELETE FROM contacts WHERE ghost_id = ?1",
                params![id.as_bytes()],
            )?;
            Ok(n > 0)
        })
    }

    pub fn set_pinned(&self, id: &GhostId, pinned: bool) -> Result<()> {
        self.exec_setter(
            "UPDATE contacts SET pinned = ?2 WHERE ghost_id = ?1",
            params![id.as_bytes(), pinned as i64],
            id,
        )
    }

    pub fn set_muted(&self, id: &GhostId, muted: bool) -> Result<()> {
        self.exec_setter(
            "UPDATE contacts SET muted = ?2 WHERE ghost_id = ?1",
            params![id.as_bytes(), muted as i64],
            id,
        )
    }

    pub fn set_verified(&self, id: &GhostId, verified: bool) -> Result<()> {
        let v = if verified {
            Verification::Verified
        } else {
            Verification::Unverified
        };
        self.exec_setter(
            "UPDATE contacts SET verification = ?2 WHERE ghost_id = ?1",
            params![id.as_bytes(), v as i64],
            id,
        )
    }

    pub fn set_retention(&self, id: &GhostId, seconds: Option<i64>) -> Result<()> {
        self.exec_setter(
            "UPDATE contacts SET retention_seconds = ?2 WHERE ghost_id = ?1",
            params![id.as_bytes(), seconds],
            id,
        )
    }

    pub fn set_last_read_at(&self, id: &GhostId, at: i64) -> Result<()> {
        self.exec_setter(
            "UPDATE contacts SET last_read_at = ?2 WHERE ghost_id = ?1",
            params![id.as_bytes(), at],
            id,
        )
    }

    fn exec_setter(&self, sql: &str, p: impl rusqlite::Params, id: &GhostId) -> Result<()> {
        self.db.with_tx(|tx| {
            let n = tx.execute(sql, p)?;
            if n == 0 {
                return Err(StorageError::NotFound(format!("contact {id}")));
            }
            Ok(())
        })
    }

    fn row_to_contact(row: &rusqlite::Row<'_>) -> Result<Contact> {
        let ghost_id_bytes: Vec<u8> = row.get(0)?;
        if ghost_id_bytes.len() != 32 {
            return Err(StorageError::InvalidBlob {
                table: "contacts",
                column: "ghost_id",
                detail: format!("expected 32 bytes, got {}", ghost_id_bytes.len()),
            });
        }
        let mut id_arr = [0u8; 32];
        id_arr.copy_from_slice(&ghost_id_bytes);
        let verification: i64 = row.get(6)?;
        let blocked: i64 = row.get(8)?;
        let dk_pub: Option<[u8; 32]> = match row.get::<_, Option<Vec<u8>>>(9)? {
            None => None,
            Some(bytes) => {
                if bytes.len() != 32 {
                    return Err(StorageError::InvalidBlob {
                        table: "contacts",
                        column: "dk_pub",
                        detail: format!("expected 32 bytes, got {}", bytes.len()),
                    });
                }
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                Some(arr)
            }
        };
        let pinned: i64 = row.get(11)?;
        let muted: i64 = row.get(12)?;
        Ok(Contact {
            ghost_id: GhostId::from_bytes(id_arr),
            display_name: row.get(1)?,
            local_alias: row.get(2)?,
            fingerprint: row.get(3)?,
            added_at: row.get(4)?,
            last_endpoint: row.get(5)?,
            verification: Verification::from_i64(verification)?,
            notes: row.get(7)?,
            blocked: blocked != 0,
            dk_pub,
            last_read_at: row.get(10)?,
            pinned: pinned != 0,
            muted: muted != 0,
            retention_seconds: row.get(13)?,
        })
    }
}

impl Database {
    pub fn contacts(&self) -> ContactsRepo<'_> {
        ContactsRepo::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master_key::derive_master_key;
    use ghost_core::Fingerprint;
    use ghost_identity::IdentityKey;

    fn fresh_db() -> Database {
        let db = Database::open_in_memory(&derive_master_key(&IdentityKey::generate())).unwrap();
        db.migrate().unwrap();
        db
    }

    fn fake_contact(seed: u8, name: &str) -> Contact {
        let id = GhostId::from_bytes([seed; 32]);
        let fp = Fingerprint::of(&id).to_string();
        Contact {
            ghost_id: id,
            display_name: Some(name.to_string()),
            local_alias: None,
            fingerprint: fp,
            added_at: 1700000000 + seed as i64,
            last_endpoint: None,
            verification: Verification::Unverified,
            notes: None,
            blocked: false,
            dk_pub: None,
            last_read_at: 0,
            pinned: false,
            muted: false,
            retention_seconds: None,
        }
    }

    #[test]
    fn insert_then_get_roundtrips() {
        let db = fresh_db();
        let c = fake_contact(1, "Alice");
        db.contacts().insert(&c).unwrap();
        let loaded = db.contacts().get(&c.ghost_id).unwrap().unwrap();
        assert_eq!(loaded.ghost_id, c.ghost_id);
        assert_eq!(loaded.display_name.as_deref(), Some("Alice"));
        assert_eq!(loaded.fingerprint, c.fingerprint);
    }

    #[test]
    fn get_returns_none_for_missing() {
        let db = fresh_db();
        let id = GhostId::from_bytes([99; 32]);
        assert!(db.contacts().get(&id).unwrap().is_none());
    }

    #[test]
    fn list_orders_by_added_at_asc() {
        let db = fresh_db();
        db.contacts().insert(&fake_contact(3, "C")).unwrap();
        db.contacts().insert(&fake_contact(1, "A")).unwrap();
        db.contacts().insert(&fake_contact(2, "B")).unwrap();
        let list = db.contacts().list().unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].display_name.as_deref(), Some("A"));
        assert_eq!(list[1].display_name.as_deref(), Some("B"));
        assert_eq!(list[2].display_name.as_deref(), Some("C"));
    }

    #[test]
    fn update_changes_mutable_fields() {
        let db = fresh_db();
        let mut c = fake_contact(5, "Old");
        db.contacts().insert(&c).unwrap();
        c.display_name = Some("New".to_string());
        c.verification = Verification::Verified;
        c.blocked = true;
        db.contacts().update(&c).unwrap();
        let loaded = db.contacts().get(&c.ghost_id).unwrap().unwrap();
        assert_eq!(loaded.display_name.as_deref(), Some("New"));
        assert_eq!(loaded.verification, Verification::Verified);
        assert!(loaded.blocked);
    }

    #[test]
    fn update_missing_returns_not_found() {
        let db = fresh_db();
        let c = fake_contact(7, "Ghost");
        let err = db.contacts().update(&c).unwrap_err();
        assert!(matches!(err, StorageError::NotFound(_)));
    }

    #[test]
    fn delete_removes_row() {
        let db = fresh_db();
        let c = fake_contact(8, "Bye");
        db.contacts().insert(&c).unwrap();
        assert!(db.contacts().delete(&c.ghost_id).unwrap());
        assert!(db.contacts().get(&c.ghost_id).unwrap().is_none());
    }

    #[test]
    fn insert_duplicate_errors() {
        let db = fresh_db();
        let c = fake_contact(9, "Once");
        db.contacts().insert(&c).unwrap();
        let err = db.contacts().insert(&c).unwrap_err();
        assert!(matches!(err, StorageError::Sqlite(_)));
    }

    #[test]
    fn dk_pub_round_trips() {
        let db = fresh_db();
        let mut c = fake_contact(10, "WithDk");
        c.dk_pub = Some([42u8; 32]);
        db.contacts().insert(&c).unwrap();
        let loaded = db.contacts().get(&c.ghost_id).unwrap().unwrap();
        assert_eq!(loaded.dk_pub, Some([42u8; 32]));
    }

    #[test]
    fn set_pinned_toggles_field() {
        let db = fresh_db();
        let c = fake_contact(20, "Pin");
        db.contacts().insert(&c).unwrap();
        db.contacts().set_pinned(&c.ghost_id, true).unwrap();
        assert!(db.contacts().get(&c.ghost_id).unwrap().unwrap().pinned);
        db.contacts().set_pinned(&c.ghost_id, false).unwrap();
        assert!(!db.contacts().get(&c.ghost_id).unwrap().unwrap().pinned);
    }

    #[test]
    fn set_muted_toggles_field() {
        let db = fresh_db();
        let c = fake_contact(21, "Mute");
        db.contacts().insert(&c).unwrap();
        db.contacts().set_muted(&c.ghost_id, true).unwrap();
        assert!(db.contacts().get(&c.ghost_id).unwrap().unwrap().muted);
    }

    #[test]
    fn set_verified_writes_correct_enum() {
        let db = fresh_db();
        let c = fake_contact(22, "Verify");
        db.contacts().insert(&c).unwrap();
        db.contacts().set_verified(&c.ghost_id, true).unwrap();
        assert_eq!(
            db.contacts().get(&c.ghost_id).unwrap().unwrap().verification,
            Verification::Verified
        );
        db.contacts().set_verified(&c.ghost_id, false).unwrap();
        assert_eq!(
            db.contacts().get(&c.ghost_id).unwrap().unwrap().verification,
            Verification::Unverified
        );
    }

    #[test]
    fn set_retention_writes_seconds_or_null() {
        let db = fresh_db();
        let c = fake_contact(23, "Retain");
        db.contacts().insert(&c).unwrap();
        db.contacts().set_retention(&c.ghost_id, Some(86400)).unwrap();
        assert_eq!(
            db.contacts().get(&c.ghost_id).unwrap().unwrap().retention_seconds,
            Some(86400)
        );
        db.contacts().set_retention(&c.ghost_id, None).unwrap();
        assert_eq!(
            db.contacts().get(&c.ghost_id).unwrap().unwrap().retention_seconds,
            None
        );
    }

    #[test]
    fn set_last_read_at_writes_value() {
        let db = fresh_db();
        let c = fake_contact(24, "Read");
        db.contacts().insert(&c).unwrap();
        db.contacts().set_last_read_at(&c.ghost_id, 1_700_000_000).unwrap();
        assert_eq!(
            db.contacts().get(&c.ghost_id).unwrap().unwrap().last_read_at,
            1_700_000_000
        );
    }

    #[test]
    fn setter_on_missing_returns_not_found() {
        let db = fresh_db();
        let id = GhostId::from_bytes([99; 32]);
        let err = db.contacts().set_pinned(&id, true).unwrap_err();
        assert!(matches!(err, StorageError::NotFound(_)));
    }
}
