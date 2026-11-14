PRAGMA foreign_keys = ON;
BEGIN;
ALTER TABLE flashes ADD COLUMN repeat_mode TEXT NOT NULL DEFAULT 'none' CHECK (repeat_mode IN ('none', 'interval', 'weekdays'));
ALTER TABLE flashes ADD COLUMN repeat_every INTEGER CHECK (repeat_every BETWEEN 1 AND 365);
ALTER TABLE flashes ADD COLUMN repeat_unit TEXT CHECK (repeat_unit IN ('minutes', 'hours', 'days', 'weeks'));
ALTER TABLE flashes ADD COLUMN repeat_weekdays INTEGER NOT NULL DEFAULT 0 CHECK (repeat_weekdays BETWEEN 0 AND 127);
CREATE TABLE flash_early_reminders (
    flash_id INTEGER NOT NULL REFERENCES flashes(id) ON DELETE CASCADE,
    offset_minutes INTEGER NOT NULL CHECK (offset_minutes BETWEEN 1 AND 525600),
    PRIMARY KEY (flash_id, offset_minutes)
);
CREATE TABLE flash_notification_log (
    flash_id INTEGER NOT NULL REFERENCES flashes(id) ON DELETE CASCADE,
    occurrence_at TEXT NOT NULL,
    offset_minutes INTEGER NOT NULL CHECK (offset_minutes BETWEEN 0 AND 525600),
    PRIMARY KEY (flash_id, occurrence_at, offset_minutes)
);
INSERT INTO flash_notification_log (flash_id, occurrence_at, offset_minutes)
SELECT id, due_at, 0 FROM flashes WHERE due_at IS NOT NULL AND notified = 1;
PRAGMA user_version = 5;
COMMIT;
