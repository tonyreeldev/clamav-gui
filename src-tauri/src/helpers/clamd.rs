//! Real-time protection talks to a clamd daemon that keeps the signatures loaded,
//! so each file takes milliseconds instead of a full clamscan database load (10-30 s).

use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    sync::Mutex,
    time::{Duration, Instant},
};

use crate::helpers::{
    path::{clamav_data_dir, clamav_db_dir, get_clamav_path},
    silent_command,
};

// Fixed so a clamd left behind by a crash is found and reused instead of loading a second copy.
const PORT: u16 = 3311;
const STARTUP_TIMEOUT: Duration = Duration::from_secs(180);
static STARTING: Mutex<()> = Mutex::new(());

fn request(cmd: &str, timeout: Duration) -> Result<String, String> {
    let addr = SocketAddr::from(([127, 0, 0, 1], PORT));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).map_err(|e| e.to_string())?;
    stream.set_read_timeout(Some(timeout)).map_err(|e| e.to_string())?;
    // "z" prefix: NUL-terminated command and reply, so paths with any characters are safe.
    stream.write_all(format!("z{}\0", cmd).as_bytes()).map_err(|e| e.to_string())?;
    let mut reply = Vec::new();
    stream.read_to_end(&mut reply).map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&reply).trim_end_matches('\0').trim().to_string())
}

pub fn is_alive() -> bool {
    request("PING", Duration::from_secs(5)).is_ok_and(|r| r == "PONG")
}

/// Rewritten on every start: the app owns this file, unlike freshclam.conf.
fn write_config() -> Result<std::path::PathBuf, String> {
    let config = clamav_data_dir().join("clamd.conf");
    let contents = format!(
        "DatabaseDirectory {}\nTCPSocket {}\nTCPAddr 127.0.0.1\nMaxFileSize 100M\nMaxScanSize 400M\n",
        clamav_db_dir().display(),
        PORT
    );
    std::fs::write(&config, contents).map_err(|e| e.to_string())?;
    Ok(config)
}

/// Starts clamd if it is not answering yet and waits until the signatures are loaded.
pub fn ensure_running() -> Result<(), String> {
    let _guard = STARTING.lock().unwrap_or_else(|e| e.into_inner());
    if is_alive() {
        return Ok(());
    }
    let clamd = get_clamav_path()?.with_file_name(if cfg!(windows) { "clamd.exe" } else { "clamd" });
    if !clamd.is_file() {
        return Err(format!("clamd not found at {}", clamd.display()));
    }
    let config = write_config()?;
    let mut child = silent_command(&clamd)
        .arg("--foreground")
        .arg(format!("--config-file={}", config.display()))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let started = Instant::now();
    while started.elapsed() < STARTUP_TIMEOUT {
        if is_alive() {
            return Ok(());
        }
        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!("clamd exited during startup ({})", status));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    let _ = child.kill();
    Err("clamd did not become ready in time".into())
}

/// Ok(Some(name)) when infected, Ok(None) when clean.
pub fn scan(path: &std::path::Path) -> Result<Option<String>, String> {
    let reply = request(&format!("SCAN {}", path.display()), Duration::from_secs(300))?;
    parse_reply(&reply)
}

fn parse_reply(reply: &str) -> Result<Option<String>, String> {
    // Reply: "<path>: OK", "<path>: <name> FOUND" or "<path>: <reason> ERROR"; paths may contain ": ".
    if reply.ends_with(": OK") {
        return Ok(None);
    }
    if let Some(rest) = reply.strip_suffix(" FOUND") {
        let name = rest.rsplit_once(": ").map(|(_, n)| n).unwrap_or(rest);
        return Ok(Some(name.trim().to_string()));
    }
    Err(reply.to_string())
}

/// Frees the ~1 GB clamd holds; a no-op when it isn't running.
pub fn shutdown() {
    let _ = request("SHUTDOWN", Duration::from_secs(5));
}

#[cfg(test)]
mod tests {
    use super::parse_reply;

    #[test]
    fn parses_clamd_replies() {
        assert_eq!(parse_reply(r"C:\a: b\x.exe: OK"), Ok(None));
        assert_eq!(
            parse_reply(r"C:\a: b\x.exe: Win.Test.EICAR_HDB-1 FOUND"),
            Ok(Some("Win.Test.EICAR_HDB-1".into()))
        );
        assert!(parse_reply(r"C:\x.exe: File path check failure: Unknown error. ERROR").is_err());
        assert!(parse_reply("").is_err());
    }
}
