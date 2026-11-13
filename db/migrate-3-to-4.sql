BEGIN;
ALTER TABLE episodes ADD COLUMN emoji TEXT NOT NULL DEFAULT '';
ALTER TABLE episodes ADD COLUMN color TEXT NOT NULL DEFAULT '#594187' CHECK (color GLOB '#[0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f]');
PRAGMA user_version = 4;
COMMIT;
