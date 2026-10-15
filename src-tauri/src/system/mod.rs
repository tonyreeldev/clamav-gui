pub mod logs;
pub mod scheduler;
pub mod sysinfo;

use std::sync::atomic::Ordering;

use specta::specta;
use tauri::command;
use tauri::Manager;

use crate::helpers::i18n::t;
use crate::helpers::real_time::REALTIME_ENABLED;
use crate::helpers::sys_tray::build_tray_menu;
use crate::{
    helpers::{
        history::append_history, i18n::{TRANSLATIONS, load_translations}, log::{initialize_log_with_id, log_err, log_info}, new_id, path::get_clamav_path
    },
    types::{
        enums::{HistoryDetails, HistoryStatus, HistoryType, LogCategory},
        structs::{AppLanguage, HistoryItem},
    },
};

#[command]
#[specta(result)]
pub fn remove_file(
    app: tauri::AppHandle,
    file_path: String,
    log_id: Option<String>,
) -> Result<(), String> {
    if file_path.trim().is_empty() {
        return Err("File path cannot be empty".into());
    }

    let log_id = log_id.unwrap_or_else(new_id);
    let init = initialize_log_with_id(&app, LogCategory::Quarantine, &log_id)?;
    let log_file = init.file.clone();

    match std::fs::remove_file(&file_path) {
        Ok(_) => {
            let message = format!("The file was deleted: {}", file_path);
            log_info(&log_file, &message);

            if let Err(e) = append_history(
                &app,
                HistoryItem {
                    id: new_id(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    action: Some(HistoryType::FileDelete),
                    details: Some(HistoryDetails::FileDelete { file_path }),
                    status: HistoryStatus::Success,
                    log_id: Some(log_id),
                    category: Some(LogCategory::Quarantine),
                    scan_type: None,
                    threat_count: None,
                    scan_result: None,
                },
            ) {
                log_err(&log_file, &format!("Failed to append history: {}", e));
            }
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to delete file: {} ({})", file_path, e);
            log_err(&log_file, &format!("Failed to delete file: {}", file_path));
            log_err(&log_file, &e.to_string());

            if let Err(e) = append_history(
                &app,
                HistoryItem {
                    id: new_id(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    action: Some(HistoryType::FileDeleteError),
                    details: Some(HistoryDetails::FileDeleteError { err: error_msg.clone(), file_path }),
                    status: HistoryStatus::Error,
                    log_id: Some(log_id),
                    category: Some(LogCategory::Quarantine),
                    scan_type: None,
                    threat_count: None,
                    scan_result: None,
                },
            ) {
                log_err(&log_file, &format!("Failed to append history: {}", e));
            }
            Err(e.to_string())
        }
    }
}

#[command]
#[specta]
pub fn check_availability() -> bool {
    get_clamav_path().is_ok()
}

#[command]
#[specta]
pub fn set_language(
    app: tauri::AppHandle,
    state: tauri::State<AppLanguage>,
    lang: String,
) -> Result<(), String> {
    {
        let mut guard = state.0.lock().unwrap();
        *guard = lang.clone();
    }
    let map = load_translations(&app,&lang);
    let mut translations = TRANSLATIONS
        .write()
        .map_err(|_| "Translation lock poisoned")?;
    *translations = map;
    Ok(())
}

#[command]
#[specta(result)]
pub fn rebuild_tray(app: tauri::AppHandle) -> Result<(), String> {
    let tray = app
        .tray_by_id("clamav_gui_tray")
        .ok_or("Tray not found")?;
    let menu = build_tray_menu(&app).map_err(|e| e.to_string())?;
    tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
    tray.set_tooltip(Some(t("tray.tooltip")))
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[command]
#[specta(result)]
pub fn update_tray_icon(app: tauri::AppHandle) -> Result<(), String> {
    let tray = app
        .tray_by_id("clamav_gui_tray")
        .ok_or("Tray not found")?;

    let icon_path = if REALTIME_ENABLED.load(Ordering::Relaxed) {
        "icons/blue.png"
    } else {
        "icons/red.png"
    };

    let image_path = app
        .path()
        .resolve(icon_path, tauri::path::BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;

    let icon = tauri::image::Image::from_path(image_path)
        .map_err(|e| e.to_string())?;

    tray.set_icon(Some(icon)).map_err(|e| e.to_string())?;
    Ok(())
}

#[command]
#[specta(result)]
pub fn sync_autostart(app: tauri::AppHandle) -> Result<(), String> {
    crate::helpers::startup::sync_autostart(&app)
}

/// Installs ClamAV through winget (official Cisco.ClamAV MSI, hash-verified by winget),
/// then downloads the first signatures. Windows shows one UAC prompt for the MSI.
#[command]
#[specta(result)]
pub async fn install_clamav(app: tauri::AppHandle) -> Result<(), String> {
    if !cfg!(windows) {
        return Err("Automatic installation is only available on Windows".into());
    }
    let output = tauri::async_runtime::spawn_blocking(|| {
        crate::helpers::silent_command("winget")
            .args([
                "install", "--id", "Cisco.ClamAV", "--exact", "--silent",
                "--accept-package-agreements", "--accept-source-agreements",
                "--disable-interactivity",
            ])
            .output()
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("winget is not available: {}", e))?;

    // winget's exit codes vary (e.g. "already installed"), so trust what is actually on disk.
    if get_clamav_path().is_err() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = stdout.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
        return Err(format!("ClamAV installation failed (exit code {}): {}", output.status.code().unwrap_or(-1), detail));
    }
    crate::antivirus::update::update_definitions(app)
}

#[command]
#[specta]
pub fn notify(app: tauri::AppHandle, title: String, body: String, kind: crate::helpers::notifier::NotifyKind) {
    crate::helpers::notifier::notify(&app, &title, &body, kind);
}
