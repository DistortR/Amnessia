PRAGMA foreign_keys = ON;
BEGIN;

CREATE TABLE episodes (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    position INTEGER NOT NULL DEFAULT 0,
    is_default INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
    emoji TEXT NOT NULL DEFAULT '',
    color TEXT NOT NULL DEFAULT '#594187' CHECK (color GLOB '#[0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f]')
);

CREATE UNIQUE INDEX one_default_episode ON episodes(is_default) WHERE is_default = 1;
INSERT INTO episodes (name, is_default) VALUES ('Static', 1);

CREATE TABLE flashes (
    id INTEGER PRIMARY KEY,
    episode_id INTEGER NOT NULL REFERENCES episodes(id) ON DELETE RESTRICT,
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'resolved', 'faded')),
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    due_at TEXT,
    intensity INTEGER NOT NULL DEFAULT 1 CHECK (intensity BETWEEN 1 AND 3),
    notified INTEGER NOT NULL DEFAULT 0 CHECK (notified IN (0, 1)),
    repeat_mode TEXT NOT NULL DEFAULT 'none' CHECK (repeat_mode IN ('none', 'interval', 'weekdays')),
    repeat_every INTEGER CHECK (repeat_every BETWEEN 1 AND 365),
    repeat_unit TEXT CHECK (repeat_unit IN ('minutes', 'hours', 'days', 'weeks')),
    repeat_weekdays INTEGER NOT NULL DEFAULT 0 CHECK (repeat_weekdays BETWEEN 0 AND 127)
);

CREATE INDEX flashes_episode_order ON flashes(episode_id, position, id);
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
PRAGMA user_version = 5;
COMMIT;
