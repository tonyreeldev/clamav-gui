pub mod bulk_actions;
pub mod history;
pub mod quarantine;
pub mod scan;
pub mod stats;
pub mod update;

use serde::Serialize;
use specta::{specta, Type};
use std::{path::PathBuf, sync::atomic::Ordering};
use tauri::{command, Manager};

use crate::{
    helpers::{
        settings::get_settings_as_array, history::append_realtime_history, log::{initialize_log, log_err, log_info}, new_id, real_time::{current_mode, start_realtime_scan, stop_realtime_scan, LOW_MEMORY, REALTIME_ENABLED},
        scan::{get_main_paths, outermost_paths},
    },
    types::{
        enums::{BehaviorMode, HistoryDetails, HistoryStatus, HistoryType, LogCategory, ScanType, SettingKeyArray},
        structs::HistoryItem,
    },
};

#[command]
#[specta(result)]
pub fn start_real_time_scan(
    app: tauri::AppHandle,
    behavior: BehaviorMode,
) -> Result<(), String> {
    LOW_MEMORY.store(low_memory_enabled(&app), Ordering::SeqCst);
    let mut configured = get_settings_as_array(&app,SettingKeyArray::MonitoringPaths)?;
    // On first launch the UI fills this setting asynchronously, often after this call:
    // protection is on by default, so it must not depend on that race.
    if configured.is_empty() {
        configured = get_main_paths(app.path()).iter().map(|p| p.to_string_lossy().to_string()).collect();
    }
    let log = initialize_log(&app, LogCategory::Realtime)?;
    let log_id = log.id.clone();
    let log_file = log.file.clone();
    // One missing folder (e.g. a deleted "Music") must not leave the whole PC unprotected.
    let (existing, missing): (Vec<PathBuf>, Vec<PathBuf>) =
        configured.iter().map(PathBuf::from).partition(|p| p.exists());
    for path in &missing {
        log_err(&log_file, &format!("Skipping missing folder: {}", path.display()));
    }
    let paths: Vec<String> = outermost_paths(existing)
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    if paths.is_empty() {
        log_err(&log_file, "No existing folder to monitor");
        return Err("No existing folder to monitor".into());
    }
    if let Err(e) = start_realtime_scan(app.clone(), paths.clone(), behavior, log_id.clone()) {
        log_err(&log_file, &format!("Failed to start real-time scan: {}", e));
        append_realtime_history(
            &app,
            HistoryItem {
                id: new_id(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                action: Some(HistoryType::RealTimeError),
                details: Some(HistoryDetails::RealTimeError { err: e.clone() }),
                status: HistoryStatus::Error,
                category: Some(LogCategory::Realtime),
                log_id: Some(log_id),
                scan_type: Some(ScanType::Realtime),
                threat_count: None,
                scan_result: None,
            },
            &log_file,
        );
        return Err(e);
    }
    log_info(
        &log_file,
        &format!("Real-time scan started, monitoring {} path(s)", paths.len()),
    );
    append_realtime_history(
        &app,
        HistoryItem {
            id: new_id(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            action: Some(HistoryType::RealTimeStart),
            details: Some(HistoryDetails::RealTimeStart {
                behavior,
                paths: paths.len()
            }),
            status: HistoryStatus::Success,
            category: Some(LogCategory::Realtime),
            log_id: Some(log_id),
            scan_type: Some(ScanType::Realtime),
            threat_count: None,
            scan_result: None,
        },
        &log_file,
    );

    Ok(())
}

#[command]
#[specta(result)]
pub fn stop_real_time_scan(app: tauri::AppHandle) -> Result<(), String> {
    let log = initialize_log(&app, LogCategory::Realtime)?;
    let log_id = log.id.clone();
    let log_file = log.file.clone();

    stop_realtime_scan();

    log_info(&log_file, "Real-time scan stopped");

    append_realtime_history(
        &app,
        HistoryItem {
            id: new_id(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            action: Some(HistoryType::RealTimeStop),
            details: Some(HistoryDetails::RealTimeStop),
            status: HistoryStatus::Warning,
            category: Some(LogCategory::Realtime),
            log_id: Some(log_id),
            scan_type: Some(ScanType::Realtime),
            threat_count: None,
            scan_result: None,
        },
        &log_file,
    );

    Ok(())
}

fn store_value(app: &tauri::AppHandle, key: &str) -> Option<serde_json::Value> {
    use tauri_plugin_store::StoreExt;
    app.store("settings.json").ok()?.get(key)
}

/// Protection starts with the app, not with the window (which a login launch never builds).
/// The UI mirrors its "realTime"/"behavior" preferences into the store for this. On by default.
pub fn start_from_settings(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        if !store_value(&app, "realTime").and_then(|v| v.as_bool()).unwrap_or(true) {
            return;
        }
        let behavior = store_value(&app, "behavior")
            .and_then(|v| serde_json::from_value::<BehaviorMode>(v).ok())
            .unwrap_or(BehaviorMode::Balanced);
        match start_real_time_scan(app.clone(), behavior) {
            Ok(()) => {
                let _ = crate::system::update_tray_icon(app.clone());
                return;
            }
            // E.g. during login before the profile folders are reachable: protection must not stay off.
            Err(e) => eprintln!("Real-time start failed, retrying in 30 s: {}", e),
        }
        std::thread::sleep(std::time::Duration::from_secs(30));
    });
}

#[derive(Serialize, Type)]
pub struct RealTimeStatus {
    running: bool,
    behavior: Option<BehaviorMode>,
}

/// Lets a freshly built window adopt the running session instead of restarting it.
#[command]
#[specta]
pub fn get_real_time_status() -> RealTimeStatus {
    let running = REALTIME_ENABLED.load(Ordering::SeqCst);
    RealTimeStatus { running, behavior: running.then(current_mode).flatten() }
}

/// Machines with this much RAM or less unload clamd when idle unless the user says otherwise.
const LOW_MEMORY_AUTO_GB: f64 = 8.5;

fn total_ram_gb() -> f64 {
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    system.total_memory() as f64 / 1024f64.powi(3)
}

fn low_memory_enabled(app: &tauri::AppHandle) -> bool {
    store_value(app, "lowMemoryMode")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| total_ram_gb() <= LOW_MEMORY_AUTO_GB)
}

#[derive(Serialize, Type)]
pub struct LowMemoryInfo {
    enabled: bool,
    /// True while the user never chose: the value then follows the installed RAM.
    automatic: bool,
    total_ram_gb: f64,
}

#[command]
#[specta]
pub fn get_low_memory_mode(app: tauri::AppHandle) -> LowMemoryInfo {
    LowMemoryInfo {
        enabled: low_memory_enabled(&app),
        automatic: store_value(&app, "lowMemoryMode").and_then(|v| v.as_bool()).is_none(),
        total_ram_gb: (total_ram_gb() * 10.0).round() / 10.0,
    }
}

#[command]
#[specta(result)]
pub fn set_low_memory_mode(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    store.set("lowMemoryMode", enabled);
    store.save().map_err(|e| e.to_string())?;
    LOW_MEMORY.store(enabled, Ordering::SeqCst);
    // Turning it off brings back instant scans: load the signatures now rather than on the next file.
    if !enabled && REALTIME_ENABLED.load(Ordering::SeqCst) {
        std::thread::spawn(|| {
            let _ = crate::helpers::clamd::ensure_running();
        });
    }
    Ok(())
}
