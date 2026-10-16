use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use once_cell::sync::Lazy;
use regex::Regex;

use crate::types::enums::SettingKeyArray;
#[cfg(target_os = "macos")]
use crate::types::enums::SettingKeyBool;

#[derive(Clone)]
pub struct ExclusionMatcher {
    pub regexes: Vec<Regex>,
}
impl ExclusionMatcher {
    pub fn new(patterns: Vec<String>) -> Result<Self, regex::Error> {
        let regexes = patterns
            .into_iter()
            .map(|p| Regex::new(&p))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { regexes })
    }
    pub fn is_excluded(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        self.regexes.iter().any(|r| r.is_match(&path_str))
    }
}

pub static EXCLUSIONS: Lazy<Arc<Mutex<Option<ExclusionMatcher>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));


#[cfg(target_os = "linux")]
const DEFAULT_LINUX_EXCLUDED_DIRS: &[&str] = &[
    "/proc",
    "/sys",
    "/dev",
    "/run",
];
#[cfg(target_os = "macos")]
pub const DEFAULT_MACOS_EXCLUDED_DIRS: &[&str] = &[
    "/System/Library/PrivateFrameworks/TextToSpeech.framework",
];

/// The user's exclusions plus the quarantine folder, which holds live malware:
/// scanning it would re-detect every item and quarantine the quarantine files themselves.
pub fn user_exclusions(app: &tauri::AppHandle) -> Result<Vec<String>, String> {
    use crate::helpers::{quarantine::quarantine_dir, settings::get_settings_as_array};
    let mut exclusions = get_settings_as_array(app, SettingKeyArray::Exclusions)?;
    exclusions.push(quarantine_dir(app).to_string_lossy().to_string());
    Ok(exclusions)
}

/// The exclusions as a matcher, for code that walks files itself.
pub fn settings_matcher(app: &tauri::AppHandle) -> Option<ExclusionMatcher> {
    use crate::helpers::path::path_to_regex;
    let exclusions = user_exclusions(app).ok()?;
    ExclusionMatcher::new(exclusions.iter().map(|e| path_to_regex(e)).collect()).ok()
}

/// Exclusions read straight from the settings store, for headless scans that run without Tauri.
pub fn exclusions_from_disk() -> Vec<String> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(std::path::PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share")))
    };
    let Some(app_dir) = base.map(|b| b.join("com.clamavgui.app")) else {
        return vec![];
    };
    let mut exclusions: Vec<String> = std::fs::read_to_string(app_dir.join("settings.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|json| json.get(SettingKeyArray::Exclusions.as_str()).cloned())
        .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
        .unwrap_or_default();
    exclusions.push(app_dir.join("quarantine").to_string_lossy().to_string());
    exclusions
}

pub fn apply_exclusions(app: &tauri::AppHandle, args: &mut Vec<String>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    use crate::helpers::settings::get_settings_as_bool;
    use crate::helpers::path::path_to_regex;

    #[cfg(target_os = "linux")]
    for dir in DEFAULT_LINUX_EXCLUDED_DIRS {
        args.push(format!("--exclude-dir={}", path_to_regex(dir)));
    }

    #[cfg(target_os = "macos")]
    {
        let include_mac_exclusions =
            get_settings_as_bool(app, SettingKeyBool::IncludeMacSystemExclusions)
                .unwrap_or(false);

        if include_mac_exclusions {
            for dir in DEFAULT_MACOS_EXCLUDED_DIRS {
                args.push(format!("--exclude-dir={}", path_to_regex(dir)));
            }
        }
    }

    let exclusions = user_exclusions(app)?;

    for item in exclusions.into_iter() {
        let regex = path_to_regex(&item);
        args.push(format!("--exclude-dir={}", regex));
    }
    Ok(())
}
