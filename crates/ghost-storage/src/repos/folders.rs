//! Folders repository: user-defined groupings of contacts.

use crate::{Database, Result, StorageError};
use ghost_core::GhostId;
use rusqlite::params;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folder {
    pub id: i64,
    pub name: String,
    pub icon: Option<String>,
    pub sort_order: i64,
    pub created_at: i64,
}

pub struct FoldersRepo<'a> {
    db: &'a Database,
}

impl<'a> FoldersRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Create a folder. Returns the new id. Errors on duplicate name (UNIQUE constraint).
    pub fn create(&self, name: &str, icon: Option<&str>, sort_order: i64, now: i64) -> Result<i64> {
        self.db.with_tx(|tx| {
            tx.execute(
                "INSERT INTO folders (name, icon, sort_order, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![name, icon, sort_order, now],
            )?;
            Ok(tx.last_insert_rowid())
        })
    }

    /// Rename a folder. Errors NotFound if id absent.
    pub fn rename(&self, id: i64, new_name: &str) -> Result<()> {
        self.db.with_tx(|tx| {
            let n = tx.execute(
                "UPDATE folders SET name = ?2 WHERE id = ?1",
                params![id, new_name],
            )?;
            if n == 0 {
                return Err(StorageError::NotFound(format!("folder {id}")));
            }
            Ok(())
        })
    }

    /// Delete a folder. CASCADE removes contact_folders entries automatically.
    pub fn delete(&self, id: i64) -> Result<bool> {
        self.db.with_tx(|tx| {
            let n = tx.execute("DELETE FROM folders WHERE id = ?1", params![id])?;
            Ok(n > 0)
        })
    }

    pub fn list(&self) -> Result<Vec<Folder>> {
        self.db.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT id, name, icon, sort_order, created_at
                   FROM folders ORDER BY sort_order ASC, id ASC",
            )?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(Folder {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        icon: row.get(2)?,
                        sort_order: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    pub fn add_contact(&self, folder_id: i64, contact_id: &GhostId) -> Result<()> {
        self.db.with_tx(|tx| {
            tx.execute(
                "INSERT OR IGNORE INTO contact_folders (folder_id, contact_id) VALUES (?1, ?2)",
                params![folder_id, contact_id.as_bytes()],
            )?;
            Ok(())
        })
    }

    pub fn remove_contact(&self, folder_id: i64, contact_id: &GhostId) -> Result<()> {
        self.db.with_tx(|tx| {
            tx.execute(
                "DELETE FROM contact_folders WHERE folder_id = ?1 AND contact_id = ?2",
                params![folder_id, contact_id.as_bytes()],
            )?;
            Ok(())
        })
    }

    /// Return GhostIds of all contacts in a folder.
    pub fn contacts_in_folder(&self, folder_id: i64) -> Result<Vec<GhostId>> {
        self.db.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT contact_id FROM contact_folders WHERE folder_id = ?1",
            )?;
            let rows = stmt
                .query_map(params![folder_id], |row| row.get::<_, Vec<u8>>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let mut out = Vec::with_capacity(rows.len());
            for bytes in rows {
                if bytes.len() != 32 {
                    return Err(StorageError::InvalidBlob {
                        table: "contact_folders",
                        column: "contact_id",
                        detail: format!("expected 32 bytes, got {}", bytes.len()),
                    });
                }
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                out.push(GhostId::from_bytes(arr));
            }
            Ok(out)
        })
    }
}

impl Database {
    pub fn folders(&self) -> FoldersRepo<'_> {
        FoldersRepo::new(self)
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

    fn fake_contact(db: &Database, seed: u8) -> GhostId {
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
                last_read_at: 0,
                pinned: false,
                muted: false,
                retention_seconds: None,
            })
            .unwrap();
        id
    }

    #[test]
    fn create_and_list_folders() {
        let db = fresh_db();
        let id1 = db.folders().create("Personal", None, 0, 100).unwrap();
        let id2 = db.folders().create("Work", Some("users"), 1, 200).unwrap();
        let list = db.folders().list().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, id1);
        assert_eq!(list[0].name, "Personal");
        assert_eq!(list[1].name, "Work");
        assert_eq!(list[1].icon.as_deref(), Some("users"));
    }

    #[test]
    fn rename_folder() {
        let db = fresh_db();
        let id = db.folders().create("Old", None, 0, 0).unwrap();
        db.folders().rename(id, "New").unwrap();
        let list = db.folders().list().unwrap();
        assert_eq!(list[0].name, "New");
    }

    #[test]
    fn rename_missing_returns_not_found() {
        let db = fresh_db();
        let err = db.folders().rename(999, "x").unwrap_err();
        assert!(matches!(err, StorageError::NotFound(_)));
    }

    #[test]
    fn delete_folder_cascades_contact_folders() {
        let db = fresh_db();
        let id = db.folders().create("Tmp", None, 0, 0).unwrap();
        let contact = fake_contact(&db, 1);
        db.folders().add_contact(id, &contact).unwrap();
        assert_eq!(db.folders().contacts_in_folder(id).unwrap().len(), 1);
        assert!(db.folders().delete(id).unwrap());
        // Folder deleted; join rows also gone.
        let folders = db.folders().list().unwrap();
        assert!(folders.is_empty());
    }

    #[test]
    fn add_remove_contact_idempotent() {
        let db = fresh_db();
        let id = db.folders().create("F", None, 0, 0).unwrap();
        let c = fake_contact(&db, 2);
        db.folders().add_contact(id, &c).unwrap();
        db.folders().add_contact(id, &c).unwrap(); // duplicate ignored
        assert_eq!(db.folders().contacts_in_folder(id).unwrap().len(), 1);
        db.folders().remove_contact(id, &c).unwrap();
        assert_eq!(db.folders().contacts_in_folder(id).unwrap().len(), 0);
    }

    #[test]
    fn duplicate_name_errors() {
        let db = fresh_db();
        db.folders().create("Dup", None, 0, 0).unwrap();
        let err = db.folders().create("Dup", None, 0, 0).unwrap_err();
        assert!(matches!(err, StorageError::Sqlite(_)));
    }
}
