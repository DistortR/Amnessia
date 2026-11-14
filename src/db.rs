use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Result, Row};
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
    if version == 1 || version == 2 || version == 3 {
        db.execute_batch(include_str!("../db/migrate-3-to-4.sql"))
            .map_err(|e| e.to_string())?;
    }
    if (1..=4).contains(&version) {
        db.execute_batch(include_str!("../db/migrate-4-to-5.sql"))
            .map_err(|e| e.to_string())?;
    }
    if !(1..=5).contains(&version) {
        return Err(format!(
            "Unsupported database version {version}; expected 1, 2, 3, 4 or 5"
        ));
    }
    Ok(db)
}

#[derive(Debug, Clone)]
pub struct Episode {
    pub id: i64,
    pub name: String,
    pub emoji: String,
    pub color: String,
    pub is_default: bool,
}

pub fn episodes(db: &Connection) -> Result<Vec<Episode>> {
    db.prepare(
        "SELECT id, name, emoji, color, is_default FROM episodes ORDER BY is_default DESC, position, id",
    )?
    .query_map([], |r| {
        Ok(Episode {
            id: r.get(0)?,
            name: r.get(1)?,
            emoji: r.get(2)?,
            color: r.get(3)?,
                is_default: r.get(4)?,
        })
    })?
    .collect()
}
#[derive(Debug, Clone)]
pub struct Flash {
    pub id: i64,
    pub title: String,
    pub status: String,
    pub intensity: u8,
    pub due_at: Option<String>,
    pub early_reminders: Vec<i64>,
    pub repeat_rule: RepeatRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepeatRule {
    None,
    Interval { every: u32, unit: RepeatUnit },
    Weekdays { days: u8 },
}

impl RepeatRule {
    pub fn is_repeating(&self) -> bool {
        !matches!(self, Self::None)
    }

    fn columns(&self) -> (&'static str, Option<u32>, Option<&'static str>, u8) {
        match self {
            Self::None => ("none", None, None, 0),
            Self::Interval { every, unit } => ("interval", Some(*every), Some(unit.as_str()), 0),
            Self::Weekdays { days } => ("weekdays", None, None, *days),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatUnit {
    Minutes,
    Hours,
    Days,
    Weeks,
}

impl RepeatUnit {
    fn as_str(self) -> &'static str {
        match self {
            Self::Minutes => "minutes",
            Self::Hours => "hours",
            Self::Days => "days",
            Self::Weeks => "weeks",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "minutes" => Some(Self::Minutes),
            "hours" => Some(Self::Hours),
            "days" => Some(Self::Days),
            "weeks" => Some(Self::Weeks),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PendingReminder {
    pub flash: Flash,
    pub occurrence_at: String,
    pub offset_minutes: i64,
}

fn flash_from_row(row: &Row<'_>, early_reminders: Vec<i64>) -> rusqlite::Result<Flash> {
    let repeat_mode: String = row.get(5)?;
    let repeat_every: Option<u32> = row.get(6)?;
    let repeat_unit: Option<String> = row.get(7)?;
    let repeat_days: u8 = row.get(8)?;
    let repeat_rule = match repeat_mode.as_str() {
        "none" => RepeatRule::None,
        "interval" => RepeatRule::Interval {
            every: repeat_every.ok_or(rusqlite::Error::InvalidQuery)?,
            unit: RepeatUnit::parse(
                repeat_unit
                    .as_deref()
                    .ok_or(rusqlite::Error::InvalidQuery)?,
            )
            .ok_or(rusqlite::Error::InvalidQuery)?,
        },
        "weekdays" => RepeatRule::Weekdays { days: repeat_days },
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(Flash {
        id: row.get(0)?,
        title: row.get(1)?,
        status: row.get(2)?,
        intensity: row.get(3)?,
        due_at: row.get(4)?,
        early_reminders,
        repeat_rule,
    })
}

fn load_early_reminders(db: &Connection, flash_id: i64) -> Result<Vec<i64>> {
    db.prepare(
        "SELECT offset_minutes FROM flash_early_reminders WHERE flash_id = ? ORDER BY offset_minutes",
    )?
    .query_map([flash_id], |r| r.get(0))?
    .collect()
}

pub fn flashes(db: &Connection, episode: i64) -> Result<Vec<Flash>> {
    let mut items = db
        .prepare("SELECT id, title, status, intensity, due_at, repeat_mode, repeat_every, repeat_unit, repeat_weekdays FROM flashes WHERE episode_id = ? ORDER BY position, created_at, id")?
        .query_map([episode], |r| flash_from_row(r, Vec::new()))?
        .collect::<Result<Vec<_>>>()?;
    for flash in &mut items {
        flash.early_reminders = load_early_reminders(db, flash.id)?;
    }
    Ok(items)
}

pub fn add_episode(db: &Connection, name: &str, emoji: &str, color: &str) -> Result<()> {
    db.execute("INSERT INTO episodes (name, position, emoji, color) VALUES (?1, COALESCE((SELECT MAX(position) + 1 FROM episodes), 0), ?2, ?3)", params![name.trim(), emoji.trim(), color])?;
    Ok(())
}

pub fn update_episode(
    db: &Connection,
    id: i64,
    name: &str,
    emoji: &str,
    color: &str,
) -> Result<()> {
    db.execute(
        "UPDATE episodes SET name = ?2, emoji = ?3, color = ?4 WHERE id = ?1 AND is_default = 0",
        params![id, name.trim(), emoji.trim(), color],
    )?;
    Ok(())
}
pub fn add_flash(
    db: &Connection,
    episode: i64,
    title: &str,
    due_at: Option<&str>,
    intensity: u8,
    early_reminders: &[i64],
    repeat_rule: &RepeatRule,
) -> Result<()> {
    validate_schedule(due_at, early_reminders, repeat_rule)?;
    let tx = db.unchecked_transaction()?;
    let (mode, every, unit, days) = repeat_rule.columns();
    tx.execute("INSERT INTO flashes (episode_id, title, position, due_at, intensity, repeat_mode, repeat_every, repeat_unit, repeat_weekdays) VALUES (?1, ?2, COALESCE((SELECT MAX(position) + 1 FROM flashes WHERE episode_id = ?1), 0), ?3, ?4, ?5, ?6, ?7, ?8)", params![episode,title.trim(),due_at,intensity,mode,every,unit,days])?;
    let id = tx.last_insert_rowid();
    save_early_reminders(&tx, id, early_reminders)?;
    tx.commit()?;
    Ok(())
}

pub fn update_flash(
    db: &Connection,
    id: i64,
    title: &str,
    due_at: Option<&str>,
    intensity: u8,
    early_reminders: &[i64],
    repeat_rule: &RepeatRule,
) -> Result<()> {
    validate_schedule(due_at, early_reminders, repeat_rule)?;
    let tx = db.unchecked_transaction()?;
    let (mode, every, unit, days) = repeat_rule.columns();
    tx.execute("UPDATE flashes SET title = ?2, intensity = ?3, notified = CASE WHEN due_at IS NOT ?4 THEN 0 ELSE notified END, status = CASE WHEN status = 'faded' AND due_at IS NOT ?4 THEN 'active' ELSE status END, due_at = ?4, repeat_mode = ?5, repeat_every = ?6, repeat_unit = ?7, repeat_weekdays = ?8 WHERE id = ?1", params![id,title.trim(),intensity,due_at,mode,every,unit,days])?;
    tx.execute(
        "DELETE FROM flash_notification_log WHERE flash_id = ?1 AND occurrence_at IS NOT ?2",
        params![id, due_at],
    )?;
    save_early_reminders(&tx, id, early_reminders)?;
    tx.commit()?;
    Ok(())
}

fn validate_schedule(
    due_at: Option<&str>,
    early_reminders: &[i64],
    repeat_rule: &RepeatRule,
) -> Result<()> {
    if due_at.is_none() && (!early_reminders.is_empty() || repeat_rule.is_repeating()) {
        return Err(rusqlite::Error::InvalidParameterName(
            "earlier reminders and repetition need a due date".into(),
        ));
    }
    if early_reminders
        .iter()
        .any(|offset| !(1..=525_600).contains(offset))
    {
        return Err(rusqlite::Error::InvalidParameterName(
            "reminder offset must be between 1 minute and 365 days".into(),
        ));
    }
    match repeat_rule {
        RepeatRule::None => {}
        RepeatRule::Interval { every, .. } if (1..=365).contains(every) => {}
        RepeatRule::Weekdays { days } if (1..=127).contains(days) => {}
        _ => {
            return Err(rusqlite::Error::InvalidParameterName(
                "invalid repeat schedule".into(),
            ));
        }
    }
    Ok(())
}

fn save_early_reminders(
    tx: &rusqlite::Transaction<'_>,
    flash_id: i64,
    offsets: &[i64],
) -> Result<()> {
    tx.execute(
        "DELETE FROM flash_early_reminders WHERE flash_id = ?1",
        [flash_id],
    )?;
    for offset in offsets
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
    {
        tx.execute(
            "INSERT INTO flash_early_reminders (flash_id, offset_minutes) VALUES (?1, ?2)",
            params![flash_id, offset],
        )?;
    }
    Ok(())
}

pub fn due_reminders(db: &Connection, now: &str) -> Result<Vec<PendingReminder>> {
    const DUE: &str = "SELECT f.id, f.title, f.status, f.intensity, f.due_at, f.repeat_mode, f.repeat_every, f.repeat_unit, f.repeat_weekdays, f.due_at AS occurrence_at, 0 AS offset_minutes FROM flashes f WHERE f.due_at IS NOT NULL AND datetime(f.due_at) <= datetime(?1) AND f.status != 'resolved' AND NOT EXISTS (SELECT 1 FROM flash_notification_log l WHERE l.flash_id = f.id AND l.occurrence_at = f.due_at AND l.offset_minutes = 0)";
    const EARLY: &str = "SELECT f.id, f.title, f.status, f.intensity, f.due_at, f.repeat_mode, f.repeat_every, f.repeat_unit, f.repeat_weekdays, f.due_at AS occurrence_at, r.offset_minutes FROM flashes f JOIN flash_early_reminders r ON r.flash_id = f.id WHERE f.due_at IS NOT NULL AND datetime(f.due_at) > datetime(?1) AND datetime(f.due_at, printf('-%d minutes', r.offset_minutes)) <= datetime(?1) AND f.status != 'resolved' AND NOT EXISTS (SELECT 1 FROM flash_notification_log l WHERE l.flash_id = f.id AND l.occurrence_at = f.due_at AND l.offset_minutes = r.offset_minutes)";
    let query = format!("{DUE} UNION ALL {EARLY} ORDER BY occurrence_at, offset_minutes DESC");
    db.prepare(&query)?
        .query_map([now], |r| {
            Ok(PendingReminder {
                flash: flash_from_row(r, Vec::new())?,
                occurrence_at: r.get(9)?,
                offset_minutes: r.get(10)?,
            })
        })?
        .collect()
}

pub fn fade(db: &Connection, id: i64) -> Result<()> {
    db.execute(
        "UPDATE flashes SET status = 'faded' WHERE id = ? AND status = 'active'",
        [id],
    )?;
    Ok(())
}
pub fn complete_reminder(
    db: &Connection,
    reminder: &PendingReminder,
    next_due_at: Option<&str>,
) -> Result<()> {
    let tx = db.unchecked_transaction()?;
    let current: Option<(String, Option<String>)> = tx
        .query_row(
            "SELECT status, due_at FROM flashes WHERE id = ?1",
            [reminder.flash.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((status, due_at)) = current else {
        tx.commit()?;
        return Ok(());
    };
    if status == "resolved" || due_at.as_deref() != Some(reminder.occurrence_at.as_str()) {
        tx.commit()?;
        return Ok(());
    }
    tx.execute(
        "INSERT OR IGNORE INTO flash_notification_log (flash_id, occurrence_at, offset_minutes) VALUES (?1, ?2, ?3)",
        params![reminder.flash.id, reminder.occurrence_at, reminder.offset_minutes],
    )?;
    if reminder.offset_minutes == 0 {
        if let Some(next_due_at) = next_due_at {
            tx.execute(
                "UPDATE flashes SET status = 'faded', due_at = ?2, notified = 0 WHERE id = ?1",
                params![reminder.flash.id, next_due_at],
            )?;
            tx.execute(
                "DELETE FROM flash_notification_log WHERE flash_id = ?1 AND occurrence_at IS NOT ?2",
                params![reminder.flash.id, next_due_at],
            )?;
        } else {
            tx.execute(
                "UPDATE flashes SET status = 'faded', notified = 1 WHERE id = ?1",
                [reminder.flash.id],
            )?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn set_status(db: &Connection, id: i64, status: &str, next_due_at: Option<&str>) -> Result<()> {
    if let Some(next_due_at) = next_due_at {
        let tx = db.unchecked_transaction()?;
        tx.execute(
            "UPDATE flashes SET status = ?2, due_at = ?3, notified = 0 WHERE id = ?1",
            params![id, status, next_due_at],
        )?;
        tx.execute(
            "DELETE FROM flash_notification_log WHERE flash_id = ?1",
            [id],
        )?;
        tx.commit()?;
    } else {
        db.execute(
            "UPDATE flashes SET status = ?1 WHERE id = ?2",
            params![status, id],
        )?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saves_episodes_and_flash_status() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(include_str!("../db/schema.sql")).unwrap();
        add_episode(&db, " Work ", "", "#594187").unwrap();
        assert_eq!(episodes(&db).unwrap()[0].name, "Static");
        let work = episodes(&db).unwrap().remove(1);
        let id = work.id;
        assert_eq!(work.name, "Work");
        add_flash(
            &db,
            id,
            " First ",
            Some("2030-01-01T09:30:00Z"),
            2,
            &[],
            &RepeatRule::None,
        )
        .unwrap();
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
        update_episode(&db, id, " Renamed ", "💡", "#3B82F6").unwrap();
        let renamed = episodes(&db).unwrap().remove(1);
        assert_eq!((renamed.id, renamed.name.as_str()), (id, "Renamed"));
        assert_eq!(flashes(&db, id).unwrap()[0].id, flash.id);
        assert!(update_episode(&db, id, " ", "", "#3B82F6").is_err());
        set_status(&db, flash.id, "resolved", None).unwrap();
        assert_eq!(flashes(&db, id).unwrap()[0].status, "resolved");
        assert!(add_flash(&db, 999, "orphan", None, 1, &[], &RepeatRule::None).is_err());
        assert!(add_episode(&db, " ", "", "#594187").is_err());
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
            assert_eq!(items[0].name, "Static");
            assert_eq!((items[1].id, items[1].name.as_str()), (10, "Work"));
            assert_eq!(flashes(&db, 10).unwrap()[0].title, "Existing");
            add_flash(
                &db,
                items[0].id,
                "Uncategorized",
                None,
                1,
                &[],
                &RepeatRule::None,
            )
            .unwrap();
            assert_eq!(flashes(&db, items[0].id).unwrap()[0].title, "Uncategorized");
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
        let episode = episodes(&db).unwrap()[0].id;
        add_flash(
            &db,
            episode,
            "Due",
            Some("2000-01-01T00:00:00Z"),
            3,
            &[],
            &RepeatRule::None,
        )
        .unwrap();
        add_flash(
            &db,
            episode,
            "Future",
            Some("2999-01-01T00:00:00Z"),
            1,
            &[],
            &RepeatRule::None,
        )
        .unwrap();
        add_flash(
            &db,
            episode,
            "Already done",
            Some("2000-01-01T00:00:00Z"),
            2,
            &[],
            &RepeatRule::None,
        )
        .unwrap();
        let done = flashes(&db, episode).unwrap().pop().unwrap().id;
        set_status(&db, done, "resolved", None).unwrap();
        let due = due_reminders(&db, "2026-11-13T00:00:00Z").unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].offset_minutes, 0);
        fade(&db, due[0].flash.id).unwrap();
        assert_eq!(due_reminders(&db, "2026-11-13T00:00:00Z").unwrap().len(), 1);
        complete_reminder(&db, &due[0], None).unwrap();
        assert!(due_reminders(&db, "2026-11-13T00:00:00Z")
            .unwrap()
            .is_empty());
        assert_eq!(flashes(&db, episode).unwrap()[0].status, "faded");
    }
    #[test]
    fn editing_preserves_or_reschedules_reminders() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(include_str!("../db/schema.sql")).unwrap();
        let episode = episodes(&db).unwrap()[0].id;
        add_flash(
            &db,
            episode,
            "Before",
            Some("2000-01-01T00:00:00Z"),
            1,
            &[],
            &RepeatRule::None,
        )
        .unwrap();
        let id = flashes(&db, episode).unwrap()[0].id;
        fade(&db, id).unwrap();
        let due = due_reminders(&db, "2026-11-13T00:00:00Z").unwrap();
        complete_reminder(&db, &due[0], None).unwrap();
        update_flash(
            &db,
            id,
            "Renamed",
            Some("2000-01-01T00:00:00Z"),
            2,
            &[],
            &RepeatRule::None,
        )
        .unwrap();
        let f = flashes(&db, episode).unwrap().remove(0);
        assert_eq!(
            (f.title.as_str(), f.status.as_str(), f.intensity),
            ("Renamed", "faded", 2)
        );
        assert!(due_reminders(&db, "2026-11-13T00:00:00Z")
            .unwrap()
            .is_empty());
        update_flash(
            &db,
            id,
            "Renamed",
            Some("2999-01-01T00:00:00Z"),
            3,
            &[],
            &RepeatRule::None,
        )
        .unwrap();
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
        update_flash(&db, id, "Renamed", None, 3, &[], &RepeatRule::None).unwrap();
        assert!(flashes(&db, episode).unwrap()[0].due_at.is_none());
    }

    #[test]
    fn v5_schema_persists_reminders_repeats_and_delivery_state() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(include_str!("../db/schema.sql")).unwrap();
        let episode = episodes(&db).unwrap()[0].id;
        add_flash(
            &db,
            episode,
            "Weekly",
            Some("2030-01-01T09:00:00Z"),
            2,
            &[15, 1440],
            &RepeatRule::Weekdays { days: 10 },
        )
        .unwrap();
        let id = flashes(&db, episode).unwrap()[0].id;
        db.execute(
            "INSERT INTO flash_notification_log (flash_id, occurrence_at, offset_minutes) VALUES (?1, '2030-01-01T09:00:00Z', 15)",
            [id],
        )
        .unwrap();

        let saved: (String, i64, i64) = db
            .query_row(
                "SELECT repeat_mode, repeat_weekdays, (SELECT COUNT(*) FROM flash_early_reminders WHERE flash_id = flashes.id) FROM flashes WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(saved, ("weekdays".into(), 10, 2));
        assert_eq!(
            flashes(&db, episode).unwrap()[0].early_reminders,
            [15, 1_440]
        );
        assert_eq!(
            flashes(&db, episode).unwrap()[0].repeat_rule,
            RepeatRule::Weekdays { days: 10 }
        );
        update_flash(
            &db,
            id,
            "Updated weekly",
            Some("2030-01-01T09:00:00Z"),
            3,
            &[30, 60],
            &RepeatRule::Interval {
                every: 2,
                unit: RepeatUnit::Weeks,
            },
        )
        .unwrap();
        let updated = flashes(&db, episode).unwrap().remove(0);
        assert_eq!(updated.title, "Updated weekly");
        assert_eq!(updated.intensity, 3);
        assert_eq!(updated.early_reminders, [30, 60]);
        assert_eq!(
            updated.repeat_rule,
            RepeatRule::Interval {
                every: 2,
                unit: RepeatUnit::Weeks
            }
        );
    }

    #[test]
    fn migrates_version_four_notifications_without_sending_them_again() {
        let path =
            std::env::temp_dir().join(format!("amnessia-v4-migration-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let old = Connection::open(&path).unwrap();
            old.execute_batch("CREATE TABLE episodes (id INTEGER PRIMARY KEY, name TEXT NOT NULL, position INTEGER NOT NULL DEFAULT 0, is_default INTEGER NOT NULL DEFAULT 0, emoji TEXT NOT NULL DEFAULT '', color TEXT NOT NULL DEFAULT '#594187'); CREATE TABLE flashes (id INTEGER PRIMARY KEY, episode_id INTEGER NOT NULL REFERENCES episodes(id), title TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'active', position INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, due_at TEXT, intensity INTEGER NOT NULL DEFAULT 1, notified INTEGER NOT NULL DEFAULT 0); INSERT INTO episodes (id, name, is_default) VALUES (1, 'Static', 1); INSERT INTO flashes (id, episode_id, title, status, due_at, notified) VALUES (9, 1, 'Already sent', 'faded', '2000-01-01T09:00:00Z', 1); PRAGMA user_version = 4;").unwrap();
        }
        {
            let db = open(&path).unwrap();
            let version: i64 = db
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .unwrap();
            assert_eq!(version, 5);
            assert!(due_reminders(&db, "2030-01-01T00:00:00Z")
                .unwrap()
                .is_empty());
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn selected_early_reminders_fire_at_each_offset_then_fade_at_due_time() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(include_str!("../db/schema.sql")).unwrap();
        let episode = episodes(&db).unwrap()[0].id;
        add_flash(
            &db,
            episode,
            "Dentist",
            Some("2030-01-01T09:00:00Z"),
            1,
            &[15, 60],
            &RepeatRule::None,
        )
        .unwrap();

        let one_hour = due_reminders(&db, "2030-01-01T08:00:00Z").unwrap();
        assert_eq!(one_hour.len(), 1);
        assert_eq!(one_hour[0].offset_minutes, 60);
        complete_reminder(&db, &one_hour[0], None).unwrap();
        assert_eq!(flashes(&db, episode).unwrap()[0].status, "active");

        let fifteen_minutes = due_reminders(&db, "2030-01-01T08:45:00Z").unwrap();
        assert_eq!(fifteen_minutes.len(), 1);
        assert_eq!(fifteen_minutes[0].offset_minutes, 15);
        complete_reminder(&db, &fifteen_minutes[0], None).unwrap();

        let due = due_reminders(&db, "2030-01-01T09:00:00Z").unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].offset_minutes, 0);
        fade(&db, due[0].flash.id).unwrap();
        complete_reminder(&db, &due[0], None).unwrap();
        assert_eq!(flashes(&db, episode).unwrap()[0].status, "faded");
        assert!(due_reminders(&db, "2030-01-01T09:01:00Z")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn repeating_reminder_advances_and_resolved_stops_the_next_occurrence() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(include_str!("../db/schema.sql")).unwrap();
        let episode = episodes(&db).unwrap()[0].id;
        add_flash(
            &db,
            episode,
            "Medication",
            Some("2030-01-01T09:00:00Z"),
            2,
            &[],
            &RepeatRule::Interval {
                every: 2,
                unit: RepeatUnit::Hours,
            },
        )
        .unwrap();
        let due = due_reminders(&db, "2030-01-01T12:15:00Z").unwrap();
        assert_eq!(due.len(), 1);
        fade(&db, due[0].flash.id).unwrap();
        complete_reminder(&db, &due[0], Some("2030-01-01T13:00:00Z")).unwrap();
        let flash = flashes(&db, episode).unwrap().remove(0);
        assert_eq!(flash.status, "faded");
        assert_eq!(flash.due_at.as_deref(), Some("2030-01-01T13:00:00Z"));
        assert!(due_reminders(&db, "2030-01-01T12:59:00Z")
            .unwrap()
            .is_empty());
        set_status(&db, flash.id, "resolved", None).unwrap();
        assert!(due_reminders(&db, "2030-01-01T13:00:00Z")
            .unwrap()
            .is_empty());
    }
}
