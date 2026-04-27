-- Plan 06 migration: add 32-byte DK public key column to contacts.
ALTER TABLE contacts ADD COLUMN dk_pub BLOB;
