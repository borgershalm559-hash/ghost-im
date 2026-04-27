//! MLS group state repository. Stores TLS-serialized MlsGroup blobs keyed by group_id.

use crate::{Database, Result, StorageError};
use ghost_core::GhostId;
use rusqlite::params;

#[derive(Clone, Debug)]
pub struct MlsGroupRow {
    pub group_id: [u8; 32],
    pub contact_id: GhostId,
    pub state_blob: Vec<u8>,
    pub current_epoch: u64,
    pub created_at: i64,
    pub last_updated: i64,
}

pub struct MlsGroupsRepo<'a> {
    db: &'a Database,
}

impl<'a> MlsGroupsRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn upsert(&self, row: &MlsGroupRow) -> Result<()> {
        self.db.with_tx(|tx| {
            tx.execute(
                "INSERT OR REPLACE INTO mls_groups (
                    group_id, contact_id, state_blob, current_epoch, created_at, last_updated
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    &row.group_id[..],
                    row.contact_id.as_bytes(),
                    row.state_blob,
                    row.current_epoch as i64,
                    row.created_at,
                    row.last_updated,
                ],
            )?;
            Ok(())
        })
    }

    pub fn load_for_contact(&self, contact: &GhostId) -> Result<Option<MlsGroupRow>> {
        self.db.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT group_id, contact_id, state_blob, current_epoch, created_at, last_updated
                   FROM mls_groups WHERE contact_id = ?1",
            )?;
            let mut rows = stmt.query(params![contact.as_bytes()])?;
            match rows.next()? {
                Some(row) => Ok(Some(Self::row_to_struct(row)?)),
                None => Ok(None),
            }
        })
    }

    pub fn delete_for_contact(&self, contact: &GhostId) -> Result<bool> {
        self.db.with_tx(|tx| {
            let n = tx.execute(
                "DELETE FROM mls_groups WHERE contact_id = ?1",
                params![contact.as_bytes()],
            )?;
            Ok(n > 0)
        })
    }

    fn row_to_struct(row: &rusqlite::Row<'_>) -> Result<MlsGroupRow> {
        let group_id_bytes: Vec<u8> = row.get(0)?;
        if group_id_bytes.len() != 32 {
            return Err(StorageError::InvalidBlob {
                table: "mls_groups",
                column: "group_id",
                detail: format!("expected 32 bytes, got {}", group_id_bytes.len()),
            });
        }
        let mut group_id = [0u8; 32];
        group_id.copy_from_slice(&group_id_bytes);

        let contact_bytes: Vec<u8> = row.get(1)?;
        if contact_bytes.len() != 32 {
            return Err(StorageError::InvalidBlob {
                table: "mls_groups",
                column: "contact_id",
                detail: format!("expected 32 bytes, got {}", contact_bytes.len()),
            });
        }
        let mut contact_arr = [0u8; 32];
        contact_arr.copy_from_slice(&contact_bytes);

        let epoch_i64: i64 = row.get(3)?;
        Ok(MlsGroupRow {
            group_id,
            contact_id: GhostId::from_bytes(contact_arr),
            state_blob: row.get(2)?,
            current_epoch: epoch_i64 as u64,
            created_at: row.get(4)?,
            last_updated: row.get(5)?,
        })
    }
}

impl Database {
    pub fn mls_groups(&self) -> MlsGroupsRepo<'_> {
        MlsGroupsRepo::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master_key::derive_master_key;
    use crate::repos::contacts::{Contact, Verification};
    use ghost_core::Fingerprint;
    use ghost_identity::IdentityKey;

    fn fresh_db() -> Database {
        let db = Database::open_in_memory(&derive_master_key(&IdentityKey::generate())).unwrap();
        db.migrate().unwrap();
        db
    }

    fn insert_contact(db: &Database, seed: u8) -> GhostId {
        let id = GhostId::from_bytes([seed; 32]);
        let fp = Fingerprint::of(&id).to_string();
        db.contacts()
            .insert(&Contact {
                ghost_id: id,
                display_name: None,
                local_alias: None,
                fingerprint: fp,
                added_at: 0,
                last_endpoint: None,
                verification: Verification::Unverified,
                notes: None,
                blocked: false,
                dk_pub: None,
            })
            .unwrap();
        id
    }

    #[test]
    fn upsert_then_load_roundtrips() {
        let db = fresh_db();
        let contact = insert_contact(&db, 1);
        let row = MlsGroupRow {
            group_id: [42u8; 32],
            contact_id: contact,
            state_blob: vec![1, 2, 3, 4, 5],
            current_epoch: 7,
            created_at: 1700000000,
            last_updated: 1700000060,
        };
        db.mls_groups().upsert(&row).unwrap();
        let loaded = db.mls_groups().load_for_contact(&contact).unwrap().unwrap();
        assert_eq!(loaded.group_id, row.group_id);
        assert_eq!(loaded.state_blob, vec![1, 2, 3, 4, 5]);
        assert_eq!(loaded.current_epoch, 7);
    }

    #[test]
    fn upsert_replaces_existing_state() {
        let db = fresh_db();
        let contact = insert_contact(&db, 2);
        let v1 = MlsGroupRow {
            group_id: [10u8; 32],
            contact_id: contact,
            state_blob: vec![0x11],
            current_epoch: 1,
            created_at: 0,
            last_updated: 0,
        };
        db.mls_groups().upsert(&v1).unwrap();
        let v2 = MlsGroupRow {
            group_id: [10u8; 32],
            contact_id: contact,
            state_blob: vec![0x22, 0x33],
            current_epoch: 2,
            created_at: 0,
            last_updated: 1,
        };
        db.mls_groups().upsert(&v2).unwrap();
        let loaded = db.mls_groups().load_for_contact(&contact).unwrap().unwrap();
        assert_eq!(loaded.state_blob, vec![0x22, 0x33]);
        assert_eq!(loaded.current_epoch, 2);
        assert_eq!(loaded.last_updated, 1);
    }

    #[test]
    fn load_for_unknown_contact_returns_none() {
        let db = fresh_db();
        let contact = GhostId::from_bytes([0xff; 32]);
        assert!(db
            .mls_groups()
            .load_for_contact(&contact)
            .unwrap()
            .is_none());
    }

    #[test]
    fn upsert_with_unknown_contact_violates_fk() {
        let db = fresh_db();
        let row = MlsGroupRow {
            group_id: [0u8; 32],
            contact_id: GhostId::from_bytes([0xfe; 32]),
            state_blob: vec![],
            current_epoch: 0,
            created_at: 0,
            last_updated: 0,
        };
        let err = db.mls_groups().upsert(&row).unwrap_err();
        assert!(matches!(err, StorageError::Sqlite(_)));
    }

    #[test]
    fn delete_for_contact_removes_state() {
        let db = fresh_db();
        let contact = insert_contact(&db, 3);
        db.mls_groups()
            .upsert(&MlsGroupRow {
                group_id: [0u8; 32],
                contact_id: contact,
                state_blob: vec![],
                current_epoch: 0,
                created_at: 0,
                last_updated: 0,
            })
            .unwrap();
        assert!(db.mls_groups().delete_for_contact(&contact).unwrap());
        assert!(db
            .mls_groups()
            .load_for_contact(&contact)
            .unwrap()
            .is_none());
    }
}
