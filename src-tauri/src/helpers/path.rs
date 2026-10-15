//use std::path::Path;

use std::{path::PathBuf, process::Command};

use crate::helpers::{resolve_command, silent_command};

/// Per-user folder holding the app's own freshclam.conf and signature database.
/// Owning it means updates never need admin rights (Program Files is read-only for users)
/// and no manual ClamAV configuration. Resolved without an AppHandle so headless scans can use it.
pub fn clamav_data_dir() -> PathBuf {
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    };
    base.unwrap_or_else(std::env::temp_dir)
        .join("com.clamavgui.app")
        .join("clamav")
}

pub fn clamav_db_dir() -> PathBuf {
    clamav_data_dir().join("database")
}

pub fn has_signatures() -> bool {
    std::fs::read_dir(clamav_db_dir())
        .map(|mut entries| entries.any(|e| e.is_ok()))
        .unwrap_or(false)
}

/// Creates the database folder and a default freshclam.conf if missing.
/// An existing config is kept, so admins can add proxy or mirror settings.
pub fn ensure_freshclam_config() -> Result<PathBuf, String> {
    let db_dir = clamav_db_dir();
    std::fs::create_dir_all(&db_dir).map_err(|e| e.to_string())?;
    let config = clamav_data_dir().join("freshclam.conf");
    if !config.exists() {
        let contents = format!(
            "DatabaseDirectory {}\nDatabaseMirror database.clamav.net\n",
            db_dir.display()
        );
        std::fs::write(&config, contents).map_err(|e| e.to_string())?;
    }
    Ok(config)
}

/// freshclam bound to the app-owned config.
pub fn freshclam_command() -> Result<Command, String> {
    let config = ensure_freshclam_config()?;
    let mut cmd = silent_command(freshclam_path()?);
    cmd.arg(format!("--config-file={}", config.display()));
    Ok(cmd)
}

/// freshclam ships next to clamscan, so this works even when ClamAV isn't in PATH.
fn freshclam_path() -> Result<PathBuf, String> {
    let sibling = get_clamav_path()?
        .with_file_name(if cfg!(windows) { "freshclam.exe" } else { "freshclam" });
    if sibling.is_file() {
        Ok(sibling)
    } else {
        resolve_command("freshclam")
    }
}

/// clamscan bound to the app-owned signature database.
pub fn clamscan_command() -> Result<Command, String> {
    let mut cmd = silent_command(get_clamav_path()?);
    cmd.arg(format!("--database={}", clamav_db_dir().display()));
    Ok(cmd)
}

#[cfg(windows)]
fn normalize_path(path: &str) -> String {
    path.replace('\\', r"\\")
}

#[cfg(not(windows))]
fn normalize_path(path: &str) -> String {
    path.to_string()
}

/// Convert a filesystem path into a safe, anchored regex
/// usable by ClamAV and the realtime exclusion matcher.
pub fn path_to_regex(path: &str) -> String {
    // Escape regex metacharacters
    let escaped = regex::escape(path);

    // Platform-specific normalization
    let normalized = normalize_path(&escaped);

    // Anchor to start
    format!("^{}", normalized)
}

// Convenience helper when you already have a Path
// pub fn pathbuf_to_regex(path: &Path) -> String {
//     path_to_regex(&path.to_string_lossy())
// }

#[cfg(target_os = "macos")]
const MACOS_CLAMSCAN_PATHS: &[&str] = &[
    "/opt/homebrew/bin/clamscan",
    "/usr/local/bin/clamscan",
];
pub fn get_clamav_path() -> Result<PathBuf,String>{
    #[cfg(debug_assertions)]
    println!("PATH={:?}", std::env::var("PATH"));
    
    if let Ok(path) = resolve_command("clamscan") {
        #[cfg(debug_assertions)]
        println!("clamscan={:?}", path);
        return Ok(path);
    }
    if let Err(err) = resolve_command("clamscan") {
        #[cfg(debug_assertions)]
        println!("clamscan lookup failed: {}", err);
    }
    #[cfg(target_os = "macos")]
    {
        for path in MACOS_CLAMSCAN_PATHS {
            let candidate = PathBuf::from(path);

            if candidate.is_file() {
                #[cfg(debug_assertions)]
                println!("Found macOS fallback path: {:?}", candidate);

                return Ok(candidate);
            }
        }
    }
    // The official MSI installs here; PATH may be missing it or not yet refreshed
    // in this process right after an automatic install.
    #[cfg(windows)]
    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        let candidate = PathBuf::from(program_files).join("ClamAV").join("clamscan.exe");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err("ClamAV not found. Please install ClamAV and ensure clamscan is available in PATH, or report the bug if something went wrong.".into(),)
}