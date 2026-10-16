use std::{
    io::{BufRead as _, BufReader, Read},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tauri::{path::PathResolver, Emitter, Wry};
use walkdir::WalkDir;

use crate::{
    helpers::{
        history::append_scan_history, log::{log_err, log_info, log_path}, matcher::{exclusions_from_disk, ExclusionMatcher}, new_id, path::{clamscan_command, path_to_regex}
    },
    types::{
        enums::{HistoryDetails, HistoryStatus, HistoryType, LogCategory, ScanResult, ScanType},
        structs::{HistoryItem, StartupScan},
    },
};

pub fn estimate_total_files(paths: &[PathBuf], exclusions: Option<ExclusionMatcher>) -> u64 {
    paths
        .iter()
        .flat_map(|p| WalkDir::new(p).max_depth(10).into_iter())
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            exclusions
                .as_ref()
                .map(|m| !m.is_excluded(e.path()))
                .unwrap_or(true)
        })
        .count() as u64
}

pub static SCAN_PROCESS: once_cell::sync::Lazy<Mutex<Option<u32>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(None));

/// Set from the moment a scan command is accepted until its thread ends,
/// so a second request is refused before it writes any history.
static SCAN_BUSY: AtomicBool = AtomicBool::new(false);

pub struct ScanSlot;
impl Drop for ScanSlot {
    fn drop(&mut self) {
        SCAN_BUSY.store(false, Ordering::SeqCst);
    }
}

pub fn claim_scan() -> Result<ScanSlot, String> {
    if SCAN_BUSY.swap(true, Ordering::SeqCst) {
        return Err("SCAN_ALREADY_RUNNING".into());
    }
    Ok(ScanSlot)
}

pub fn is_scan_busy() -> bool {
    SCAN_BUSY.load(Ordering::SeqCst)
}

/// Set by stop_scan: a killed clamscan exits with 1, which would otherwise read as "threats found".
pub static SCAN_STOPPED: AtomicBool = AtomicBool::new(false);

/// Lines decoded lossily: `lines()` would silently drop any line that isn't valid UTF-8 (e.g. a detection in an oddly named file).
fn lossy_lines(reader: impl Read) -> impl Iterator<Item = String> {
    BufReader::new(reader)
        .split(b'\n')
        .map_while(Result::ok)
        .map(|bytes| String::from_utf8_lossy(&bytes).trim_end_matches('\r').to_string())
}

pub fn run_scan(
    app: tauri::AppHandle,
    log_id: String,
    mut cmd: Command,
    scan_type: ScanType,
) -> Result<(), String> {
    {
        let guard = SCAN_PROCESS.lock().unwrap();
        if guard.is_some() {
            #[cfg(debug_assertions)]
            println!("Scan Process Already running: {:?}", *guard);
            return Err("SCAN_ALREADY_RUNNING".into());
        }
    }

    let log_path = log_path(&app, LogCategory::Scan, &log_id);
    let log_file = Arc::new(Mutex::new(
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| e.to_string())?,
    ));

    SCAN_STOPPED.store(false, Ordering::SeqCst);
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    {
        let mut guard = SCAN_PROCESS.lock().unwrap();
        *guard = Some(child.id());
    }

    let threats_count = Arc::new(AtomicUsize::new(0));

    let stdout_handle = if let Some(out) = child.stdout.take() {
        let app_clone = app.clone();
        let log_clone = log_file.clone();
        let threats_clone = threats_count.clone();

        Some(std::thread::spawn(move || {
            for line in lossy_lines(out) {
                if line.ends_with(" FOUND") {
                    threats_clone.fetch_add(1, Ordering::Relaxed);
                }
                let _ = app_clone.emit("clamscan:log", &line)
                    .map_err(|e| e.to_string());
                log_info(&log_clone, &line);
            }
        }))
    } else {
        None
    };

    let stderr_handle = if let Some(err) = child.stderr.take() {
        let app_clone = app.clone();
        let log_clone = log_file.clone();

        Some(std::thread::spawn(move || {
            for line in lossy_lines(err) {
                let _ = app_clone.emit("clamscan:log", &line)
                    .map_err(|e| e.to_string());
                log_err(&log_clone, &line);
            }
        }))
    } else {
        None
    };

    let exit_code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);

    if let Some(handle) = stdout_handle {
        handle.join().ok();
    }
    if let Some(handle) = stderr_handle {
        handle.join().ok();
    }

    {
        let mut guard = SCAN_PROCESS.lock().unwrap();
        *guard = None;
    }

    let found = threats_count.load(Ordering::Relaxed);

    let (status, scan_result) = match exit_code {
        _ if SCAN_STOPPED.swap(false, Ordering::SeqCst) => (
            HistoryStatus::Warning,
            ScanResult::Cancelled,
        ),
        0 => (
            HistoryStatus::Success,
            ScanResult::Clean,
        ),
        1 => (
            HistoryStatus::Warning,
            ScanResult::ThreatsFound,
        ),
        2 if found > 0 => (
            HistoryStatus::Warning,
            ScanResult::Partial,
        ),
        2 => (
            HistoryStatus::Error,
            ScanResult::ClamavError,
        ),
        _ => (
            HistoryStatus::Error,
            ScanResult::Failed,
        ),
    };

    append_scan_history(
        &app,
        HistoryItem {
            id: new_id(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            action: Some(HistoryType::ScanFinish),
            details: Some(HistoryDetails::ScanFinish { result: scan_result, exit_code, found_threats: found }),
            status,
            category: Some(LogCategory::Scan),
            log_id: Some(log_id),
            scan_type: Some(scan_type),
            threat_count: Some(found as u32),
            scan_result: Some(scan_result),
        },
        &log_file,
    );

    app.emit("clamscan:finished", exit_code)
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_root_path() -> &'static str {
    if cfg!(windows) {
        "C:\\"
    } else {
        "/"
    }
}

pub fn get_main_paths(resolver: &PathResolver<Wry>) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = [
        resolver.audio_dir(),
        resolver.desktop_dir(),
        resolver.document_dir(),
        resolver.download_dir(),
        resolver.picture_dir(),
        resolver.video_dir(),
    ]
    .into_iter()
    .flatten() // remove None
    .collect();

    #[cfg(windows)]
    {
        paths.extend([
            resolver.home_dir(),
            resolver.local_data_dir(),
            resolver.public_dir(),
            resolver.temp_dir(),
        ].into_iter().flatten());
    }

    paths.retain(|p| p.exists());
    paths.sort();
    // The scan is recursive, so a folder inside another listed one (e.g. Desktop inside home) would be scanned twice.
    let mut roots: Vec<PathBuf> = Vec::new();
    for p in paths {
        if !roots.iter().any(|r| p.starts_with(r)) {
            roots.push(p);
        }
    }
    let paths = roots;

    #[cfg(debug_assertions)]
    for p in &paths {
        println!("Main scan path: {}", p.display());
    }

    paths
}

pub fn run_headless_scan(startup: StartupScan) -> Result<(), String> {
    let scan_type = match startup.scan_type {
        Some(s) => s,
        None => return Ok(()),
    };
    let mut cmd = clamscan_command()?;
    let mut paths = Vec::new();
    if let Some(home) = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }) {
        let home = PathBuf::from(home);
        paths.push(home.join("Desktop"));
        paths.push(home.join("Documents"));
        paths.push(home.join("Downloads"));
    }
    if cfg!(windows) {
        paths.push(PathBuf::from("C:\\Program Files"));
        paths.push(PathBuf::from("C:\\Program Files (x86)"));
    } else {
        paths.push(PathBuf::from("/usr"));
        paths.push(PathBuf::from("/home"));
    }
    // Runs before Tauri starts, so the store is read from disk.
    for item in exclusions_from_disk() {
        cmd.arg(format!("--exclude-dir={}", path_to_regex(&item)));
    }
    match scan_type {
        ScanType::Main => {
            cmd.args([
                "--recursive",
                "--heuristic-alerts",
                "--alert-encrypted",
                "--max-filesize=100M",
                "--max-scansize=400M",
            ]);
            #[cfg(debug_assertions)]
            for p in &paths {
                println!("Headless scan path: {}", p.display());
            }
            for path in paths {
                cmd.arg(path);
            }
        }
        ScanType::Full => {
            cmd.args([
                "--recursive",
                "--cross-fs=yes",
                "--heuristic-alerts",
                "--alert-encrypted",
                get_root_path(),
            ]);
        }
        _ => return Ok(()),
    }

    let status = cmd.status().map_err(|e| e.to_string())?;
    match status.code() {
        Some(0) | Some(1) => Ok(()),
        Some(2) => Err("ClamAV scan error".into()),
        _ => Err("Scan failed".into()),
    }
}

pub fn fetch_custom_scan_args(args: Option<Vec<String>>, has_directory: bool) -> Vec<String> {
    let mut default_args = vec![
        "--heuristic-alerts".to_string(),
        "--alert-encrypted".to_string(),
        "--max-filesize=100M".to_string(),
        "--max-scansize=400M".to_string(),
        "--verbose".to_string(),
        "--no-summary".to_string(),
    ];
    if has_directory {
        default_args.push("--recursive".to_string());
    }
    if let Some(args) = args {
        args
    } else {
        default_args
    }
}
