-- User-defined folders for grouping contacts.
--   folders.id is a synthetic primary key, folders.name is what the user types.
--   contact_folders is the M2M join: a contact may live in 0..n folders.
--
-- Default folders ('all', 'archive') are NOT inserted here — they're implicit
-- views in the UI. Users only create their own custom folders.

CREATE TABLE folders (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL UNIQUE,
    icon        TEXT,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL
);

CREATE TABLE contact_folders (
    folder_id   INTEGER NOT NULL,
    contact_id  BLOB NOT NULL,
    PRIMARY KEY (folder_id, contact_id),
    FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE CASCADE,
    FOREIGN KEY (contact_id) REFERENCES contacts(ghost_id) ON DELETE CASCADE
);

CREATE INDEX idx_contact_folders_contact ON contact_folders(contact_id);
