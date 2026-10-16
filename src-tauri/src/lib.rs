// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![macro_use]

mod antivirus;
mod helpers;
mod system;
mod types;

use std::sync::{
    atomic::Ordering,
    Mutex
};
use tauri_specta::{collect_commands, Builder};

use crate::{
    antivirus::{
        bulk_actions::{clear_quarantine, delete_all, quarantine_all, restore_all},
        history::{clear_history, load_history, mark_as_acknowledged, clear_by_date, clear_by_range},
        quarantine::{delete_quarantine, list_quarantine, quarantine_file, restore_quarantine},
        scan::{get_startup_scan, start_custom_scan, start_full_scan, start_main_scan, stop_scan, get_scan_status},
        start_real_time_scan, get_real_time_status, get_low_memory_mode, set_low_memory_mode,
        stats::get_stats,
        stop_real_time_scan,
        update::{get_clamav_version, update_definitions, is_updating_definitions},
    },
    helpers::{
        flags::parse_startup_flags,
        scan::run_headless_scan,
        sys_tray::{generate_system_tray, SHOULD_QUIT},
        i18n::{load_translations,TRANSLATIONS},
        startup::{on_second_instance, start_auto_update, MINIMIZED_FLAG},
        window::{show_main_window, take_pending_route}
    },
    system::{
        check_availability,
        sync_autostart,
        install_clamav,
        notify,
        set_language,
        rebuild_tray,
        update_tray_icon,
        logs::{read_log, reveal_log},
        remove_file,
        scheduler::{
            clear_scheduled_jobs, list_scheduler, remove_scheduled_task, run_job_now, schedule_task,
        },
        sysinfo::{get_sys_info, get_sys_stats},
        security::{get_security_status, open_security_settings, set_security_features},
    },
    types::structs::AppLanguage
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let scan_flag = parse_startup_flags();
    if scan_flag.is_scheduled {
        if let Err(e) = run_headless_scan(scan_flag) {
            eprintln!("Failed to run a headless scan: {}", e.to_string());
        }
        std::process::exit(0);
    }

    let specta_builder = Builder::<tauri::Wry>::new().commands(collect_commands![
        check_availability,
        get_sys_stats,
        get_sys_info,
        start_main_scan,
        start_full_scan,
        start_custom_scan,
        stop_scan,
        update_definitions,
        get_clamav_version,
        quarantine_file,
        list_quarantine,
        restore_quarantine,
        delete_quarantine,
        remove_file,
        quarantine_all,
        delete_all,
        restore_all,
        clear_quarantine,
        load_history,
        mark_as_acknowledged,
        clear_history,
        read_log,
        reveal_log,
        get_stats,
        get_startup_scan,
        schedule_task,
        list_scheduler,
        remove_scheduled_task,
        run_job_now,
        clear_scheduled_jobs,
        start_real_time_scan,
        stop_real_time_scan,
        set_language,
        rebuild_tray,
        update_tray_icon,
        get_scan_status,
        clear_by_date,
        clear_by_range,
        is_updating_definitions,
        sync_autostart,
        install_clamav,
        notify,
        get_security_status,
        take_pending_route,
        get_real_time_status,
        get_low_memory_mode,
        set_low_memory_mode,
        open_security_settings,
        set_security_features
    ]);

    tauri::Builder::default()
        // Must be registered first so a second launch exits before anything else starts.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| on_second_instance(app, args)))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![MINIMIZED_FLAG]),
        ))
        .setup(|app| {
            use tauri::Manager;
            
            let app_handle = app.handle();
            let lang = crate::helpers::i18n::boot_language(app_handle);
            {
                let mut guard = TRANSLATIONS.write().unwrap();
                *guard = load_translations(&app_handle.clone(), &lang);
            }
            *app.state::<AppLanguage>().0.lock().unwrap() = lang;
            // At early login explorer may not be ready; without a tray, show the window so the app stays reachable.
            let has_tray = match generate_system_tray(app_handle) {
                Ok(_) => true,
                Err(e) => {
                    eprintln!("Failed to create the tray icon: {}", e);
                    false
                }
            };
            // A login launch stays in the tray without building the window (and its WebView2) at all.
            if !has_tray || !std::env::args().any(|arg| arg == MINIMIZED_FLAG) {
                show_main_window(app_handle);
            }
            if let Err(e) = crate::helpers::startup::sync_autostart(app_handle) {
                eprintln!("Failed to sync autostart: {}", e);
            }
            // Protection no longer waits for the window: it may never be opened in a session.
            crate::antivirus::start_from_settings(app_handle.clone());
            start_auto_update(app_handle.clone());
            Ok(())
        })
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_os::init())
        .manage(AppLanguage(Mutex::new("en".into())))
        .manage(scan_flag.clone())
        .invoke_handler(specta_builder.invoke_handler())
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_, event| match event {
            // Closing the window to the tray destroys it; the app keeps running unless the user chose to quit.
            tauri::RunEvent::ExitRequested { api, code: None, .. } if !SHOULD_QUIT.load(Ordering::Relaxed) => {
                api.prevent_exit();
            }
            // clamd is a separate process holding ~1 GB; it would outlive the app otherwise.
            tauri::RunEvent::Exit => crate::helpers::clamd::shutdown(),
            _ => {}
        });
}
