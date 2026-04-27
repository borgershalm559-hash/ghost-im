//! Our own published KeyPackages.

use crate::{Database, Result, StorageError};
use rusqlite::params;

#[derive(Clone, Debug)]
pub struct MyKeyPackageRow {
    pub package_id: [u8; 32],
    pub package_blob: Vec<u8>,
    pub private_key: Vec<u8>,
    pub created_at: i64,
    pub consumed_at: Option<i64>,
    pub is_last_resort: bool,
}

pub struct MyKeyPackagesRepo<'a> {
    db: &'a Database,
}

impl<'a> MyKeyPackagesRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn insert(&self, row: &MyKeyPackageRow) -> Result<()> {
        self.db.with_tx(|tx| {
            tx.execute(
                "INSERT INTO my_keypackages (
                    package_id, package_blob, private_key, created_at, consumed_at, is_last_resort
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    &row.package_id[..],
                    row.package_blob,
                    row.private_key,
                    row.created_at,
                    row.consumed_at,
                    row.is_last_resort as i64,
                ],
            )?;
            Ok(())
        })
    }

    pub fn mark_consumed(&self, package_id: &[u8; 32], when: i64) -> Result<()> {
        self.db.with_tx(|tx| {
            let n = tx.execute(
                "UPDATE my_keypackages SET consumed_at = ?2
                  WHERE package_id = ?1 AND consumed_at IS NULL",
                params![&package_id[..], when],
            )?;
            if n == 0 {
                return Err(StorageError::NotFound(format!(
                    "keypackage {} (or already consumed)",
                    hex::encode(package_id)
                )));
            }
            Ok(())
        })
    }

    pub fn list_available_one_time(&self) -> Result<Vec<MyKeyPackageRow>> {
        self.db.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT package_id, package_blob, private_key, created_at, consumed_at, is_last_resort
                   FROM my_keypackages
                  WHERE consumed_at IS NULL AND is_last_resort = 0
                  ORDER BY created_at ASC",
            )?;
            let rows = stmt
                .query_map([], |row| Ok(Self::row_to_struct(row)))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows.into_iter().collect()
        })
    }

    pub fn last_resort(&self) -> Result<Option<MyKeyPackageRow>> {
        self.db.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT package_id, package_blob, private_key, created_at, consumed_at, is_last_resort
                   FROM my_keypackages
                  WHERE is_last_resort = 1
                  LIMIT 1",
            )?;
            let mut rows = stmt.query([])?;
            match rows.next()? {
                Some(row) => Ok(Some(Self::row_to_struct(row)?)),
                None => Ok(None),
            }
        })
    }

    fn row_to_struct(row: &rusqlite::Row<'_>) -> Result<MyKeyPackageRow> {
        let pkg_id_bytes: Vec<u8> = row.get(0)?;
        if pkg_id_bytes.len() != 32 {
            return Err(StorageError::InvalidBlob {
                table: "my_keypackages",
                column: "package_id",
                detail: format!("expected 32 bytes, got {}", pkg_id_bytes.len()),
            });
        }
        let mut package_id = [0u8; 32];
        package_id.copy_from_slice(&pkg_id_bytes);
        let is_last_resort: i64 = row.get(5)?;
        Ok(MyKeyPackageRow {
            package_id,
            package_blob: row.get(1)?,
            private_key: row.get(2)?,
            created_at: row.get(3)?,
            consumed_at: row.get(4)?,
            is_last_resort: is_last_resort != 0,
        })
    }
}

impl Database {
    pub fn my_keypackages(&self) -> MyKeyPackagesRepo<'_> {
        MyKeyPackagesRepo::new(self)
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

    fn fake_kp(seed: u8, last_resort: bool, created_at: i64) -> MyKeyPackageRow {
        MyKeyPackageRow {
            package_id: [seed; 32],
            package_blob: vec![seed; 16],
            private_key: vec![seed; 8],
            created_at,
            consumed_at: None,
            is_last_resort: last_resort,
        }
    }

    #[test]
    fn insert_and_list_available() {
        let db = fresh_db();
        db.my_keypackages().insert(&fake_kp(1, false, 100)).unwrap();
        db.my_keypackages().insert(&fake_kp(2, false, 200)).unwrap();
        let avail = db.my_keypackages().list_available_one_time().unwrap();
        assert_eq!(avail.len(), 2);
        assert_eq!(avail[0].package_id, [1u8; 32]);
        assert_eq!(avail[1].package_id, [2u8; 32]);
    }

    #[test]
    fn last_resort_separated_from_one_time_list() {
        let db = fresh_db();
        db.my_keypackages().insert(&fake_kp(1, false, 0)).unwrap();
        db.my_keypackages().insert(&fake_kp(2, true, 0)).unwrap();
        let avail = db.my_keypackages().list_available_one_time().unwrap();
        assert_eq!(avail.len(), 1);
        assert_eq!(avail[0].package_id, [1u8; 32]);
        let lr = db.my_keypackages().last_resort().unwrap().unwrap();
        assert_eq!(lr.package_id, [2u8; 32]);
    }

    #[test]
    fn mark_consumed_excludes_from_list() {
        let db = fresh_db();
        db.my_keypackages().insert(&fake_kp(1, false, 0)).unwrap();
        db.my_keypackages()
            .mark_consumed(&[1u8; 32], 12345)
            .unwrap();
        let avail = db.my_keypackages().list_available_one_time().unwrap();
        assert!(avail.is_empty());
    }

    #[test]
    fn mark_consumed_twice_returns_not_found() {
        let db = fresh_db();
        db.my_keypackages().insert(&fake_kp(1, false, 0)).unwrap();
        db.my_keypackages().mark_consumed(&[1u8; 32], 1).unwrap();
        let err = db
            .my_keypackages()
            .mark_consumed(&[1u8; 32], 2)
            .unwrap_err();
        assert!(matches!(err, StorageError::NotFound(_)));
    }
}
