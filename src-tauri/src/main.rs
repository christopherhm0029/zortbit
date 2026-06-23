// Zortbit — Sprint 1
// Propose-first organizer: watches ~/Downloads (Desktop & co. excluded), and
// scans the confirmed bulk scope on demand. Every file becomes a *proposal*
// the user approves; nothing moves on its own. Moves are safe + undoable.

mod config;
mod db;
mod engine;
mod mover;
mod ocr;

use config::Config;
use engine::Proposal;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, State};

struct AppState {
    cfg: Config,
    conn: Mutex<rusqlite::Connection>,
    counter: AtomicU64,
    ocr_bin: Option<std::path::PathBuf>,
}

fn home() -> std::path::PathBuf {
    dirs::home_dir().expect("home directory")
}

fn now() -> String {
    chrono::Local::now().to_rfc3339()
}

fn ext_of(name: &str) -> String {
    std::path::Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase()
}

// Park the popover just under the menu bar, top-right.
fn position_top_right(win: &tauri::WebviewWindow) {
    if let Ok(Some(m)) = win.current_monitor() {
        let mon_w_logical = m.size().width as f64 / m.scale_factor();
        let x = (mon_w_logical - 380.0 - 14.0).max(0.0);
        let _ = win.set_position(tauri::LogicalPosition::new(x, 34.0));
    }
}

#[tauri::command]
fn get_config(state: State<AppState>) -> Config {
    state.cfg.clone()
}

#[tauri::command]
fn scan_bulk(app: tauri::AppHandle, state: State<AppState>) {
    let cfg = state.cfg.clone();
    let ocr_bin = state.ocr_bin.clone();
    // Compute the learning hint ONCE per scan (never per-file).
    let hints = match state.conn.lock() {
        Ok(c) => engine::hint_string(&c, &cfg),
        Err(_) => String::new(),
    };
    let base = state.counter.fetch_add(100_000, Ordering::SeqCst);
    std::thread::spawn(move || {
        let home = home();
        let mut i = base;
        for rel in &cfg.bulk_scope {
            let dir = home.join(rel);
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for e in entries.flatten() {
                    let p = e.path();
                    if p.is_file() {
                        if let Some(prop) =
                            engine::propose(&p, &cfg, &home, i, ocr_bin.as_deref(), &hints)
                        {
                            let _ = app.emit("proposal", prop);
                        }
                        i += 1;
                    }
                }
            }
        }
        let _ = app.emit("scan-done", ());
    });
}

#[tauri::command]
fn approve(state: State<AppState>, proposal: Proposal) -> Result<String, String> {
    let conn = state.conn.lock().map_err(|_| "internal lock error")?;
    let id = mover::apply(
        &conn,
        &proposal.path,
        &proposal.action,
        &proposal.target_folder,
        &proposal.suggested_name,
    )?;
    let _ = db::log_decision(
        &conn,
        &now(),
        "approve",
        &proposal.action,
        &ext_of(&proposal.current_name),
        &proposal.current_name,
        &proposal.suggested_name,
        &proposal.target_folder,
        &proposal.source,
    );
    Ok(id)
}

// Logged so Zortbit can learn what you choose NOT to do, too.
#[tauri::command]
fn skip(state: State<AppState>, proposal: Proposal) {
    if let Ok(conn) = state.conn.lock() {
        let _ = db::log_decision(
            &conn,
            &now(),
            "skip",
            &proposal.action,
            &ext_of(&proposal.current_name),
            &proposal.current_name,
            &proposal.suggested_name,
            &proposal.target_folder,
            &proposal.source,
        );
    }
}

#[derive(Clone, Serialize)]
struct ApplyResult {
    id: String,
    current_name: String,
    suggested_name: String,
    action: String,
    ok: bool,
    move_id: String,
    error: String,
}

// Batch approve — runs on a worker thread (its own DB connection) and streams
// "apply-result" events, so the UI stays responsive even for 100s of files
// (and slow Finder-backed trashes don't freeze the app).
#[tauri::command]
fn approve_many(app: tauri::AppHandle, proposals: Vec<Proposal>) {
    std::thread::spawn(move || {
        let conn = match db::open() {
            Ok(c) => c,
            Err(_) => {
                let _ = app.emit("apply-done", ());
                return;
            }
        };
        let mut done = 0u32;
        for p in proposals {
            let r = match mover::apply(&conn, &p.path, &p.action, &p.target_folder, &p.suggested_name) {
                Ok(move_id) => {
                    let _ = db::log_decision(
                        &conn, &now(), "approve", &p.action, &ext_of(&p.current_name),
                        &p.current_name, &p.suggested_name, &p.target_folder, &p.source,
                    );
                    ApplyResult {
                        id: p.id, current_name: p.current_name, suggested_name: p.suggested_name,
                        action: p.action, ok: true, move_id, error: String::new(),
                    }
                }
                Err(e) => ApplyResult {
                    id: p.id, current_name: p.current_name, suggested_name: p.suggested_name,
                    action: p.action, ok: false, move_id: String::new(), error: e,
                },
            };
            let _ = app.emit("apply-result", r);
            // Gentle batches: pause every 15 so we never flood the UI or hammer the disk.
            done += 1;
            if done % 15 == 0 {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
        let _ = app.emit("apply-done", ());
    });
}

#[tauri::command]
fn undo_move(state: State<AppState>, id: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|_| "internal lock error")?;
    mover::undo(&conn, &id)
}

fn main() {
    let cfg = Config::load_or_init();
    let conn = db::open().expect("open zortbit.db");

    // OCR sidecar (src-tauri/bin/zb_ocr). Dev resolution via the crate manifest
    // dir; distribution will switch to a bundled sidecar (deferred to release).
    let ocr_bin = {
        let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/zb_ocr");
        if p.exists() { Some(p) } else { None }
    };
    let ocr_for_watch = ocr_bin.clone();

    tauri::Builder::default()
        .manage(AppState {
            cfg: cfg.clone(),
            conn: Mutex::new(conn),
            counter: AtomicU64::new(1),
            ocr_bin,
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            scan_bulk,
            approve,
            approve_many,
            skip,
            undo_move
        ])
        .on_window_event(|window, event| {
            // Click-away: hide the popover when it loses focus (menu-bar behaviour).
            if let tauri::WindowEvent::Focused(false) = event {
                let _ = window.hide();
            }
        })
        .setup(move |app| {
            // Menu-bar app: no Dock icon, starts hidden — lives in the tray.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.hide();
            }

            let quit = tauri::menu::MenuItem::with_id(app, "quit", "Quit Zortbit", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(app, &[&quit])?;

            let tray_icon = tauri::image::Image::new(include_bytes!("../icons/tray.rgba"), 44, 44);
            let _tray = TrayIconBuilder::new()
                .icon(tray_icon)
                .icon_as_template(true)
                .tooltip("Zortbit")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    if event.id.as_ref() == "quit" {
                        app.exit(0);
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(win) = app.get_webview_window("main") {
                            position_top_right(&win);
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                })
                .build(app)?;

            let handle = app.handle().clone();
            let cfg2 = cfg.clone();
            let ocr2 = ocr_for_watch.clone();
            std::thread::spawn(move || watch(handle, cfg2, ocr2));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Zortbit");
}

fn watch(handle: tauri::AppHandle, cfg: Config, ocr_bin: Option<std::path::PathBuf>) {
    use notify_debouncer_full::notify::{EventKind, RecursiveMode, Watcher};
    use notify_debouncer_full::{new_debouncer, DebounceEventResult};
    use std::time::Duration;

    let home = home();
    let downloads = home.join("Downloads");
    let counter = AtomicU64::new(1_000_000);
    let hint_conn = db::open().ok(); // separate read connection for learning hints

    let mut debouncer = new_debouncer(
        Duration::from_secs(2),
        None,
        move |res: DebounceEventResult| {
            if let Ok(events) = res {
                let hints = hint_conn
                    .as_ref()
                    .map(|c| engine::hint_string(c, &cfg))
                    .unwrap_or_default();
                for ev in events {
                    if !matches!(ev.kind, EventKind::Create(_)) {
                        continue;
                    }
                    for path in &ev.paths {
                        if !path.is_file() {
                            continue;
                        }
                        let c = counter.fetch_add(1, Ordering::SeqCst);
                        if let Some(prop) =
                            engine::propose(path, &cfg, &home, c, ocr_bin.as_deref(), &hints)
                        {
                            println!("[zortbit] proposal: {}", prop.current_name);
                            let _ = handle.emit("proposal", prop);
                        }
                    }
                }
            }
        },
    )
    .expect("failed to build file watcher");

    if downloads.exists() {
        let _ = debouncer.watcher().watch(&downloads, RecursiveMode::NonRecursive);
        println!("[zortbit] watching {}", downloads.display());
    }
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}
