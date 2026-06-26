// SQLite "memory" — the undo/move log (and room to grow: learned decisions).

use rusqlite::Connection;
use std::path::PathBuf;

fn db_file() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| dirs::home_dir().expect("home"));
    base.join("com.xaviour.zortbit").join("zortbit.db")
}

pub fn open() -> rusqlite::Result<Connection> {
    let p = db_file();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let conn = Connection::open(p)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS moves (
            id        TEXT PRIMARY KEY,
            from_path TEXT NOT NULL,
            to_path   TEXT NOT NULL,
            action    TEXT NOT NULL,
            ts        TEXT NOT NULL,
            undone    INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS decisions (
            ts             TEXT NOT NULL,
            decision       TEXT NOT NULL,
            action         TEXT NOT NULL,
            ext            TEXT,
            current_name   TEXT,
            suggested_name TEXT,
            target_folder  TEXT,
            source         TEXT
        );
        CREATE TABLE IF NOT EXISTS folders (
            name        TEXT PRIMARY KEY,
            source_path TEXT NOT NULL,
            file_count  INTEGER NOT NULL,
            tokens      TEXT NOT NULL,
            updated     TEXT NOT NULL
        );",
    )?;
    Ok(conn)
}

// Cache the learned folder fingerprints (full replace — discovery is cheap).
pub fn save_folders(conn: &Connection, folders: &[crate::learn::Folder]) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM folders", [])?;
    let now = chrono::Local::now().to_rfc3339();
    let mut stmt = conn.prepare(
        "INSERT INTO folders (name, source_path, file_count, tokens, updated) VALUES (?1,?2,?3,?4,?5)",
    )?;
    for f in folders {
        let toks = serde_json::to_string(&f.tokens).unwrap_or_else(|_| "{}".into());
        stmt.execute(rusqlite::params![
            f.name,
            f.source_path,
            f.file_count as i64,
            toks,
            now
        ])?;
    }
    Ok(())
}

pub fn load_folders(conn: &Connection) -> Vec<crate::learn::Folder> {
    let mut stmt = match conn.prepare("SELECT name, source_path, file_count, tokens FROM folders") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map([], |r| {
        let name: String = r.get(0)?;
        let source_path: String = r.get(1)?;
        let fc: i64 = r.get(2)?;
        let toks: String = r.get(3)?;
        let tokens = serde_json::from_str(&toks).unwrap_or_default();
        Ok(crate::learn::Folder {
            name,
            source_path,
            file_count: fc as u32,
            tokens,
        })
    });
    match rows {
        Ok(it) => it.filter_map(|x| x.ok()).collect(),
        Err(_) => Vec::new(),
    }
}

pub fn log_move(
    conn: &Connection,
    id: &str,
    from: &str,
    to: &str,
    action: &str,
    ts: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO moves (id, from_path, to_path, action, ts) VALUES (?1,?2,?3,?4,?5)",
        rusqlite::params![id, from, to, action, ts],
    )?;
    Ok(())
}

pub fn get_move(conn: &Connection, id: &str) -> rusqlite::Result<Option<(String, String, String)>> {
    let mut stmt =
        conn.prepare("SELECT from_path, to_path, action FROM moves WHERE id=?1 AND undone=0")?;
    let mut rows = stmt.query([id])?;
    if let Some(row) = rows.next()? {
        Ok(Some((row.get(0)?, row.get(1)?, row.get(2)?)))
    } else {
        Ok(None)
    }
}

pub fn mark_undone(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("UPDATE moves SET undone=1 WHERE id=?1", [id])?;
    Ok(())
}

// Aggregate the user's confirmed filing habits — COUNTS ONLY, never filenames,
// so sensitive names can't leak into the model prompt. Returns (ext, folder, n).
pub fn learned_hints(conn: &Connection) -> rusqlite::Result<Vec<(String, String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT ext, target_folder, COUNT(*) AS n
         FROM decisions
         WHERE decision='approve' AND action='move' AND source='local'
           AND ext IS NOT NULL AND ext <> ''
           AND target_folder IS NOT NULL AND target_folder <> ''
         GROUP BY ext, target_folder
         HAVING n >= 2
         ORDER BY n DESC
         LIMIT 8",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    rows.collect()
}

// How many times the user has approved this (ext -> category) move. Auto-mode
// uses this as the "trusted pattern" signal to act without asking.
pub fn trusted_count(conn: &Connection, ext: &str, cat: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM decisions
         WHERE decision='approve' AND action='move' AND source='local'
           AND ext=?1 AND target_folder LIKE '%/' || ?2",
        rusqlite::params![ext, cat],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

// Log every approve/skip so Zortbit can later learn the user's habits
// (e.g. "you usually delete .dmg installers").
#[allow(clippy::too_many_arguments)]
pub fn log_decision(
    conn: &Connection,
    ts: &str,
    decision: &str,
    action: &str,
    ext: &str,
    current_name: &str,
    suggested_name: &str,
    target_folder: &str,
    source: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO decisions (ts, decision, action, ext, current_name, suggested_name, target_folder, source)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        rusqlite::params![ts, decision, action, ext, current_name, suggested_name, target_folder, source],
    )?;
    Ok(())
}
