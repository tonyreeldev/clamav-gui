use notify::{event::ModifyKind, EventKind, RecursiveMode, Watcher};
use once_cell::sync::Lazy;
use tauri::Emitter;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

static NOTIFIED: Lazy<Mutex<HashSet<PathBuf>>> = Lazy::new(|| Mutex::new(HashSet::new()));
pub static REALTIME_ENABLED: AtomicBool = AtomicBool::new(false);
/// Low-memory mode: clamd (~1 GB) is loaded on the first file and unloaded after IDLE_UNLOAD without activity.
pub static LOW_MEMORY: AtomicBool = AtomicBool::new(false);
const IDLE_UNLOAD: Duration = Duration::from_secs(15 * 60);

fn idle_unload() -> Duration {
    // Development builds only: lets the unload be checked without waiting 15 minutes.
    #[cfg(debug_assertions)]
    if let Some(secs) = std::env::var("CLAMAVGUI_IDLE_UNLOAD_SECS").ok().and_then(|v| v.parse().ok()) {
        return Duration::from_secs(secs);
    }
    IDLE_UNLOAD
}
static CURRENT_MODE: Mutex<Option<BehaviorMode>> = Mutex::new(None);

pub fn current_mode() -> Option<BehaviorMode> {
    *CURRENT_MODE.lock().unwrap_or_else(|e| e.into_inner())
}
/// Failed scan attempts per file (locked or still being written); see MAX_SCAN_RETRIES.
static RETRIES: Lazy<Mutex<HashMap<PathBuf, u8>>> = Lazy::new(|| Mutex::new(HashMap::new()));
const MAX_SCAN_RETRIES: u8 = 5;
/// Files waiting to be scanned, with the time of their latest event.
static PENDING: Lazy<Mutex<HashMap<PathBuf, Instant>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static BEHAVIOR: Lazy<Arc<Mutex<BehaviorConfig>>> =
    Lazy::new(|| Arc::new(Mutex::new(behavior_config(BehaviorMode::Balanced))));

use crate::{
    antivirus::{quarantine::quarantine_file},
    helpers::{
        clamd,
        history::append_realtime_history,
        log::{initialize_log_with_id, log_err, log_info},
        new_id,
        matcher::{user_exclusions, EXCLUSIONS, ExclusionMatcher},
        path::{clamscan_command, path_to_regex},
        notifier::{notify, NotifyKind},
        i18n::t,
    },
    types::{
        enums::{BehaviorMode, HistoryDetails, HistoryStatus, HistoryType, LogCategory, ScanType},
        structs::{BehaviorConfig, HistoryItem}
    },
};

/// Executables, scripts, installers and shortcuts: the usual first stage of an infection.
const EXECUTABLE_TYPES: &[&str] = &[
    "exe", "dll", "com", "scr", "msi", "msix", "bat", "cmd", "ps1", "js", "jse", "vbs", "vbe", "wsf", "hta", "jar", "lnk",
];
/// Containers that commonly carry them: macro-enabled Office files, archives and disk images.
const CONTAINER_TYPES: &[&str] = &["pdf", "docm", "xlsm", "pptm", "zip", "rar", "7z", "cab", "iso", "img"];

fn behavior_config(mode: BehaviorMode) -> BehaviorConfig {
    let with_containers = || [EXECUTABLE_TYPES, CONTAINER_TYPES].concat();
    match mode {
        BehaviorMode::Balanced => BehaviorConfig {
            scan_extensions: with_containers(),
            auto_quarantine: true,
            rescan_on_modify: false,
        },
        BehaviorMode::Safe => BehaviorConfig {
            scan_extensions: EXECUTABLE_TYPES.to_vec(),
            auto_quarantine: false,
            rescan_on_modify: false,
        },
        BehaviorMode::Strict => BehaviorConfig {
            scan_extensions: [with_containers(), vec!["txt"]].concat(),
            auto_quarantine: true,
            rescan_on_modify: true,
        },
        BehaviorMode::Expert => BehaviorConfig {
            scan_extensions: vec![],
            auto_quarantine: false,
            rescan_on_modify: true,
        },
    }
}

/// Bumped on every start/stop; threads from an older session see the change and exit.
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// A file is scanned once it has had no events for this long, so half-written downloads aren't scanned.
const QUIET_PERIOD: Duration = Duration::from_secs(2);

fn is_current(generation: u64) -> bool {
    GENERATION.load(Ordering::SeqCst) == generation
}

fn start_watcher(paths: Vec<String>, generation: u64, log: LogFile) -> Result<(), String> {
    if paths.is_empty() {
        return Err("No paths provided for monitoring".into());
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(tx).map_err(|e| e.to_string())?;
    let mut failed_paths = Vec::new();
    for path in &paths {
        if let Err(e) = watcher.watch(path.as_ref(), RecursiveMode::Recursive) {
            log_err(&log, &format!("Failed to watch {}: {}", path, e));
            failed_paths.push(path.clone());
        }
    }
    if failed_paths.len() == paths.len() {
        return Err(format!("Could not watch any path: {}", failed_paths.join(", ")));
    }

    thread::spawn(move || {
        let _watcher = watcher; // dropping it stops the OS notifications
        while is_current(generation) {
            let event = match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(Ok(event)) => event,
                Ok(Err(e)) => {
                    log_err(&log, &format!("File watcher error: {}", e));
                    continue;
                }
                Err(_) => continue,
            };
            // Renames count as new files: browsers download to a temp name and rename at the end.
            let is_new = matches!(event.kind, EventKind::Create(_) | EventKind::Modify(ModifyKind::Name(_)));
            if !is_new && !matches!(event.kind, EventKind::Modify(_)) {
                continue;
            }
            let behavior = BEHAVIOR.lock().unwrap().clone();
            for path in event.paths {
                let excluded = EXCLUSIONS
                    .lock()
                    .unwrap()
                    .as_ref()
                    .is_some_and(|m| m.is_excluded(&path));
                if excluded || !should_scan(&path, &behavior) {
                    continue;
                }
                let mut pending = PENDING.lock().unwrap();
                // A new file still being written stays pending; other edits only count in rescan modes.
                if is_new || behavior.rescan_on_modify || pending.contains_key(&path) {
                    pending.insert(path, Instant::now());
                }
            }
        }
    });

    Ok(())
}

fn take_settled() -> Vec<PathBuf> {
    let mut pending = PENDING.lock().unwrap();
    let ready: Vec<PathBuf> = pending
        .iter()
        .filter(|(_, last)| last.elapsed() >= QUIET_PERIOD)
        .map(|(path, _)| path.clone())
        .collect();
    for path in &ready {
        pending.remove(path);
    }
    ready
}

fn start_scan_worker(app: tauri::AppHandle, log_id: String, generation: u64, log: LogFile) {
    thread::spawn(move || {
        // Reported once per session in the history; every later failure only goes to the log file.
        let mut reported_clamd_down = false;
        let mut clamd_down = |err: String, app: &tauri::AppHandle| {
            log_err(&log, &format!("clamd unavailable, using clamscan (slower): {}", err));
            if !reported_clamd_down {
                reported_clamd_down = true;
                record(app, &log, &log_id, HistoryType::RealTimeError, HistoryStatus::Error, HistoryDetails::RealTimeError { err });
            }
        };
        // Load the signatures now rather than on the first file, unless memory matters more than the first ~10 s.
        let mut clamd_loaded = clamd::is_alive();
        if !LOW_MEMORY.load(Ordering::SeqCst) {
            match clamd::ensure_running() {
                Ok(()) => clamd_loaded = true,
                Err(e) => clamd_down(e, &app),
            }
        }
        let mut last_activity = Instant::now();
        while is_current(generation) {
            thread::sleep(Duration::from_secs(1));
            let batch = take_settled();
            if batch.is_empty() {
                if clamd_loaded && LOW_MEMORY.load(Ordering::SeqCst) && last_activity.elapsed() >= idle_unload() {
                    clamd::shutdown();
                    clamd_loaded = false;
                    log_info(&log, &format!("Low-memory mode: signatures unloaded after {} minutes without new files", idle_unload().as_secs() / 60));
                }
                continue;
            }
            last_activity = Instant::now();
            let behavior = BEHAVIOR.lock().unwrap().clone();

            for path in batch {
                if !is_current(generation) {
                    break;
                }
                if !path.is_file() {
                    continue; // deleted or renamed again before it settled
                }
                if !clamd_loaded && LOW_MEMORY.load(Ordering::SeqCst) {
                    // Tells the user what keeps the signatures in memory in low-memory mode.
                    log_info(&log, &format!("Low-memory mode: loading signatures to scan {}", path.display()));
                }
                let result = match clamd::ensure_running() {
                    Ok(()) => {
                        clamd_loaded = true;
                        clamd::scan(&path)
                    }
                    Err(e) => {
                        clamd_down(e, &app);
                        scan_file(&path)
                    }
                };
                match result {
                    Ok(Some(threat_name)) => {
                        RETRIES.lock().unwrap().remove(&path);
                        if !NOTIFIED.lock().unwrap().insert(path.clone()) {
                            continue;
                        }
                        handle_detection(&app, &log, &log_id, &behavior, &path, threat_name);
                    }
                    Ok(None) => {
                        RETRIES.lock().unwrap().remove(&path);
                    }
                    Err(e) => {
                        // Often the file is still locked by whoever is writing it: try again a few times before giving up.
                        let mut retries = RETRIES.lock().unwrap();
                        let attempts = retries.entry(path.clone()).or_insert(0);
                        *attempts += 1;
                        if *attempts <= MAX_SCAN_RETRIES {
                            PENDING.lock().unwrap().insert(path, Instant::now());
                        } else {
                            retries.remove(&path);
                            log_err(&log, &format!("Could not scan {}: {}", path.display(), e));
                        }
                    }
                }
            }
        }
    });
}

type LogFile = Arc<Mutex<std::fs::File>>;

fn record(app: &tauri::AppHandle, log: &LogFile, log_id: &str, action: HistoryType, status: HistoryStatus, details: HistoryDetails) {
    append_realtime_history(
        app,
        HistoryItem {
            id: new_id(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            action: Some(action),
            details: Some(details),
            status,
            category: Some(LogCategory::Realtime),
            log_id: Some(log_id.to_string()),
            scan_type: Some(ScanType::Realtime),
            threat_count: Some(1),
            scan_result: None,
        },
        log,
    );
}

/// The notification must say what really happened: "blocked" only once the file is in quarantine.
fn handle_detection(app: &tauri::AppHandle, log: &LogFile, log_id: &str, behavior: &BehaviorConfig, path: &Path, threat_name: String) {
    let file = path.display().to_string();
    log_info(log, &format!("Threat detected: {} in {}", threat_name, file));
    let quarantined = behavior.auto_quarantine
        && match quarantine_file(app.clone(), file.clone(), threat_name.clone(), Some(log_id.to_string())) {
            Ok(()) => true,
            Err(e) => {
                log_err(log, &format!("Could not quarantine {}: {}", file, e));
                false
            }
        };
    let mut body = t("notification.threat-body").replace("{threat}", &threat_name).replace("{file}", &file);
    let title = if quarantined {
        t("notification.threat-blocked")
    } else if behavior.auto_quarantine {
        body = format!("{}\n{}", body, t("notification.threat-not-isolated-hint"));
        t("notification.threat-not-isolated")
    } else {
        t("notification.threat-detected")
    };
    notify(app, &title, &body, NotifyKind::Alert);

    if quarantined {
        // quarantine_file already wrote the history entry. The path is free again: a new copy there must be caught too.
        NOTIFIED.lock().unwrap().remove(path);
        let _ = app.emit("realtime:quarantined", ());
    } else {
        // Nothing else records it, so a missed notification would otherwise lose the detection.
        record(
            app,
            log,
            log_id,
            HistoryType::RealTimeThreat,
            if behavior.auto_quarantine { HistoryStatus::Error } else { HistoryStatus::Warning },
            HistoryDetails::RealTimeThreat { threat: threat_name, file_path: file, quarantine_failed: behavior.auto_quarantine },
        );
    }
}

fn should_scan(path: &Path, behavior: &BehaviorConfig) -> bool {
    if !path.is_file() {
        return false;
    }
    if behavior.scan_extensions.is_empty() {
        return true;
    }
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => {
            let ext_lower = ext.to_lowercase();
            behavior
                .scan_extensions
                .iter()
                .any(|e| e.eq_ignore_ascii_case(&ext_lower))
        }
        None => false,
    }
}

/// Fallback when clamd can't run: loads the whole database for one file, so it is slow.
fn scan_file(path: &Path) -> Result<Option<String>, String> {
    let mut scanner = clamscan_command()?;
    let output = scanner
        .arg("--no-summary")
        .arg(path)
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if output.status.code() == Some(1) {
        if let Some(line) = stdout.lines().find(|l| l.contains(" FOUND")) {
            if let Some((_, rest)) = line.split_once(": ") {
                if let Some((name, _)) = rest.split_once(" FOUND") {
                    return Ok(Some(name.trim().to_string()));
                }
            }
        }
        return Ok(Some("Unknown.Threat".into()));
    }

    if let Some(code) = output.status.code() {
        if code != 0 {
            eprintln!("Clamscan exited with code {} for {:?}", code, path);
        }
    }
    Ok(None)
}

pub fn start_realtime_scan(
    app: tauri::AppHandle,
    paths: Vec<String>,
    mode: BehaviorMode,
    log_id: String,
) -> Result<(), String> {
    {
        #[allow(unused_mut)]
        let mut exclusions = user_exclusions(&app)?;

        #[cfg(target_os = "macos")]
        {
            use crate::helpers::{matcher::DEFAULT_MACOS_EXCLUDED_DIRS, settings::get_settings_as_bool};
            use crate::types::enums::SettingKeyBool;
            let include_mac_exclusions =
                get_settings_as_bool(&app, SettingKeyBool::IncludeMacSystemExclusions)
                    .unwrap_or(false);

            if include_mac_exclusions {
                exclusions.extend(
                    DEFAULT_MACOS_EXCLUDED_DIRS.iter().map(|p| p.to_string())
                );
            }
        }

        let patterns: Vec<String> = exclusions
            .into_iter()
            .map(|e| path_to_regex(&e)) // or convert path → regex here
            .collect();

        let matcher = ExclusionMatcher::new(patterns).map_err(|e| e.to_string())?;

        *EXCLUSIONS.lock().unwrap() = Some(matcher);
    }
    if paths.is_empty() {
        return Err("No paths provided for real-time scanning".into());
    }
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    PENDING.lock().unwrap().clear();
    RETRIES.lock().unwrap().clear();
    NOTIFIED.lock().unwrap().clear();
    *BEHAVIOR.lock().unwrap() = behavior_config(mode);
    *CURRENT_MODE.lock().unwrap() = Some(mode);
    let log = initialize_log_with_id(&app, LogCategory::Realtime, &log_id)?.file;
    // The bump above already ended any previous session, so a failure leaves protection off.
    start_watcher(paths, generation, log.clone()).inspect_err(|_| REALTIME_ENABLED.store(false, Ordering::SeqCst))?;
    REALTIME_ENABLED.store(true, Ordering::SeqCst);
    start_scan_worker(app, log_id, generation, log);

    Ok(())
}

pub fn stop_realtime_scan() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    REALTIME_ENABLED.store(false, Ordering::SeqCst);
    *EXCLUSIONS.lock().unwrap() = None;
    PENDING.lock().unwrap().clear();
    NOTIFIED.lock().unwrap().clear();
    // Off the command thread: clamd may take a moment to answer.
    thread::spawn(|| {
        if !REALTIME_ENABLED.load(Ordering::SeqCst) {
            clamd::shutdown();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_watch_the_right_file_types() {
        let balanced = behavior_config(BehaviorMode::Balanced).scan_extensions;
        let safe = behavior_config(BehaviorMode::Safe).scan_extensions;
        for ext in ["msi", "scr", "lnk", "hta", "docm", "rar", "iso", "exe"] {
            assert!(balanced.contains(&ext), "balanced misses {}", ext);
        }
        assert!(safe.contains(&"msi") && !safe.contains(&"zip"));
        assert!(behavior_config(BehaviorMode::Strict).scan_extensions.contains(&"txt"));
        assert!(behavior_config(BehaviorMode::Expert).scan_extensions.is_empty());
    }
}
