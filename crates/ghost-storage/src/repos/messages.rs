//! Messages repository.

use crate::{Database, Result, StorageError};
use ghost_core::GhostId;
use rusqlite::params;

#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Outgoing = 0,
    Incoming = 1,
}

impl Direction {
    pub fn from_i64(v: i64) -> Result<Self> {
        match v {
            0 => Ok(Self::Outgoing),
            1 => Ok(Self::Incoming),
            other => Err(StorageError::Invalid(format!("direction {other}"))),
        }
    }
}

#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageStatus {
    Pending = 0,
    Sent = 1,
    Delivered = 2,
    Read = 3,
    Failed = 4,
}

impl MessageStatus {
    pub fn from_i64(v: i64) -> Result<Self> {
        match v {
            0 => Ok(Self::Pending),
            1 => Ok(Self::Sent),
            2 => Ok(Self::Delivered),
            3 => Ok(Self::Read),
            4 => Ok(Self::Failed),
            other => Err(StorageError::Invalid(format!("message status {other}"))),
        }
    }
}

#[derive(Clone, Debug)]
pub struct MessageRow {
    pub msg_uuid: [u8; 16],
    pub contact_id: GhostId,
    pub direction: Direction,
    pub content_type: i64,
    pub content: String,
    pub sent_at: i64,
    pub received_at: Option<i64>,
    pub status: MessageStatus,
    pub reply_to: Option<[u8; 16]>,
    pub expires_at: Option<i64>,
}

pub struct MessagesRepo<'a> {
    db: &'a Database,
}

impl<'a> MessagesRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn insert(&self, msg: &MessageRow) -> Result<()> {
        self.db.with_tx(|tx| {
            tx.execute(
                "INSERT INTO messages (
                    msg_uuid, contact_id, direction, content_type, content,
                    sent_at, received_at, status, reply_to, expires_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    &msg.msg_uuid[..],
                    msg.contact_id.as_bytes(),
                    msg.direction as i64,
                    msg.content_type,
                    msg.content,
                    msg.sent_at,
                    msg.received_at,
                    msg.status as i64,
                    msg.reply_to.as_ref().map(|b| &b[..]),
                    msg.expires_at,
                ],
            )?;
            Ok(())
        })
    }

    /// Page through messages for a contact, ordered by sent_at ascending.
    pub fn list_for_contact(
        &self,
        contact: &GhostId,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MessageRow>> {
        self.db.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT msg_uuid, contact_id, direction, content_type, content,
                        sent_at, received_at, status, reply_to, expires_at
                   FROM messages
                  WHERE contact_id = ?1
                  ORDER BY sent_at ASC
                  LIMIT ?2 OFFSET ?3",
            )?;
            let rows = stmt
                .query_map(
                    params![contact.as_bytes(), limit as i64, offset as i64],
                    |row| Ok(Self::row_to_struct(row)),
                )?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows.into_iter().collect()
        })
    }

    pub fn update_status(&self, msg_uuid: &[u8; 16], status: MessageStatus) -> Result<()> {
        self.db.with_tx(|tx| {
            let n = tx.execute(
                "UPDATE messages SET status = ?2 WHERE msg_uuid = ?1",
                params![&msg_uuid[..], status as i64],
            )?;
            if n == 0 {
                return Err(StorageError::NotFound(format!(
                    "message {}",
                    hex::encode(msg_uuid)
                )));
            }
            Ok(())
        })
    }

    /// Delete messages whose `expires_at` is past `now`.
    pub fn purge_expired(&self, now: i64) -> Result<usize> {
        self.db.with_tx(|tx| {
            let n = tx.execute(
                "DELETE FROM messages WHERE expires_at IS NOT NULL AND expires_at < ?1",
                params![now],
            )?;
            Ok(n)
        })
    }

    fn row_to_struct(row: &rusqlite::Row<'_>) -> Result<MessageRow> {
        let uuid_bytes: Vec<u8> = row.get(0)?;
        if uuid_bytes.len() != 16 {
            return Err(StorageError::InvalidBlob {
                table: "messages",
                column: "msg_uuid",
                detail: format!("expected 16 bytes, got {}", uuid_bytes.len()),
            });
        }
        let mut msg_uuid = [0u8; 16];
        msg_uuid.copy_from_slice(&uuid_bytes);

        let contact_bytes: Vec<u8> = row.get(1)?;
        if contact_bytes.len() != 32 {
            return Err(StorageError::InvalidBlob {
                table: "messages",
                column: "contact_id",
                detail: format!("expected 32 bytes, got {}", contact_bytes.len()),
            });
        }
        let mut contact_arr = [0u8; 32];
        contact_arr.copy_from_slice(&contact_bytes);

        let reply_to_bytes: Option<Vec<u8>> = row.get(8)?;
        let reply_to = match reply_to_bytes {
            None => None,
            Some(b) => {
                if b.len() != 16 {
                    return Err(StorageError::InvalidBlob {
                        table: "messages",
                        column: "reply_to",
                        detail: format!("expected 16 bytes, got {}", b.len()),
                    });
                }
                let mut arr = [0u8; 16];
                arr.copy_from_slice(&b);
                Some(arr)
            }
        };

        let direction: i64 = row.get(2)?;
        let status: i64 = row.get(7)?;

        Ok(MessageRow {
            msg_uuid,
            contact_id: GhostId::from_bytes(contact_arr),
            direction: Direction::from_i64(direction)?,
            content_type: row.get(3)?,
            content: row.get(4)?,
            sent_at: row.get(5)?,
            received_at: row.get(6)?,
            status: MessageStatus::from_i64(status)?,
            reply_to,
            expires_at: row.get(9)?,
        })
    }
}

impl Database {
    pub fn messages(&self) -> MessagesRepo<'_> {
        MessagesRepo::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master_key::derive_master_key;
    use crate::repos::contacts::{Contact, Verification};
    use ghost_core::Fingerprint;
    use ghost_identity::IdentityKey;

    fn fresh_db_with_contact(seed: u8) -> (Database, GhostId) {
        let db = Database::open_in_memory(&derive_master_key(&IdentityKey::generate())).unwrap();
        db.migrate().unwrap();
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
        (db, id)
    }

    fn msg(uuid_seed: u8, contact: GhostId, direction: Direction, sent_at: i64) -> MessageRow {
        MessageRow {
            msg_uuid: [uuid_seed; 16],
            contact_id: contact,
            direction,
            content_type: 0,
            content: format!("hello-{uuid_seed}"),
            sent_at,
            received_at: if direction == Direction::Incoming {
                Some(sent_at + 1)
            } else {
                None
            },
            status: MessageStatus::Pending,
            reply_to: None,
            expires_at: None,
        }
    }

    #[test]
    fn insert_then_list() {
        let (db, contact) = fresh_db_with_contact(1);
        db.messages()
            .insert(&msg(1, contact, Direction::Outgoing, 100))
            .unwrap();
        db.messages()
            .insert(&msg(2, contact, Direction::Incoming, 200))
            .unwrap();
        let list = db.messages().list_for_contact(&contact, 100, 0).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].content, "hello-1");
        assert_eq!(list[1].content, "hello-2");
    }

    #[test]
    fn list_paginates() {
        let (db, contact) = fresh_db_with_contact(2);
        for i in 0..10u8 {
            db.messages()
                .insert(&msg(i + 1, contact, Direction::Outgoing, i as i64))
                .unwrap();
        }
        let page = db.messages().list_for_contact(&contact, 3, 5).unwrap();
        assert_eq!(page.len(), 3);
        assert_eq!(page[0].content, "hello-6");
        assert_eq!(page[2].content, "hello-8");
    }

    #[test]
    fn update_status_changes_field() {
        let (db, contact) = fresh_db_with_contact(3);
        db.messages()
            .insert(&msg(1, contact, Direction::Outgoing, 0))
            .unwrap();
        db.messages()
            .update_status(&[1u8; 16], MessageStatus::Delivered)
            .unwrap();
        let list = db.messages().list_for_contact(&contact, 10, 0).unwrap();
        assert_eq!(list[0].status, MessageStatus::Delivered);
    }

    #[test]
    fn purge_expired_removes_only_expired() {
        let (db, contact) = fresh_db_with_contact(4);
        let mut keep = msg(1, contact, Direction::Outgoing, 0);
        keep.expires_at = Some(2_000_000_000);
        let mut go = msg(2, contact, Direction::Outgoing, 0);
        go.expires_at = Some(1_700_000_000);
        db.messages().insert(&keep).unwrap();
        db.messages().insert(&go).unwrap();
        let removed = db.messages().purge_expired(1_800_000_000).unwrap();
        assert_eq!(removed, 1);
        let remaining = db.messages().list_for_contact(&contact, 10, 0).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].msg_uuid, [1u8; 16]);
    }
}
