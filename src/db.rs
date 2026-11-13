use rusqlite::{params, Connection, OpenFlags, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn path() -> Result<PathBuf, String> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .filter(|p| Path::new(p).is_absolute())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/share")))
        .ok_or("Cannot find the user data directory")?;
    Ok(base.join("amnessia/amnessia.db"))
}

pub fn open(path: &Path) -> Result<Connection, String> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let db = Connection::open(path).map_err(|e| e.to_string())?;
        db.execute_batch(include_str!("../db/schema.sql"))
            .map_err(|e| e.to_string())?;
        return Ok(db);
    }
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|e| e.to_string())?;
    db.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| e.to_string())?;
    let version: i64 = db
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if version == 1 {
        db.execute_batch(include_str!("../db/migrate-1-to-2.sql"))
            .map_err(|e| e.to_string())?;
    }
    if version == 1 || version == 2 {
        db.execute_batch(include_str!("../db/migrate-2-to-3.sql"))
            .map_err(|e| e.to_string())?;
    }
    if !(1..=3).contains(&version) {
        return Err(format!(
            "Unsupported database version {version}; expected 1, 2 or 3"
        ));
    }
    Ok(db)
}

pub fn episodes(db: &Connection) -> Result<Vec<(i64, String)>> {
    db.prepare("SELECT id, name FROM episodes ORDER BY is_default DESC, position, id")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect()
}
#[derive(Debug, Clone)]
pub struct Flash {
    pub id: i64,
    pub title: String,
    pub status: String,
    pub intensity: u8,
    pub due_at: Option<String>,
}

pub fn flashes(db: &Connection, episode: i64) -> Result<Vec<Flash>> {
    db.prepare("SELECT id, title, status, intensity, due_at FROM flashes WHERE episode_id = ? ORDER BY position, created_at, id")?
        .query_map([episode], |r| Ok(Flash { id:r.get(0)?, title:r.get(1)?, status:r.get(2)?, intensity:r.get(3)?, due_at:r.get(4)? }))?.collect()
}

pub fn add_episode(db: &Connection, name: &str) -> Result<()> {
    db.execute("INSERT INTO episodes (name, position) VALUES (?1, COALESCE((SELECT MAX(position) + 1 FROM episodes), 0))", [name.trim()])?;
    Ok(())
}
pub fn add_flash(
    db: &Connection,
    episode: i64,
    title: &str,
    due_at: Option<&str>,
    intensity: u8,
) -> Result<()> {
    db.execute("INSERT INTO flashes (episode_id, title, position, due_at, intensity) VALUES (?1, ?2, COALESCE((SELECT MAX(position) + 1 FROM flashes WHERE episode_id = ?1), 0), ?3, ?4)", params![episode,title.trim(),due_at,intensity])?;
    Ok(())
}

pub fn update_flash(
    db: &Connection,
    id: i64,
    title: &str,
    due_at: Option<&str>,
    intensity: u8,
) -> Result<()> {
    db.execute("UPDATE flashes SET title = ?2, intensity = ?3, notified = CASE WHEN due_at IS NOT ?4 THEN 0 ELSE notified END, status = CASE WHEN status = 'faded' AND due_at IS NOT ?4 THEN 'active' ELSE status END, due_at = ?4 WHERE id = ?1", params![id,title.trim(),intensity,due_at])?;
    Ok(())
}

pub fn due_reminders(db: &Connection) -> Result<Vec<(Flash, bool)>> {
    db.prepare("SELECT id, title, status, intensity, due_at, notified FROM flashes WHERE due_at IS NOT NULL AND datetime(due_at) <= datetime('now') AND (status = 'active' OR (status = 'faded' AND notified = 0))")?
       .query_map([], |r| Ok((Flash { id:r.get(0)?, title:r.get(1)?, status:r.get(2)?, intensity:r.get(3)?, due_at:r.get(4)? }, r.get(5)?)))?.collect()
}

pub fn fade(db: &Connection, id: i64) -> Result<()> {
    db.execute(
        "UPDATE flashes SET status = 'faded' WHERE id = ? AND status = 'active'",
        [id],
    )?;
    Ok(())
}
pub fn mark_notified(db: &Connection, id: i64) -> Result<()> {
    db.execute("UPDATE flashes SET notified = 1 WHERE id = ?", [id])?;
    Ok(())
}

pub fn set_status(db: &Connection, id: i64, status: &str) -> Result<()> {
    db.execute(
        "UPDATE flashes SET status = ? WHERE id = ?",
        params![status, id],
    )?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saves_episodes_and_flash_status() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(include_str!("../db/schema.sql")).unwrap();
        add_episode(&db, " Work ").unwrap();
        assert_eq!(episodes(&db).unwrap()[0].1, "Static");
        let (id, name) = episodes(&db).unwrap().remove(1);
        assert_eq!(name, "Work");
        add_flash(&db, id, " First ", Some("2030-01-01T09:30:00Z"), 2).unwrap();
        let flash = flashes(&db, id).unwrap().remove(0);
        assert_eq!(
            (
                flash.title.as_str(),
                flash.status.as_str(),
                flash.intensity,
                flash.due_at.as_deref()
            ),
            ("First", "active", 2, Some("2030-01-01T09:30:00Z"))
        );
        set_status(&db, flash.id, "resolved").unwrap();
        assert_eq!(flashes(&db, id).unwrap()[0].status, "resolved");
        assert!(add_flash(&db, 999, "orphan", None, 1).is_err());
        assert!(add_episode(&db, " ").is_err());
    }
    #[test]
    fn migrates_existing_database_without_moving_flashes() {
        let path =
            std::env::temp_dir().join(format!("amnessia-migration-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let old = Connection::open(&path).unwrap();
            old.execute_batch("CREATE TABLE episodes (id INTEGER PRIMARY KEY, name TEXT NOT NULL, position INTEGER NOT NULL DEFAULT 0); CREATE TABLE flashes (id INTEGER PRIMARY KEY, episode_id INTEGER NOT NULL REFERENCES episodes(id), title TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'active', position INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, due_at TEXT); INSERT INTO episodes (id, name) VALUES (10, 'Work'); INSERT INTO flashes (episode_id, title) VALUES (10, 'Existing'); PRAGMA user_version=1;").unwrap();
        }
        {
            let db = open(&path).unwrap();
            let items = episodes(&db).unwrap();
            assert_eq!(items[0].1, "Static");
            assert_eq!(items[1], (10, "Work".to_string()));
            assert_eq!(flashes(&db, 10).unwrap()[0].title, "Existing");
            add_flash(&db, items[0].0, "Uncategorized", None, 1).unwrap();
            assert_eq!(flashes(&db, items[0].0).unwrap()[0].title, "Uncategorized");
            assert!(db
                .execute(
                    "INSERT INTO episodes (name, is_default) VALUES ('Duplicate', 1)",
                    []
                )
                .is_err());
        }
        {
            let db = open(&path).unwrap();
            assert_eq!(episodes(&db).unwrap().len(), 2);
        }
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn due_reminders_fade_and_are_not_sent_twice() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(include_str!("../db/schema.sql")).unwrap();
        let episode = episodes(&db).unwrap()[0].0;
        add_flash(&db, episode, "Due", Some("2000-01-01T00:00:00Z"), 3).unwrap();
        add_flash(&db, episode, "Future", Some("2999-01-01T00:00:00Z"), 1).unwrap();
        add_flash(
            &db,
            episode,
            "Already done",
            Some("2000-01-01T00:00:00Z"),
            2,
        )
        .unwrap();
        let done = flashes(&db, episode).unwrap().pop().unwrap().id;
        set_status(&db, done, "resolved").unwrap();
        let due = due_reminders(&db).unwrap();
        assert_eq!(due.len(), 1);
        assert!(!due[0].1);
        fade(&db, due[0].0.id).unwrap();
        assert_eq!(due_reminders(&db).unwrap().len(), 1);
        mark_notified(&db, due[0].0.id).unwrap();
        assert!(due_reminders(&db).unwrap().is_empty());
        assert_eq!(flashes(&db, episode).unwrap()[0].status, "faded");
    }
    #[test]
    fn editing_preserves_or_reschedules_reminders() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(include_str!("../db/schema.sql")).unwrap();
        let episode = episodes(&db).unwrap()[0].0;
        add_flash(&db, episode, "Before", Some("2000-01-01T00:00:00Z"), 1).unwrap();
        let id = flashes(&db, episode).unwrap()[0].id;
        fade(&db, id).unwrap();
        mark_notified(&db, id).unwrap();
        update_flash(&db, id, "Renamed", Some("2000-01-01T00:00:00Z"), 2).unwrap();
        let f = flashes(&db, episode).unwrap().remove(0);
        assert_eq!(
            (f.title.as_str(), f.status.as_str(), f.intensity),
            ("Renamed", "faded", 2)
        );
        assert!(due_reminders(&db).unwrap().is_empty());
        update_flash(&db, id, "Renamed", Some("2999-01-01T00:00:00Z"), 3).unwrap();
        let f = flashes(&db, episode).unwrap().remove(0);
        assert_eq!(
            (f.status.as_str(), f.intensity, f.due_at.as_deref()),
            ("active", 3, Some("2999-01-01T00:00:00Z"))
        );
        let notified: i64 = db
            .query_row("SELECT notified FROM flashes WHERE id=?", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(notified, 0);
        update_flash(&db, id, "Renamed", None, 3).unwrap();
        assert!(flashes(&db, episode).unwrap()[0].due_at.is_none());
    }
}
