// Safe file operations: move (with collision-safe naming) or Trash (recoverable),
// every action logged so it can be undone.

use crate::db;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

pub fn apply(
    conn: &Connection,
    from: &str,
    action: &str,
    target_folder: &str,
    suggested_name: &str,
) -> Result<String, String> {
    let from_path = PathBuf::from(from);
    if !from_path.exists() {
        return Err("Source file no longer exists.".into());
    }
    let id = format!("m{}", chrono::Local::now().timestamp_micros());
    let ts = chrono::Local::now().to_rfc3339();

    if action == "trash" {
        to_trash(&from_path)?;
        db::log_move(conn, &id, from, "<Trash>", "trash", &ts).map_err(|e| e.to_string())?;
        return Ok(id);
    }

    let dir = PathBuf::from(target_folder);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Create folder failed: {e}"))?;
    let dest = unique_dest(&dir, suggested_name);
    move_file(&from_path, &dest)?;
    db::log_move(conn, &id, from, &dest.display().to_string(), "move", &ts)
        .map_err(|e| e.to_string())?;
    Ok(id)
}

// Move to ~/.Trash directly — instant, no Finder automation (so no "control
// Finder" permission prompt and no Finder-driven freeze on big batches).
// Still fully recoverable from the Trash.
fn to_trash(from: &Path) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("no home dir")?;
    let trash_dir = home.join(".Trash");
    std::fs::create_dir_all(&trash_dir).map_err(|e| format!("Trash dir: {e}"))?;
    let name = from.file_name().and_then(|s| s.to_str()).ok_or("bad filename")?;
    let dest = unique_dest(&trash_dir, name);
    move_file(from, &dest)
}

fn unique_dest(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    let ext = Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    let mut i = 1;
    loop {
        let cand = dir.join(format!("{stem}-{i}{ext}"));
        if !cand.exists() {
            return cand;
        }
        i += 1;
    }
}

fn move_file(from: &Path, to: &Path) -> Result<(), String> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    // Cross-volume fallback.
    std::fs::copy(from, to).map_err(|e| format!("Copy failed: {e}"))?;
    std::fs::remove_file(from).map_err(|e| format!("Remove after copy failed: {e}"))?;
    Ok(())
}

pub fn undo(conn: &Connection, id: &str) -> Result<(), String> {
    let (from, to, action) = db::get_move(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or("Nothing to undo for that id.")?;
    if action == "trash" {
        return Err("It's in the Trash — restore it from Finder (right-click → Put Back).".into());
    }
    let from_path = PathBuf::from(&from);
    if let Some(parent) = from_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    move_file(Path::new(&to), &from_path)?;
    db::mark_undone(conn, id).map_err(|e| e.to_string())?;
    Ok(())
}
