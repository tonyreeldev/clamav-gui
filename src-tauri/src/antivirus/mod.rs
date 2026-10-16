pub mod bulk_actions;
pub mod history;
pub mod quarantine;
pub mod scan;
pub mod stats;
pub mod update;

use specta::specta;
use std::path::PathBuf;
use tauri::{command, Manager};

use crate::{
    helpers::{
        settings::get_settings_as_array, history::append_realtime_history, log::{initialize_log, log_err, log_info}, new_id, real_time::{start_realtime_scan, stop_realtime_scan},
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
