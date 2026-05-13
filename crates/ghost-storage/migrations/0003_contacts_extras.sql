-- Per-contact UI state for the sidebar redesign:
--   last_read_at      — UNIX seconds; messages with received_at > this count as unread
--   pinned            — 0/1; pinned chats sort first in the list
--   muted             — 0/1; suppresses notifications (UI-only until OS notifications land)
--   retention_seconds — NULL = forever; otherwise expires_at on new messages = now + seconds
--
-- All columns get safe defaults so existing rows from migration 0001/0002 remain valid.

ALTER TABLE contacts ADD COLUMN last_read_at INTEGER NOT NULL DEFAULT 0;
ALTER TABLE contacts ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
ALTER TABLE contacts ADD COLUMN muted INTEGER NOT NULL DEFAULT 0;
ALTER TABLE contacts ADD COLUMN retention_seconds INTEGER;
