use std::time::Duration;

use tauri::{Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_store::StoreExt;

use crate::antivirus::update::update_definitions;

pub const MINIMIZED_FLAG: &str = "--minimized";
const FIRST_UPDATE_DELAY: Duration = Duration::from_secs(60);
// shortcut: fixed interval, make it a setting if users ask for it
const UPDATE_INTERVAL: Duration = Duration::from_secs(4 * 60 * 60);

/// Missing keys count as enabled, so both features are on after a fresh install.
fn is_enabled(app: &tauri::AppHandle, key: &str) -> bool {
    app.store("settings.json")
        .ok()
        .and_then(|store| store.get(key))
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

/// Makes the OS login entry match the "autostart" setting. Re-run on every launch
/// so the entry follows the executable path after the app is updated or moved.
pub fn sync_autostart(app: &tauri::AppHandle) -> Result<(), String> {
    let launcher = app.autolaunch();
    let wanted = is_enabled(app, "autostart");
    let current = launcher.is_enabled().map_err(|e| e.to_string())?;
    // Re-enable even when already on, so the entry is rewritten with the current exe path.
    let result = match (wanted, current) {
        (true, _) => launcher.enable(),
        (false, true) => launcher.disable(),
        (false, false) => Ok(()),
    };
    result.map_err(|e| e.to_string())
}

/// Runs freshclam shortly after launch and then every UPDATE_INTERVAL,
/// as long as "autoUpdateDefinitions" is on.
pub fn start_auto_update(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        // Give the network time to come up after a cold boot, unless scans can't work yet.
        if crate::helpers::path::has_signatures() {
            std::thread::sleep(FIRST_UPDATE_DELAY);
        }
        loop {
            // Without signatures scans can't work at all, so the first download ignores the setting.
            if is_enabled(&app, "autoUpdateDefinitions") || !crate::helpers::path::has_signatures() {
                if let Err(e) = update_definitions(app.clone()) {
                    eprintln!("Automatic definitions update failed: {}", e);
                }
            }
            std::thread::sleep(UPDATE_INTERVAL);
        }
    });
}

/// Called when the user launches the app while it is already running in the tray.
pub fn on_second_instance(app: &tauri::AppHandle, args: Vec<String>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    let route = args.iter().find_map(|arg| match arg.as_str() {
        "--scan=main" => Some("/scan/main"),
        "--scan=full" => Some("/scan/full"),
        _ => None,
    });
    if let Some(route) = route {
        app.emit("systray:move", route).ok();
    }
}
