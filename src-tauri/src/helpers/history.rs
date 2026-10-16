use std::{
    fs::File,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::Manager;

use crate::{helpers::log::log_err, types::structs::HistoryItem};

pub fn history_dir(app: &tauri::AppHandle) -> PathBuf {
    let mut dir = app.path().app_data_dir().unwrap();
    dir.push("history");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("[ERROR]: {}", e.to_string())
    }
    dir
}

/// Scans, real-time protection, freshclam and the UI all write history from different threads;
/// every read-modify-write of a day file holds this so entries aren't lost.
static HISTORY_LOCK: Mutex<()> = Mutex::new(());

pub fn lock_history() -> std::sync::MutexGuard<'static, ()> {
    HISTORY_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Write to a temp file and rename it over the target, so a crash mid-write can't truncate a day file.
pub fn write_history_file(path: &std::path::Path, items: &[HistoryItem]) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(items).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

pub fn append_history(app: &tauri::AppHandle, item: HistoryItem) -> Result<(), String> {
    let dir = history_dir(app);
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let file = dir.join(format!("{}.json", date));
    let _guard = lock_history();

    let mut items: Vec<HistoryItem> = match std::fs::read_to_string(&file) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_else(|_| {
            // Keep the unreadable file aside instead of overwriting the whole day with one entry.
            let _ = std::fs::rename(&file, file.with_extension("json.corrupt"));
            vec![]
        }),
        Err(_) => vec![],
    };

    items.push(item);
    write_history_file(&file, &items)
}

pub fn append_update_history(app: &tauri::AppHandle, item: HistoryItem, log: &Arc<Mutex<File>>) {
    if let Err(e) = append_history(app, item) {
        log_err(log, &format!("Failed to append update history: {}", e));
    }
}

pub fn append_scan_history(app: &tauri::AppHandle, item: HistoryItem, log: &Arc<Mutex<File>>) {
    if let Err(e) = append_history(app, item) {
        log_err(log, &format!("Scan history append failed: {}", e));
    }
}

pub fn append_scheduler_history(
    app: &tauri::AppHandle,
    item: HistoryItem,
    log_file: &std::sync::Arc<std::sync::Mutex<std::fs::File>>,
) {
    if let Err(e) = append_history(app, item) {
        log_err(log_file, &format!("Failed to append history: {}", e));
    }
}

pub fn append_realtime_history(
    app: &tauri::AppHandle,
    item: HistoryItem,
    log_file: &std::sync::Arc<std::sync::Mutex<std::fs::File>>,
) {
    if let Err(e) = append_history(app, item) {
        log_err(
            log_file,
            &format!("Failed to append real-time scan history: {}", e),
        );
    }
}

pub fn clear_by_status(
    path: &std::path::PathBuf,
    retain_predicate: impl Fn(&HistoryItem) -> bool,
) -> Result<(), String> {
    let _guard = lock_history();
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut items: Vec<HistoryItem> = serde_json::from_str(&content).map_err(|e| e.to_string())?;

    items.retain(retain_predicate);

    if items.is_empty() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    } else {
        write_history_file(path, &items)?;
    }

    Ok(())
}

pub fn parse_date_from_path(path: &std::path::PathBuf) -> Option<chrono::NaiveDate> {
    let stem = path.file_stem()?.to_str()?;
    chrono::NaiveDate::parse_from_str(stem, "%Y-%m-%d").ok()
}

pub fn clear_by_days(path: &PathBuf, days: i64) -> Result<(), String>{
    if let Some(file_date) = parse_date_from_path(path) {
        // "Clear last N days" removes the recent files, not the old ones.
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).date_naive();
        if file_date >= cutoff {
            std::fs::remove_file(path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}