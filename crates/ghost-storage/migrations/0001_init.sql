-- Ghost MVP-1 database schema, version 1.

-- Schema versioning: each applied migration appends a row.
CREATE TABLE IF NOT EXISTS schema_version (
    version    INTEGER PRIMARY KEY,
    applied_at INTEGER NOT NULL
);

-- Contacts.
CREATE TABLE contacts (
    ghost_id      BLOB PRIMARY KEY,
    display_name  TEXT,
    local_alias   TEXT,
    fingerprint   TEXT NOT NULL,
    added_at      INTEGER NOT NULL,
    last_endpoint TEXT,
    verification  INTEGER NOT NULL DEFAULT 0,
    notes         TEXT,
    blocked       INTEGER NOT NULL DEFAULT 0
);

-- MLS group state per 1-on-1 conversation.
CREATE TABLE mls_groups (
    group_id      BLOB PRIMARY KEY,
    contact_id    BLOB NOT NULL,
    state_blob    BLOB NOT NULL,
    current_epoch INTEGER NOT NULL,
    created_at    INTEGER NOT NULL,
    last_updated  INTEGER NOT NULL,
    FOREIGN KEY (contact_id) REFERENCES contacts(ghost_id)
);

CREATE INDEX idx_mls_groups_contact ON mls_groups(contact_id);

-- Our own published KeyPackages.
CREATE TABLE my_keypackages (
    package_id     BLOB PRIMARY KEY,
    package_blob   BLOB NOT NULL,
    private_key    BLOB NOT NULL,
    created_at     INTEGER NOT NULL,
    consumed_at    INTEGER,
    is_last_resort INTEGER NOT NULL DEFAULT 0
);

-- Messages.
CREATE TABLE messages (
    msg_uuid     BLOB PRIMARY KEY,
    contact_id   BLOB NOT NULL,
    direction    INTEGER NOT NULL,
    content_type INTEGER NOT NULL,
    content      TEXT NOT NULL,
    sent_at      INTEGER NOT NULL,
    received_at  INTEGER,
    status       INTEGER NOT NULL DEFAULT 0,
    reply_to     BLOB,
    expires_at   INTEGER,
    FOREIGN KEY (contact_id) REFERENCES contacts(ghost_id)
);

CREATE INDEX idx_messages_contact_time ON messages(contact_id, sent_at);
CREATE INDEX idx_messages_expires ON messages(expires_at) WHERE expires_at IS NOT NULL;

-- Outbox.
CREATE TABLE outbox (
    msg_uuid       BLOB PRIMARY KEY,
    recipient_id   BLOB NOT NULL,
    envelope_blob  BLOB NOT NULL,
    attempts       INTEGER NOT NULL DEFAULT 0,
    next_retry_at  INTEGER NOT NULL,
    last_error     TEXT
);

CREATE INDEX idx_outbox_retry ON outbox(next_retry_at);

-- Inbox dedup.
CREATE TABLE inbox_dedup (
    msg_uuid    BLOB PRIMARY KEY,
    received_at INTEGER NOT NULL
);

CREATE INDEX idx_inbox_dedup_time ON inbox_dedup(received_at);

-- Settings.
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
