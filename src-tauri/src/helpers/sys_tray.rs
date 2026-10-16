use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{
    Manager, Wry, image::Image, menu::{Menu, MenuBuilder, MenuItem, SubmenuBuilder}, tray::{TrayIcon, TrayIconBuilder}
};
use tauri_plugin_dialog::DialogExt;
use crate::helpers::{i18n::t, real_time::REALTIME_ENABLED, window::{open_route, show_main_window}};

pub static SHOULD_QUIT: AtomicBool = AtomicBool::new(false);

pub fn build_tray_menu(app: &tauri::AppHandle) -> Result<Menu<Wry>, tauri::Error> {
    let quit = MenuItem::with_id(app, "quit", t("tray.quit"), true, None::<&str>)?;
    let about = MenuItem::with_id(app, "about", t("tray.about"), true, None::<&str>)?;
    let open_ui = MenuItem::with_id(app, "open-ui", t("tray.open"), true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", t("tray.settings"), true, None::<&str>)?;
    let update = MenuItem::with_id(app, "update", t("tray.update"), true, None::<&str>)?;

    let main_scan = MenuItem::with_id(app, "main-scan", t("tray.scan.main"), true, None::<&str>)?;
    let full_scan = MenuItem::with_id(app, "full-scan", t("tray.scan.full"), true, None::<&str>)?;
    let custom_scan = MenuItem::with_id(app, "custom-scan", t("tray.scan.custom"), true, None::<&str>)?;
    let file_scan = MenuItem::with_id(app, "file-scan", t("tray.scan.file"), true, None::<&str>)?;

    let scan = SubmenuBuilder::new(app, t("tray.scan.title"))
        .items(&[&main_scan, &full_scan, &custom_scan, &file_scan])
        .build()?;

    MenuBuilder::new(app)
        .items(&[&open_ui, &about])
        .separator()
        .items(&[&settings, &scan, &update])
        .separator()
        .item(&quit)
        .build()
}

/// Percent-encodes a query value: `&`, `#`, `%` or `?` in a path would otherwise cut it short.
fn encode_query(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{:02X}", b),
        })
        .collect()
}

pub fn generate_system_tray(app: &tauri::AppHandle) -> Result<TrayIcon, tauri::Error> {
    let menu = build_tray_menu(app).map_err(|e|tauri::Error::from(e))?;

    let icon_path = if REALTIME_ENABLED.load(Ordering::Relaxed){
        "icons/blue.png"
    } else {
        "icons/red.png"
    };
    let image_path = app.path().resolve(icon_path, tauri::path::BaseDirectory::Resource)?;
    let icon = Image::from_path(image_path).unwrap_or(app.default_window_icon().unwrap().clone());

    TrayIconBuilder::with_id("clamav_gui_tray")
        .tooltip(t("tray.tooltip"))
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app_handle, event| match event.id.as_ref() {
            "quit" => {
                SHOULD_QUIT.store(true, Ordering::Relaxed);
                app_handle.exit(0);
            }
            "about" => {
                open_route(app_handle, "/about");
            }
            "main-scan" => {
                open_route(app_handle, "/scan/main");
            }
            "full-scan" => {
                open_route(app_handle, "/scan/full");
            }
            // Non-blocking pickers: the menu handler runs on the UI thread.
            "custom-scan" => {
                let app = app_handle.clone();
                app_handle.dialog().file().pick_folders(move |folders| {
                    if let Some(folders) = folders {
                        let paths: Vec<String> = folders
                            .iter()
                            .map(|val| format!("path={}", encode_query(&val.to_string())))
                            .collect();
                        open_route(&app, &format!("/scan/custom?{}", paths.join("&")));
                    }
                });
            }
            "file-scan" => {
                let app = app_handle.clone();
                app_handle.dialog().file().pick_file(move |path| {
                    if let Some(path) = path {
                        open_route(&app, &format!("/scan/file?path={}", encode_query(&path.to_string())));
                    }
                });
            }
            "update" => {
                open_route(app_handle, "/settings?tab=update");
            }
            "open-ui" => {
                show_main_window(app_handle);
            }
            "settings" => {
                open_route(app_handle, "/settings");
            }
            _ => {
                println!("menu item {:?} not handled", event.id);
            }
        })
        .build(app)
}

#[cfg(test)]
mod tests {
    #[test]
    fn encodes_paths_for_query_strings() {
        assert_eq!(super::encode_query(r"C:\R&D #1\a b"), "C%3A%5CR%26D%20%231%5Ca%20b");
        assert_eq!(super::encode_query("ñ"), "%C3%B1");
    }
}
