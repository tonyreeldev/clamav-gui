use specta::specta;
use std::sync::Mutex;
use sysinfo::{ProcessesToUpdate, System};
use tauri::command;

use crate::types::structs::{SysInfo, SysStats};

#[command(async)]
#[specta(result)]
pub fn get_sys_stats() -> Result<SysStats, String> {
    // Usage is the delta since the previous refresh, so a fresh System each call always read ~0.
    static SYSTEM: Mutex<Option<System>> = Mutex::new(None);
    let mut guard = SYSTEM.lock().unwrap_or_else(|e| e.into_inner());
    let system = guard.get_or_insert_with(System::new_all);

    system.refresh_cpu_all();
    system.refresh_memory();
    system.refresh_processes(ProcessesToUpdate::All, true);

    let cpu_usage = system.cpus().iter().map(|c| c.cpu_usage()).collect();
    let cpu_frequency = system.cpus().iter().map(|c| c.frequency()).collect();

    let ram_used = system.used_memory();
    let ram_total = system.total_memory();

    let mut disk_read_bytes = 0;
    let mut disk_written_bytes = 0;

    for process in system.processes().values() {
        let io = process.disk_usage();
        disk_read_bytes += io.read_bytes;
        disk_written_bytes += io.written_bytes;
    }

    Ok(SysStats {
        cpu_usage,
        cpu_frequency,
        ram_used,
        ram_total,
        disk_read_bytes,
        disk_written_bytes,
    })
}

#[command]
#[specta(result)]
pub fn get_sys_info() -> Result<SysInfo, String> {
    Ok(SysInfo {
        sys_name: System::name(),
        sys_os: System::os_version(),
        sys_host: System::host_name(),
    })
}
