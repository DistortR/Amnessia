BEGIN;
ALTER TABLE episodes ADD COLUMN is_default INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1));
CREATE UNIQUE INDEX one_default_episode ON episodes(is_default) WHERE is_default = 1;
INSERT INTO episodes (name, is_default) VALUES ('Static', 1);
PRAGMA user_version = 2;
COMMIT;
