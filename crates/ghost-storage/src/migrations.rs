//! Schema migrations.
//!
//! Each migration is a `&str` of SQL embedded via `include_str!`. They are applied
//! in order if `schema_version` does not contain their version number. All
//! migrations run inside a single transaction so a partial failure rolls back.

use crate::{Database, Result, StorageError};
use std::time::{SystemTime, UNIX_EPOCH};

pub const APP_SCHEMA_VERSION: u32 = 3;

const MIGRATIONS: &[(u32, &str)] = &[
    (1, include_str!("../migrations/0001_init.sql")),
    (2, include_str!("../migrations/0002_add_contact_dk_pub.sql")),
    (3, include_str!("../migrations/0003_contacts_extras.sql")),
];

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Database {
    /// Apply any unapplied migrations.
    pub fn migrate(&self) -> Result<()> {
        // Ensure schema_version table exists outside the transaction (idempotent).
        self.with_conn(|c| {
            c.execute(
                "CREATE TABLE IF NOT EXISTS schema_version (
                    version    INTEGER PRIMARY KEY,
                    applied_at INTEGER NOT NULL
                )",
                [],
            )?;
            Ok(())
        })?;

        let current_version: u32 = self.with_conn(|c| {
            c.query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|v| v as u32)
            .map_err(Into::into)
        })?;

        if current_version > APP_SCHEMA_VERSION {
            return Err(StorageError::SchemaTooNew {
                db_version: current_version,
                app_version: APP_SCHEMA_VERSION,
            });
        }

        for &(version, sql) in MIGRATIONS {
            if version <= current_version {
                continue;
            }
            self.with_tx(|tx| {
                tx.execute_batch(sql).map_err(|e| StorageError::Migration {
                    version,
                    detail: e.to_string(),
                })?;
                tx.execute(
                    "INSERT INTO schema_version (version, applied_at) VALUES (?1, ?2)",
                    rusqlite::params![version, now_seconds()],
                )?;
                Ok(())
            })?;
        }
        Ok(())
    }

    /// Read the current schema version (highest applied).
    pub fn schema_version(&self) -> Result<u32> {
        self.with_conn(|c| {
            c.query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|v| v as u32)
            .map_err(Into::into)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master_key::derive_master_key;
    use ghost_identity::IdentityKey;

    fn fresh_db() -> Database {
        Database::open_in_memory(&derive_master_key(&IdentityKey::generate())).unwrap()
    }

    #[test]
    fn migrate_brings_fresh_db_to_app_version() {
        let db = fresh_db();
        db.migrate().unwrap();
        assert_eq!(db.schema_version().unwrap(), APP_SCHEMA_VERSION);
    }

    #[test]
    fn migrate_is_idempotent() {
        let db = fresh_db();
        db.migrate().unwrap();
        db.migrate().unwrap();
        db.migrate().unwrap();
        assert_eq!(db.schema_version().unwrap(), APP_SCHEMA_VERSION);
    }

    #[test]
    fn after_migrate_all_app_tables_exist() {
        let db = fresh_db();
        db.migrate().unwrap();
        let tables: Vec<String> = db
            .with_conn(|c| {
                let mut stmt = c.prepare(
                    "SELECT name FROM sqlite_master \
                     WHERE type = 'table' AND name NOT LIKE 'sqlite_%' \
                     ORDER BY name",
                )?;
                let rows = stmt
                    .query_map([], |r| r.get::<_, String>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .unwrap();
        assert_eq!(
            tables,
            vec![
                "contacts".to_string(),
                "inbox_dedup".to_string(),
                "messages".to_string(),
                "mls_groups".to_string(),
                "my_keypackages".to_string(),
                "outbox".to_string(),
                "schema_version".to_string(),
                "settings".to_string(),
            ]
        );
    }

    #[test]
    fn schema_too_new_returns_error() {
        let db = fresh_db();
        db.migrate().unwrap();
        db.with_conn(|c| {
            c.execute(
                "INSERT INTO schema_version (version, applied_at) VALUES (?1, ?2)",
                rusqlite::params![APP_SCHEMA_VERSION + 99, 0_i64],
            )?;
            Ok(())
        })
        .unwrap();
        let err = db.migrate().unwrap_err();
        assert!(matches!(err, StorageError::SchemaTooNew { .. }));
    }
}
