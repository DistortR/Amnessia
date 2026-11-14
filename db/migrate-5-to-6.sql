PRAGMA foreign_keys = ON;
BEGIN;
ALTER TABLE flashes ADD COLUMN notes TEXT NOT NULL DEFAULT '';
PRAGMA user_version = 6;
COMMIT;
