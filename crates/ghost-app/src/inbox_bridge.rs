//! Bridge from the Client's inbox processor to the Tauri event bus.
//!
//! Strategy: spawn `Client::start_inbox_processor` (which persists incoming
//! messages to the DB), and a sibling watcher task that polls the `messages`
//! table for new incoming rows since the last seen `sent_at` and emits a
//! `ghost://message-received` event for each.
//!
//! Polling is acceptable here: the watcher tick is 250ms, the loopback path
//! is microseconds, the perceived latency is identical to a callback-driven
//! design for the MVP-1 demo. A callback-driven refactor of `ghost-client` is
//! tracked as MVP-2 follow-up.

use crate::dto::InboundMessageEvent;
use ghost_client::Client;
use ghost_storage::{Direction, MessageRow};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const POLL_INTERVAL: Duration = Duration::from_millis(250);
pub const EVENT_NAME: &str = "ghost://message-received";

pub async fn start_with_event_bridge(
    client: Arc<Client>,
    app: AppHandle,
) -> ghost_client::Result<(tokio::task::JoinHandle<()>, tokio::task::JoinHandle<()>)> {
    // Snapshot the current max sent_at across all incoming messages so we don't
    // re-emit pre-existing history on every app start.
    let mut last_seen_sent_at: i64 = max_incoming_sent_at(&client).unwrap_or(0);

    let processor_handle = client.start_inbox_processor().await?;

    let watcher_client = client.clone();
    let watcher_app = app.clone();
    let watcher_handle = tokio::spawn(async move {
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;
            let new_rows = match scan_new_incoming(&watcher_client, last_seen_sent_at) {
                Ok(rows) => rows,
                Err(e) => {
                    tracing::warn!("inbox watcher scan failed: {e}");
                    continue;
                }
            };
            for row in new_rows {
                last_seen_sent_at = last_seen_sent_at.max(row.sent_at);
                let payload = InboundMessageEvent {
                    from_ghost_id: row.contact_id.to_string(),
                    content: row.content.clone(),
                    received_at: row.received_at.unwrap_or(row.sent_at),
                };
                if let Err(e) = watcher_app.emit(EVENT_NAME, payload) {
                    tracing::warn!("inbox event emit failed: {e}");
                }
            }
        }
    });

    Ok((processor_handle, watcher_handle))
}

fn max_incoming_sent_at(client: &Client) -> Option<i64> {
    let contacts = client.list_contacts().ok()?;
    let mut max_at: Option<i64> = None;
    for c in contacts {
        if let Ok(rows) = client.list_messages(&c.ghost_id, u32::MAX, 0) {
            for row in rows {
                if matches!(row.direction, Direction::Incoming) {
                    max_at = Some(max_at.map(|m| m.max(row.sent_at)).unwrap_or(row.sent_at));
                }
            }
        }
    }
    max_at
}

fn scan_new_incoming(
    client: &Client,
    after: i64,
) -> ghost_client::Result<Vec<MessageRow>> {
    let mut out = Vec::new();
    for c in client.list_contacts()? {
        for row in client.list_messages(&c.ghost_id, u32::MAX, 0)? {
            if matches!(row.direction, Direction::Incoming) && row.sent_at > after {
                out.push(row);
            }
        }
    }
    out.sort_by_key(|r| r.sent_at);
    Ok(out)
}
