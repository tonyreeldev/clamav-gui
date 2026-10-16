//! "Windows Security" panel: reports whether the protections ClamAV doesn't provide
//! (firewall, Defender, network protection, ransomware protection, SmartScreen) are on.
//! Read-only: it never changes a setting, it only opens the matching Windows Security page.

use serde::{Deserialize, Serialize};
use specta::{specta, Type};
use tauri::command;

#[derive(Serialize, Deserialize, Type, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SecurityCheck {
    Firewall,
    Antivirus,
    NetworkProtection,
    RansomwareProtection,
    SmartScreen,
}

#[derive(Serialize, Type, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SecurityState {
    On,
    Off,
    /// Only some parts are on (e.g. firewall off for public networks).
    Partial,
    /// Logs but doesn't block.
    Audit,
    /// Another product handles it (third-party antivirus or firewall).
    ThirdParty,
    /// On, but its signatures are old.
    Outdated,
    /// Depends on Defender, which isn't the active antivirus.
    NotApplicable,
    /// Couldn't be read (policy, Windows Server, PowerShell blocked...).
    Unknown,
}

#[derive(Serialize, Type, Debug, Clone, PartialEq, Eq)]
pub struct SecurityItem {
    pub check: SecurityCheck,
    pub state: SecurityState,
    /// Product name, firewall profiles that are off, signature age in days, or which SmartScreen is off.
    pub detail: Option<String>,
}

/// Security Center product entry; productState bit 0x1000 = enabled, 0x10 = signatures out of date.
#[derive(Deserialize, Debug, Default)]
struct Product {
    name: Option<String>,
    state: Option<i64>,
}

impl Product {
    fn enabled(&self) -> bool {
        self.state.is_some_and(|s| s & 0x1000 != 0)
    }
    fn is_defender(&self) -> bool {
        self.name.as_deref().is_some_and(|n| n.contains("Defender"))
    }
}

#[derive(Deserialize, Debug, Default)]
struct Profile {
    name: String,
    enabled: bool,
}

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
struct Defender {
    mode: Option<String>,
    av: bool,
    rtp: bool,
    sig_age: Option<i64>,
}

/// What the PowerShell script prints; every field is optional because any query may fail on its own.
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
struct Raw {
    firewall: Option<Vec<Profile>>,
    third_firewalls: Vec<Product>,
    antivirus: Vec<Product>,
    defender: Option<Defender>,
    network_protection: Option<i64>,
    controlled_folder_access: Option<i64>,
    smart_screen: Option<String>,
    smart_screen_policy: Option<i64>,
    edge_smart_screen: Option<i64>,
}

const SIGNATURES_MAX_AGE_DAYS: i64 = 3;

const SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$o = [ordered]@{}
$fw = Get-NetFirewallProfile
if ($fw) { $o.firewall = @($fw | ForEach-Object { [ordered]@{ name = "$($_.Name)"; enabled = ("$($_.Enabled)" -eq 'True') } }) }
$o.thirdFirewalls = @(Get-CimInstance -Namespace root/SecurityCenter2 -ClassName FirewallProduct | ForEach-Object { [ordered]@{ name = $_.displayName; state = [int64]$_.productState } })
$o.antivirus = @(Get-CimInstance -Namespace root/SecurityCenter2 -ClassName AntiVirusProduct | ForEach-Object { [ordered]@{ name = $_.displayName; state = [int64]$_.productState } })
$s = Get-MpComputerStatus
if ($s) { $o.defender = [ordered]@{ mode = "$($s.AMRunningMode)"; av = [bool]$s.AntivirusEnabled; rtp = [bool]$s.RealTimeProtectionEnabled; sigAge = [int64]$s.AntivirusSignatureAge } }
$p = Get-MpPreference
if ($p) { $o.networkProtection = [int64]$p.EnableNetworkProtection; $o.controlledFolderAccess = [int64]$p.EnableControlledFolderAccess }
$o.smartScreen = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer').SmartScreenEnabled
$pol = (Get-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System').EnableSmartScreen
if ($pol -ne $null) { $o.smartScreenPolicy = [int64]$pol }
$edge = (Get-ItemProperty 'HKCU:\Software\Microsoft\Edge\SmartScreenEnabled').'(default)'
if ($edge -ne $null) { $o.edgeSmartScreen = [int64]$edge }
$o | ConvertTo-Json -Depth 4 -Compress
"#;

fn item(check: SecurityCheck, state: SecurityState, detail: Option<String>) -> SecurityItem {
    SecurityItem { check, state, detail }
}

fn firewall(raw: &Raw) -> SecurityItem {
    use SecurityState::*;
    let third = raw.third_firewalls.iter().find(|p| p.enabled());
    let Some(profiles) = raw.firewall.as_ref().filter(|p| !p.is_empty()) else {
        return match third {
            Some(p) => item(SecurityCheck::Firewall, ThirdParty, p.name.clone()),
            None => item(SecurityCheck::Firewall, Unknown, None),
        };
    };
    let off: Vec<&str> = profiles.iter().filter(|p| !p.enabled).map(|p| p.name.as_str()).collect();
    if off.is_empty() {
        return item(SecurityCheck::Firewall, On, None);
    }
    // Another firewall replaces Windows': its own profiles being off is expected then.
    if let Some(p) = third {
        return item(SecurityCheck::Firewall, ThirdParty, p.name.clone());
    }
    let state = if off.len() == profiles.len() { Off } else { Partial };
    item(SecurityCheck::Firewall, state, Some(off.join(", ")))
}

/// Whether Defender is the active antivirus (network and ransomware protection only work then).
fn defender_active(raw: &Raw) -> bool {
    raw.defender
        .as_ref()
        .is_some_and(|d| d.av && d.mode.as_deref().is_some_and(|m| m.starts_with("Normal")))
}

fn antivirus(raw: &Raw) -> SecurityItem {
    use SecurityState::*;
    if let Some(other) = raw.antivirus.iter().find(|p| p.enabled() && !p.is_defender()) {
        return item(SecurityCheck::Antivirus, ThirdParty, other.name.clone());
    }
    let Some(d) = raw.defender.as_ref() else {
        // Defender's cmdlets unavailable: fall back to what Security Center says about it.
        return match raw.antivirus.iter().find(|p| p.is_defender()) {
            Some(p) if p.enabled() => item(SecurityCheck::Antivirus, On, p.name.clone()),
            Some(p) => item(SecurityCheck::Antivirus, Off, p.name.clone()),
            None => item(SecurityCheck::Antivirus, Unknown, None),
        };
    };
    if !d.av || !d.rtp {
        return item(SecurityCheck::Antivirus, Off, Some("Microsoft Defender".into()));
    }
    match d.sig_age {
        Some(age) if age > SIGNATURES_MAX_AGE_DAYS => item(SecurityCheck::Antivirus, Outdated, Some(age.to_string())),
        _ => item(SecurityCheck::Antivirus, On, Some("Microsoft Defender".into())),
    }
}

/// EnableNetworkProtection / EnableControlledFolderAccess: 0 off, 1 on, 2 audit,
/// 3 block disk modification only, 4 audit disk modification only (folder access).
fn defender_feature(raw: &Raw, check: SecurityCheck, value: Option<i64>) -> SecurityItem {
    use SecurityState::*;
    if raw.antivirus.iter().any(|p| p.enabled() && !p.is_defender()) && !defender_active(raw) {
        return item(check, NotApplicable, None);
    }
    let state = match value {
        Some(0) => Off,
        Some(1) => On,
        Some(2) | Some(4) => Audit,
        Some(3) => Partial,
        _ => Unknown,
    };
    item(check, state, None)
}

fn smart_screen(raw: &Raw) -> SecurityItem {
    use SecurityState::*;
    // Group policy wins over the user setting.
    let windows_on = match raw.smart_screen_policy {
        Some(0) => false,
        Some(_) => true,
        // Missing value means Windows' default, which is on ("Warn").
        None => !raw.smart_screen.as_deref().is_some_and(|v| v.eq_ignore_ascii_case("off")),
    };
    let edge_on = raw.edge_smart_screen != Some(0);
    match (windows_on, edge_on) {
        (true, true) => item(SecurityCheck::SmartScreen, On, None),
        (false, false) => item(SecurityCheck::SmartScreen, Off, None),
        (false, true) => item(SecurityCheck::SmartScreen, Partial, Some("Windows".into())),
        (true, false) => item(SecurityCheck::SmartScreen, Partial, Some("Microsoft Edge".into())),
    }
}

fn evaluate(raw: &Raw) -> Vec<SecurityItem> {
    vec![
        firewall(raw),
        antivirus(raw),
        defender_feature(raw, SecurityCheck::NetworkProtection, raw.network_protection),
        defender_feature(raw, SecurityCheck::RansomwareProtection, raw.controlled_folder_access),
        smart_screen(raw),
    ]
}

#[cfg(windows)]
fn query() -> Result<Raw, String> {
    use std::time::{Duration, Instant};
    let mut child = crate::helpers::silent_command("powershell")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", SCRIPT])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    // A stuck WMI query must not leave the panel loading forever.
    let started = Instant::now();
    while child.try_wait().map_err(|e| e.to_string())?.is_none() {
        if started.elapsed() > Duration::from_secs(20) {
            let _ = child.kill();
            return Err("Security status query timed out".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(text.trim()).map_err(|e| e.to_string())
}

/// Empty outside Windows: the panel is hidden there.
#[command(async)]
#[specta(result)]
pub fn get_security_status() -> Result<Vec<SecurityItem>, String> {
    #[cfg(windows)]
    {
        Ok(evaluate(&query()?))
    }
    #[cfg(not(windows))]
    {
        Ok(vec![])
    }
}

/// The fixed command that turns a protection on or off. Defender's antivirus can't be switched from outside
/// (Tamper Protection blocks it) and SmartScreen is split between Windows and Edge, so those only open their page.
fn toggle_command(check: SecurityCheck, enable: bool) -> Result<String, String> {
    let state = if enable { "Enabled" } else { "Disabled" };
    Ok(match check {
        SecurityCheck::Firewall => format!(
            "Set-NetFirewallProfile -Profile Domain,Private,Public -Enabled {}",
            if enable { "True" } else { "False" }
        ),
        SecurityCheck::NetworkProtection => format!("Set-MpPreference -EnableNetworkProtection {}", state),
        SecurityCheck::RansomwareProtection => format!("Set-MpPreference -EnableControlledFolderAccess {}", state),
        SecurityCheck::Antivirus | SecurityCheck::SmartScreen => {
            return Err("This protection can only be changed in Windows Security".into())
        }
    })
}

/// Runs the commands in one elevated PowerShell, so Windows shows a single UAC prompt.
#[cfg(windows)]
fn run_elevated(script: &str) -> Result<(), String> {
    // A declined UAC prompt makes Start-Process throw; 1223 is Windows' ERROR_CANCELLED.
    let outer = format!(
        "try {{ $p = Start-Process powershell -Verb RunAs -WindowStyle Hidden -Wait -PassThru \
         -ArgumentList '-NoProfile','-NonInteractive','-Command','$ErrorActionPreference=''Stop''; {}'; exit $p.ExitCode }} \
         catch {{ exit 1223 }}",
        script
    );
    let status = crate::helpers::silent_command("powershell")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &outer])
        .status()
        .map_err(|e| e.to_string())?;
    match status.code() {
        Some(0) => Ok(()),
        Some(1223) => Err("UAC_CANCELLED".into()),
        code => Err(format!("Windows rejected the change (exit code {})", code.unwrap_or(-1))),
    }
}

/// Turns protections on or off with one UAC prompt. Only the fixed commands above can run.
#[command(async)]
#[specta(result)]
pub fn set_security_features(checks: Vec<SecurityCheck>, enable: bool) -> Result<(), String> {
    if checks.is_empty() {
        return Ok(());
    }
    let script = checks
        .iter()
        .map(|c| toggle_command(*c, enable))
        .collect::<Result<Vec<_>, _>>()?
        .join("; ");
    #[cfg(windows)]
    {
        run_elevated(&script)
    }
    #[cfg(not(windows))]
    {
        let _ = script;
        Err("Only available on Windows".into())
    }
}

/// Opens the Windows Security page for a check. The target list is fixed, so the webview can't open anything else.
#[command]
#[specta(result)]
pub fn open_security_settings(app: tauri::AppHandle, check: SecurityCheck) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let uri = match check {
        SecurityCheck::Firewall => "windowsdefender://network",
        SecurityCheck::Antivirus | SecurityCheck::RansomwareProtection => "windowsdefender://threat",
        SecurityCheck::SmartScreen => "windowsdefender://appbrowser",
        // No Windows Security switch exists for it; the UI shows the command instead.
        SecurityCheck::NetworkProtection => return Err("Network protection has no settings page".into()),
    };
    app.opener().open_url(uri, None::<&str>).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use SecurityState::*;

    fn eval(json: &str) -> Vec<SecurityState> {
        evaluate(&serde_json::from_str::<Raw>(json).unwrap()).iter().map(|i| i.state).collect()
    }

    // Real output captured on the developer's PC (Windows 11, Defender, Edge).
    const THIS_PC: &str = r#"{"firewall":[{"name":"Domain","enabled":true},{"name":"Private","enabled":true},{"name":"Public","enabled":true}],"thirdFirewalls":[],"antivirus":[{"name":"Windows Defender","state":397568}],"defender":{"mode":"Normal","av":true,"rtp":true,"sigAge":0},"networkProtection":0,"controlledFolderAccess":0,"smartScreen":null,"edgeSmartScreen":1}"#;

    #[test]
    fn real_pc_output() {
        assert_eq!(eval(THIS_PC), vec![On, On, Off, Off, On]);
    }

    #[test]
    fn firewall_states() {
        let partial = r#"{"firewall":[{"name":"Private","enabled":true},{"name":"Public","enabled":false}]}"#;
        let raw: Raw = serde_json::from_str(partial).unwrap();
        assert_eq!(firewall(&raw), item(SecurityCheck::Firewall, Partial, Some("Public".into())));
        assert_eq!(eval(r#"{"firewall":[{"name":"Public","enabled":false}]}"#)[0], Off);
        assert_eq!(eval(r#"{"firewall":[{"name":"Public","enabled":false}],"thirdFirewalls":[{"name":"ESET","state":266240}]}"#)[0], ThirdParty);
        assert_eq!(eval(r#"{}"#)[0], Unknown);
    }

    #[test]
    fn antivirus_states() {
        assert_eq!(eval(r#"{"defender":{"mode":"Normal","av":true,"rtp":false}}"#)[1], Off);
        assert_eq!(eval(r#"{"defender":{"mode":"Normal","av":true,"rtp":true,"sigAge":9}}"#)[1], Outdated);
        // Third-party AV active: Defender passive, its network/ransomware features don't apply.
        let third = r#"{"antivirus":[{"name":"Windows Defender","state":393472},{"name":"Avast","state":266240}],"defender":{"mode":"Passive mode","av":false,"rtp":false},"networkProtection":1}"#;
        assert_eq!(&eval(third)[1..4], &[ThirdParty, NotApplicable, NotApplicable]);
        assert_eq!(eval(r#"{"antivirus":[{"name":"Windows Defender","state":393472}]}"#)[1], Off);
        assert_eq!(eval(r#"{}"#)[1], Unknown);
    }

    #[test]
    fn defender_feature_states() {
        let base = r#""defender":{"mode":"Normal","av":true,"rtp":true}"#;
        for (value, expected) in [(1, On), (2, Audit), (3, Partial), (4, Audit), (0, Off)] {
            let json = format!(r#"{{{},"networkProtection":{},"controlledFolderAccess":{}}}"#, base, value, value);
            assert_eq!(&eval(&json)[2..4], &[expected, expected], "value {}", value);
        }
        assert_eq!(&eval(&format!("{{{}}}", base))[2..4], &[Unknown, Unknown]);
    }

    #[test]
    fn toggle_commands_are_fixed() {
        assert_eq!(
            toggle_command(SecurityCheck::Firewall, true).unwrap(),
            "Set-NetFirewallProfile -Profile Domain,Private,Public -Enabled True"
        );
        assert_eq!(
            toggle_command(SecurityCheck::RansomwareProtection, false).unwrap(),
            "Set-MpPreference -EnableControlledFolderAccess Disabled"
        );
        assert!(toggle_command(SecurityCheck::Antivirus, false).is_err());
        assert!(toggle_command(SecurityCheck::SmartScreen, true).is_err());
    }

    #[test]
    fn smart_screen_states() {
        assert_eq!(eval(r#"{"smartScreen":"Off","edgeSmartScreen":0}"#)[4], Off);
        assert_eq!(eval(r#"{"smartScreen":"Warn","edgeSmartScreen":0}"#)[4], Partial);
        assert_eq!(eval(r#"{"smartScreenPolicy":0}"#)[4], Partial);
        assert_eq!(eval(r#"{"smartScreen":"Off","smartScreenPolicy":1}"#)[4], On);
    }
}
