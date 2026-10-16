use notify::{event::ModifyKind, EventKind, RecursiveMode, Watcher};
use once_cell::sync::Lazy;
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
/// Files waiting to be scanned, with the time of their latest event.
static PENDING: Lazy<Mutex<HashMap<PathBuf, Instant>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static BEHAVIOR: Lazy<Arc<Mutex<BehaviorConfig>>> =
    Lazy::new(|| Arc::new(Mutex::new(behavior_config(BehaviorMode::Balanced))));

use crate::{
    antivirus::{quarantine::quarantine_file},
    helpers::{
        clamd,
        matcher::{user_exclusions, EXCLUSIONS, ExclusionMatcher},
        path::{clamscan_command, path_to_regex},
        notifier::{notify, NotifyKind},
        i18n::t,
    },
    types::{
        enums::BehaviorMode,
        structs::BehaviorConfig
    },
};

fn behavior_config(mode: BehaviorMode) -> BehaviorConfig {
    match mode {
        BehaviorMode::Balanced => BehaviorConfig {
            scan_extensions: vec!["exe", "dll", "js", "vbs", "bat", "ps1", "pdf", "zip", "com"],
            auto_quarantine: true,
            rescan_on_modify: false,
        },
        BehaviorMode::Safe => BehaviorConfig {
            scan_extensions: vec!["exe", "dll", "js", "vbs", "bat", "ps1", "com"],
            auto_quarantine: false,
            rescan_on_modify: false,
        },
        BehaviorMode::Strict => BehaviorConfig {
            scan_extensions: vec!["exe", "dll", "js", "vbs", "bat", "ps1", "pdf", "zip", "com", "txt"],
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

fn start_watcher(paths: Vec<String>, generation: u64) -> Result<(), String> {
    if paths.is_empty() {
        return Err("No paths provided for monitoring".into());
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(tx).map_err(|e| e.to_string())?;
    let mut failed_paths = Vec::new();
    for path in &paths {
        if let Err(e) = watcher.watch(path.as_ref(), RecursiveMode::Recursive) {
            eprintln!("Failed to watch path {}: {}", path, e);
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
                    eprintln!("File watcher error: {}", e);
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

fn start_scan_worker(app: tauri::AppHandle, log_id: String, generation: u64) {
    thread::spawn(move || {
        // Load the signatures now rather than on the first file.
        if let Err(e) = clamd::ensure_running() {
            eprintln!("clamd unavailable, falling back to clamscan: {}", e);
        }
        while is_current(generation) {
            thread::sleep(Duration::from_secs(1));
            let batch = take_settled();
            if batch.is_empty() {
                continue;
            }
            let behavior = BEHAVIOR.lock().unwrap().clone();

            for path in batch {
                if !is_current(generation) {
                    break;
                }
                if !path.is_file() {
                    continue; // deleted or renamed again before it settled
                }
                let result = match clamd::ensure_running() {
                    Ok(()) => clamd::scan(&path),
                    Err(e) => {
                        eprintln!("clamd unavailable, falling back to clamscan: {}", e);
                        scan_file(&path)
                    }
                };
                match result {
                    Ok(Some(threat_name)) => {
                        let mut notified = NOTIFIED.lock().unwrap();
                        if notified.contains(&path) {
                            continue;
                        }
                        notified.insert(path.clone());
                        drop(notified);
                        let notification_title = t(if behavior.auto_quarantine {
                            "notification.threat-blocked"
                        } else {
                            "notification.threat-detected"
                        });
                        let notification_body = t("notification.threat-body")
                            .replace("{threat}", &threat_name)
                            .replace("{file}", &path.display().to_string());
                        notify(&app, &notification_title, &notification_body, NotifyKind::Alert);

                        if behavior.auto_quarantine {
                            if let Err(e) = quarantine_file(
                                app.clone(),
                                path.to_string_lossy().to_string(),
                                threat_name,
                                Some(log_id.clone()),
                            ) {
                                eprintln!("Failed to quarantine {}: {}", path.display(), e);
                            } else {
                                // The path is free again; a new copy dropped there must be caught too.
                                NOTIFIED.lock().unwrap().remove(&path);
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        eprintln!("Scan error for {:?}: {}", path, e);
                    }
                }
            }
        }
    });
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
    let mut invalid_paths = Vec::new();
    for path in &paths {
        let p = PathBuf::from(path);
        if !p.exists() {
            invalid_paths.push(path.clone());
        }
    }
    if !invalid_paths.is_empty() {
        return Err(format!(
            "The following paths do not exist: {}",
            invalid_paths.join(", ")
        ));
    }
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    PENDING.lock().unwrap().clear();
    NOTIFIED.lock().unwrap().clear();
    *BEHAVIOR.lock().unwrap() = behavior_config(mode);
    // The bump above already ended any previous session, so a failure leaves protection off.
    start_watcher(paths, generation).inspect_err(|_| REALTIME_ENABLED.store(false, Ordering::SeqCst))?;
    REALTIME_ENABLED.store(true, Ordering::SeqCst);
    start_scan_worker(app, log_id, generation);

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
