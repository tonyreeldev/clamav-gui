use tauri::{Emitter, Manager};

use crate::{
    helpers::{
        history::append_scheduler_history,
        log::{initialize_log, log_err, log_info},
        new_id, resolve_command,
        scheduler::{load_scheduler_file, save_scheduler_file},
        silent_command,
    },
    types::{
        enums::{DayOfTheWeek, HistoryDetails, HistoryStatus, HistoryType, LogCategory, ScanType, SchedulerInterval},
        structs::{HistoryItem, SchedulerEntry, SchedulerEvent},
    },
};

pub fn schedule_scan(
    app: &tauri::AppHandle,
    task_name: String,
    time: String,
    task_command: String,
    interval: SchedulerInterval,
    days: DayOfTheWeek,
    scan_type: ScanType,
) -> Result<(), String> {
    if task_name.trim().is_empty() {
        return Err("Task name cannot be empty".into());
    }
    if time.trim().is_empty() {
        return Err("Time cannot be empty".into());
    }
    if task_command.trim().is_empty() {
        return Err("Task command cannot be empty".into());
    }

    let log = initialize_log(app, LogCategory::Scheduler)?;
    let log_id = log.id.clone();
    let log_file = log.file.clone();

    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    let file_path = config_dir.join("scheduler.json");
    let mut scheduler_file = load_scheduler_file(&file_path)?;
    // Checked before schtasks: /f would otherwise silently replace the existing task first.
    if scheduler_file.schedulers.iter().any(|e| e.id == task_name) {
        log_err(&log_file, "Scheduled job already exists");
        return Err("Scheduled job already exists".into());
    }

    let schtasks = resolve_command("schtasks")?;
    let mut cmd = silent_command(&schtasks);
    cmd.args([
        "/create",
        "/tn",
        &format!(r"\{}", task_name),
        "/st",
        &time,
        "/tr",
        &task_command,
        "/f",
    ]);

    match interval {
        SchedulerInterval::Daily => {
            cmd.args(["/sc", "daily"]);
        }
        SchedulerInterval::Weekly => {
            let day = days.as_str().to_ascii_uppercase();
            cmd.args(["/sc", "weekly", "/d", &day]);
        }
        SchedulerInterval::Monthly => {
            cmd.args(["/sc", "monthly"]);
        }
    }

    let status = cmd.status().map_err(|e| e.to_string())?;
    if !status.success() {
        log_err(&log_file, "Failed to create scheduled task");
        return Err("Failed to create scheduled task".into());
    }
    // schtasks can't set these; Windows' defaults skip the scan on battery or if the PC was off at that time.
    if let Err(e) = powershell(&format!(
        "$s = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable; \
         Set-ScheduledTask -TaskName '{}' -Settings $s -ErrorAction Stop | Out-Null",
        task_name
    )) {
        log_err(&log_file, &format!("Could not relax power settings of the task: {}", e));
    }

    let last_run = get_last_run_time(&task_name).unwrap_or(None);

    let entry = SchedulerEntry {
        id: task_name.clone(),
        scan_type,
        interval,
        days,
        time: time.clone(),
        enabled: true,
        last_run: last_run.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        log_id: Some(log_id.clone()),
    };

    scheduler_file.schedulers.push(entry);
    save_scheduler_file(&file_path, &scheduler_file)?;

    if let Err(e) = app.emit(
        "scheduler:created",
        SchedulerEvent {
            id: task_name.clone(),
            scan_type,
            interval,
            days,
            time,
            last_run: last_run.clone(),
            log_id: Some(log_id.clone()),
        },
    ) {
        log_err(
            &log_file,
            &format!("Failed to emit scheduler:created event: {}", e),
        );
    }
    log_info(
        &log_file,
        &format!("New Scheduled Job Created Successfully: {}", task_name),
    );

    append_scheduler_history(
        app,
        HistoryItem {
            id: new_id(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            action: Some(HistoryType::SchedulerCreate),
            details: Some(HistoryDetails::SchedulerCreate { task_name }),
            status: HistoryStatus::Success,
            category: Some(LogCategory::Scheduler),
            log_id: Some(log_id),
            scan_type: None,
            threat_count: None,
            scan_result: None,
        },
        &log_file,
    );

    Ok(())
}

pub fn remove_job(app: &tauri::AppHandle, task_name: String) -> Result<(), String> {
    if task_name.trim().is_empty() {
        return Err("Task name cannot be empty".into());
    }

    let log = initialize_log(app, LogCategory::Scheduler)?;
    let log_id = log.id.clone();
    let log_file = log.file.clone();

    let schtasks = resolve_command("schtasks")?;
    let mut cmd = silent_command(&schtasks);
    cmd.args(["/delete", "/tn", &task_name, "/f"]);

    let status = cmd.status().map_err(|e| e.to_string())?;
    // A task already deleted outside the app must still leave the list.
    let still_exists = || {
        silent_command(&schtasks)
            .args(["/query", "/tn", &task_name])
            .status()
            .map(|s| s.success())
            .unwrap_or(true)
    };
    if !status.success() && still_exists() {
        log_err(&log_file, "Failed to delete scheduled task");
        return Err("Failed to delete scheduled task".into());
    }

    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;

    std::fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    let file_path = config_dir.join("scheduler.json");

    let mut scheduler_file = load_scheduler_file(&file_path)?;

    let before = scheduler_file.schedulers.len();
    scheduler_file
        .schedulers
        .retain(|entry| entry.id != task_name);

    if scheduler_file.schedulers.len() == before {
        log_err(&log_file, "Scheduled job not found in scheduler.json");
        return Err("Scheduled job not found in scheduler.json".into());
    }

    save_scheduler_file(&file_path, &scheduler_file)?;

    if let Err(e) = app.emit("scheduler:deleted", &task_name) {
        log_err(
            &log_file,
            &format!("Failed to emit scheduler:deleted event: {}", e),
        );
    }

    log_info(
        &log_file,
        &format!("Scan Job Deleted Successfully: {}", task_name),
    );

    append_scheduler_history(
        app,
        HistoryItem {
            id: new_id(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            action: Some(HistoryType::SchedulerDelete),
            details: Some(HistoryDetails::SchedulerDelete { task_name }),
            status: HistoryStatus::Success,
            category: Some(LogCategory::Scheduler),
            log_id: Some(log_id),
            scan_type: None,
            threat_count: None,
            scan_result: None,
        },
        &log_file,
    );

    Ok(())
}

pub fn run_job_now(app: &tauri::AppHandle, task_name: String) -> Result<(), String> {
    if task_name.trim().is_empty() {
        return Err("Task name cannot be empty".into());
    }

    let log = initialize_log(app, LogCategory::Scheduler)?;
    let log_id = log.id.clone();
    let log_file = log.file.clone();

    let schtasks = resolve_command("schtasks")?;
    let mut cmd = silent_command(&schtasks);

    cmd.args(["/run", "/tn", &task_name]);

    let status = cmd.status().map_err(|e| e.to_string())?;

    if status.success() {
        log_info(
            &log_file,
            &format!("Scheduled job triggered manually: {}", task_name),
        );

        append_scheduler_history(
            app,
            HistoryItem {
                id: new_id(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                action: Some(HistoryType::SchedulerTrigger),
                details: Some(HistoryDetails::SchedulerTrigger { task_name }),
                status: HistoryStatus::Success,
                category: Some(LogCategory::Scheduler),
                log_id: Some(log_id),
                scan_type: None,
                threat_count: None,
                scan_result: None,
            },
            &log_file,
        );
        Ok(())
    } else {
        log_err(&log_file, "Failed to run scheduled task");

        append_scheduler_history(
            app,
            HistoryItem {
                id: new_id(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                action: Some(HistoryType::SchedulerTrigger),
                details: Some(HistoryDetails::SchedulerTriggerError { task_name }),
                status: HistoryStatus::Error,
                category: Some(LogCategory::Scheduler),
                log_id: Some(log_id),
                scan_type: None,
                threat_count: None,
                scan_result: None,
            },
            &log_file,
        );
        Err("Failed to run scheduled task".into())
    }
}

fn powershell(script: &str) -> Result<String, String> {
    let output = silent_command("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// ISO 8601 so the frontend's `new Date()` can read it; schtasks' CSV used the system language and date format.
pub fn get_last_run_time(task_name: &str) -> Result<Option<String>, String> {
    if task_name.trim().is_empty() {
        return Err("Task name cannot be empty".into());
    }
    // Task names are built by the app ("ClamAV GUI - Main - Weekly"), never with quotes.
    let out = powershell(&format!(
        "$i = Get-ScheduledTaskInfo -TaskName '{}' -ErrorAction Stop; \
         if ($i.LastRunTime -and $i.LastRunTime.Year -gt 2000) {{ $i.LastRunTime.ToUniversalTime().ToString('o') }}",
        task_name.replace('\'', "")
    ))?;
    Ok(if out.is_empty() { None } else { Some(out) })
}
