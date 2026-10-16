//! The main window only exists while it is on screen. Sending it to the tray destroys it, which frees
//! the ~430 MB WebView2 uses; protection, updates and the tray all live in the Rust side.

use std::sync::{atomic::Ordering, Mutex};

use specta::specta;
use tauri::{command, AppHandle, Emitter, Manager, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use crate::{
    helpers::{scan::is_scan_busy, settings::get_settings_as_string, sys_tray::SHOULD_QUIT},
    types::enums::SettingKeyString,
};

/// Route requested while the window didn't exist yet; the page asks for it once it has loaded.
static PENDING_ROUTE: Mutex<Option<String>> = Mutex::new(None);

/// The configured 1300x900 doesn't fit common laptops (e.g. 1920x1080 at 150% leaves ~1280x690),
/// so shrink the window to the screen's usable area (without the taskbar) and center it.
fn fit_to_screen(window: &WebviewWindow) {
    let Ok(Some(monitor)) = window.current_monitor() else { return };
    let scale = monitor.scale_factor();
    let area = monitor.work_area().size.to_logical::<f64>(scale);
    let Ok(size) = window.outer_size() else { return };
    let size = size.to_logical::<f64>(scale);
    let width = size.width.min(area.width * 0.95);
    let height = size.height.min(area.height * 0.95);
    if width < size.width || height < size.height {
        let _ = window.set_size(tauri::LogicalSize::new(width, height));
    }
    let _ = window.center();
}

fn on_close(app: &AppHandle, window: &WebviewWindow) {
    let app = app.clone();
    let win = window.clone();
    window.on_window_event(move |event| {
        let WindowEvent::CloseRequested { api, .. } = event else { return };
        if SHOULD_QUIT.load(Ordering::Relaxed) {
            return;
        }
        let behavior = get_settings_as_string(&app, SettingKeyString::CloseButtonBehavior)
            .ok()
            .flatten()
            .unwrap_or_else(|| "minimize-to-tray".to_string());
        if behavior != "minimize-to-tray" {
            // "Exit": closing the last window ends the app (run() only keeps it alive for tray closes).
            SHOULD_QUIT.store(true, Ordering::Relaxed);
        } else if is_scan_busy() {
            // The page is following a scan: keep it, just out of sight.
            api.prevent_close();
            let _ = win.hide();
        }
        // Otherwise the window closes and is rebuilt the next time it is opened.
    });
}

/// Shows the main window, creating it if it was destroyed or never built (login launch).
pub fn show_main_window(app: &AppHandle) -> Option<WebviewWindow> {
    let window = match app.get_webview_window("main") {
        Some(window) => window,
        None => {
            let config = app.config().app.windows.iter().find(|w| w.label == "main")?.clone();
            let window = WebviewWindowBuilder::from_config(app, &config).ok()?.build().ok()?;
            fit_to_screen(&window);
            on_close(app, &window);
            window
        }
    };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
    Some(window)
}

/// Opens the window on a route (tray menu, second launch with --scan=...).
pub fn open_route(app: &AppHandle, route: &str) {
    if app.get_webview_window("main").is_some() {
        show_main_window(app);
        let _ = app.emit("systray:move", route);
    } else {
        // A new page can't receive the event yet, so it picks the route up when it loads.
        *PENDING_ROUTE.lock().unwrap_or_else(|e| e.into_inner()) = Some(route.to_string());
        show_main_window(app);
    }
}

#[command]
#[specta]
pub fn take_pending_route() -> Option<String> {
    PENDING_ROUTE.lock().unwrap_or_else(|e| e.into_inner()).take()
}
