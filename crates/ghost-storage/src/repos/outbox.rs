//! Outbox repository: outgoing envelopes awaiting send.

use crate::{Database, Result, StorageError};
use ghost_core::GhostId;
use rusqlite::params;

#[derive(Clone, Debug)]
pub struct OutboxRow {
    pub msg_uuid: [u8; 16],
    pub recipient_id: GhostId,
    pub envelope_blob: Vec<u8>,
    pub attempts: u32,
    pub next_retry_at: i64,
    pub last_error: Option<String>,
}

pub struct OutboxRepo<'a> {
    db: &'a Database,
}

impl<'a> OutboxRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn enqueue(&self, row: &OutboxRow) -> Result<()> {
        self.db.with_tx(|tx| {
            tx.execute(
                "INSERT INTO outbox (msg_uuid, recipient_id, envelope_blob, attempts, next_retry_at, last_error)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    &row.msg_uuid[..],
                    row.recipient_id.as_bytes(),
                    row.envelope_blob,
                    row.attempts as i64,
                    row.next_retry_at,
                    row.last_error,
                ],
            )?;
            Ok(())
        })
    }

    pub fn due(&self, now: i64, limit: u32) -> Result<Vec<OutboxRow>> {
        self.db.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT msg_uuid, recipient_id, envelope_blob, attempts, next_retry_at, last_error
                   FROM outbox
                  WHERE next_retry_at <= ?1
                  ORDER BY next_retry_at ASC
                  LIMIT ?2",
            )?;
            let rows = stmt
                .query_map(params![now, limit as i64], |row| Ok(Self::row_to_struct(row)))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows.into_iter().collect()
        })
    }

    pub fn remove(&self, msg_uuid: &[u8; 16]) -> Result<()> {
        self.db.with_tx(|tx| {
            tx.execute("DELETE FROM outbox WHERE msg_uuid = ?1", params![&msg_uuid[..]])?;
            Ok(())
        })
    }

    pub fn record_failure(
        &self,
        msg_uuid: &[u8; 16],
        next_retry_at: i64,
        error: Option<&str>,
    ) -> Result<()> {
        self.db.with_tx(|tx| {
            let n = tx.execute(
                "UPDATE outbox
                    SET attempts = attempts + 1,
                        next_retry_at = ?2,
                        last_error = ?3
                  WHERE msg_uuid = ?1",
                params![&msg_uuid[..], next_retry_at, error],
            )?;
            if n == 0 {
                return Err(StorageError::NotFound(format!(
                    "outbox row {}",
                    hex::encode(msg_uuid)
                )));
            }
            Ok(())
        })
    }

    fn row_to_struct(row: &rusqlite::Row<'_>) -> Result<OutboxRow> {
        let uuid_bytes: Vec<u8> = row.get(0)?;
        if uuid_bytes.len() != 16 {
            return Err(StorageError::InvalidBlob {
                table: "outbox",
                column: "msg_uuid",
                detail: format!("expected 16, got {}", uuid_bytes.len()),
            });
        }
        let mut msg_uuid = [0u8; 16];
        msg_uuid.copy_from_slice(&uuid_bytes);
        let recipient_bytes: Vec<u8> = row.get(1)?;
        if recipient_bytes.len() != 32 {
            return Err(StorageError::InvalidBlob {
                table: "outbox",
                column: "recipient_id",
                detail: format!("expected 32, got {}", recipient_bytes.len()),
            });
        }
        let mut rid = [0u8; 32];
        rid.copy_from_slice(&recipient_bytes);
        let attempts_i64: i64 = row.get(3)?;
        Ok(OutboxRow {
            msg_uuid,
            recipient_id: GhostId::from_bytes(rid),
            envelope_blob: row.get(2)?,
            attempts: attempts_i64 as u32,
            next_retry_at: row.get(4)?,
            last_error: row.get(5)?,
        })
    }
}

impl Database {
    pub fn outbox(&self) -> OutboxRepo<'_> {
        OutboxRepo::new(self)
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

    fn row(seed: u8, next: i64) -> OutboxRow {
        OutboxRow {
            msg_uuid: [seed; 16],
            recipient_id: GhostId::from_bytes([seed; 32]),
            envelope_blob: vec![seed],
            attempts: 0,
            next_retry_at: next,
            last_error: None,
        }
    }

    #[test]
    fn enqueue_and_due() {
        let db = fresh_db();
        db.outbox().enqueue(&row(1, 100)).unwrap();
        db.outbox().enqueue(&row(2, 200)).unwrap();
        let due = db.outbox().due(150, 10).unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].msg_uuid, [1u8; 16]);
    }

    #[test]
    fn remove_dequeues() {
        let db = fresh_db();
        db.outbox().enqueue(&row(1, 0)).unwrap();
        db.outbox().remove(&[1u8; 16]).unwrap();
        let due = db.outbox().due(1000, 10).unwrap();
        assert!(due.is_empty());
    }

    #[test]
    fn record_failure_increments_attempts() {
        let db = fresh_db();
        db.outbox().enqueue(&row(1, 0)).unwrap();
        db.outbox().record_failure(&[1u8; 16], 500, Some("network down")).unwrap();
        let due = db.outbox().due(1000, 10).unwrap();
        assert_eq!(due[0].attempts, 1);
        assert_eq!(due[0].next_retry_at, 500);
        assert_eq!(due[0].last_error.as_deref(), Some("network down"));
    }
}
