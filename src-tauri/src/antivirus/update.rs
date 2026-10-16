use std::sync::atomic::{AtomicBool, Ordering};

use specta::specta;
use tauri::{command, Emitter};

use crate::{
    helpers::{
        history::append_update_history,
        log::{initialize_log, log_err, log_info},
        new_id, path::freshclam_command,
    },
    types::{
        enums::{HistoryDetails, HistoryStatus, HistoryType, LogCategory},
        structs::HistoryItem,
    },
};

// Two concurrent freshclam runs fight over the database lock, so only one may run at a time.
static UPDATING: AtomicBool = AtomicBool::new(false);

#[command]
#[specta(result)]
pub fn update_definitions(app: tauri::AppHandle) -> Result<(), String> {
    if UPDATING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let release = |_: &String| UPDATING.store(false, Ordering::SeqCst);
    let init = initialize_log(&app, LogCategory::Update).inspect_err(release)?;
    let log_id = init.id.clone();
    let log_file = init.file.clone();

    append_update_history(
        &app,
        HistoryItem {
            id: new_id(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            action: Some(HistoryType::DefUpdateStart),
            details: Some(HistoryDetails::DefUpdateStart),
            status: HistoryStatus::Success,
            category: Some(LogCategory::Update),
            log_id: Some(log_id.clone()),
            scan_type: None,
            threat_count: None,
            scan_result: None,
        },
        &log_file,
    );
    let mut freshclam = freshclam_command().inspect_err(release)?;
    std::thread::spawn(move || {
        let _ = app.emit("freshclam:start", ()).map_err(|e| e.to_string());

        let output = freshclam
            .arg("--stdout")
            .output();
        UPDATING.store(false, Ordering::SeqCst);
        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let exit_code = out.status.code().unwrap_or(-1);

                if !stdout.is_empty() {
                    let stdout_str = stdout.to_string();
                    log_info(&log_file, &stdout_str);
                }
                if !stderr.is_empty() {
                    let stderr_str = stderr.to_string();
                    let _ = app.emit("freshclam:error", &stderr_str)
                        .map_err(|e| e.to_string());
                    log_err(&log_file, &stderr_str);
                }

                // freshclam: 0 = updated, 1 = already up to date, anything else = failure
                let status = match exit_code {
                    0 | 1 => HistoryStatus::Success,
                    _ => HistoryStatus::Error,
                };
                if exit_code == 0 {
                    use crate::helpers::{clamd, real_time::REALTIME_ENABLED};
                    // On a fresh install real-time starts before the first signatures exist and clamd can't load;
                    // start it as soon as they arrive instead of waiting for the next new file.
                    std::thread::spawn(|| {
                        if clamd::is_alive() {
                            clamd::reload();
                        } else if REALTIME_ENABLED.load(Ordering::SeqCst) {
                            if let Err(e) = clamd::ensure_running() {
                                eprintln!("clamd still unavailable after the update: {}", e);
                            }
                        }
                    });
                }
                append_update_history(
                    &app,
                    HistoryItem {
                        id: new_id(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        action: Some(HistoryType::DefUpdateFinish),
                        details: Some(HistoryDetails::DefUpdateFinish { exit_code }),
                        status,
                        category: Some(LogCategory::Update),
                        log_id: Some(log_id),
                        scan_type: None,
                        threat_count: None,
                        scan_result: None,
                    },
                    &log_file,
                );
                let _ = app.emit("freshclam:done", exit_code)
                    .map_err(|e| e.to_string());
            }
            Err(e) => {
                let error_msg = e.to_string();
                let _ = app.emit("freshclam:error", &error_msg)
                    .map_err(|e| e.to_string());
                log_err(&log_file, &error_msg);

                append_update_history(
                    &app,
                    HistoryItem {
                        id: new_id(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        action: Some(HistoryType::DefUpdateError),
                        details: Some(HistoryDetails::DefUpdateError { err: error_msg }),
                        status: HistoryStatus::Error,
                        category: Some(LogCategory::Update),
                        log_id: Some(log_id),
                        scan_type: None,
                        threat_count: None,
                        scan_result: None,
                    },
                    &log_file,
                );
            }
        }
    });

    Ok(())
}

#[command]
#[specta]
pub fn is_updating_definitions() -> bool {
    UPDATING.load(Ordering::SeqCst)
}

#[command]
#[specta(result)]
pub fn get_clamav_version() -> Result<String, String> {
    let output = freshclam_command()?
        .arg("--version")
        .output()
        .map_err(|e| e.to_string())?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
