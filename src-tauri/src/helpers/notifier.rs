use serde::Deserialize;
use specta::Type;
use tauri_plugin_notification::NotificationExt;

/// Which shield variant a notification shows: blue = info, green = all good, red = alert.
#[derive(Deserialize, Type, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum NotifyKind {
    Info,
    Success,
    Alert,
}

impl NotifyKind {
    pub fn icon(self) -> &'static str {
        match self {
            NotifyKind::Info => "icons/blue.png",
            NotifyKind::Success => "icons/green.png",
            NotifyKind::Alert => "icons/red.png",
        }
    }
}

pub fn notify(app: &tauri::AppHandle, title: &str, body: &str, kind: NotifyKind) {
    #[cfg(windows)]
    {
        use tauri::Manager;
        use tauri_winrt_notification::{IconCrop, Toast};

        // Same rule as the notification plugin: the app's own AppUserModelID only works
        // for the installed app, so a build run from target/ borrows PowerShell's.
        let installed = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| !dir.ends_with("target/release") && !dir.ends_with("target/debug")))
            .unwrap_or(true);
        let app_id = if installed {
            app.config().identifier.clone()
        } else {
            Toast::POWERSHELL_APP_ID.to_string()
        };
        let mut toast = Toast::new(&app_id).title(title).text1(body);
        if let Ok(icon) = app.path().resolve(kind.icon(), tauri::path::BaseDirectory::Resource) {
            toast = toast.icon(&icon, IconCrop::Square, "");
        }
        if toast.show().is_ok() {
            return;
        }
    }
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        eprintln!("Failed to show notification: {}", e);
    }
}
