PRAGMA foreign_keys = ON;
BEGIN;

CREATE TABLE episodes (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    position INTEGER NOT NULL DEFAULT 0,
    is_default INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1))
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
    notified INTEGER NOT NULL DEFAULT 0 CHECK (notified IN (0, 1))
);

CREATE INDEX flashes_episode_order ON flashes(episode_id, position, id);
PRAGMA user_version = 3;
COMMIT;
