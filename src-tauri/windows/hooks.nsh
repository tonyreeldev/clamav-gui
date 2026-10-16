; Runs after the files are removed. The app registers these itself, so the uninstaller must clean them up:
; a Run entry pointing at a deleted exe and scheduled scans that would fail forever.
; Skipped when the installer uninstalls the old version during an upgrade (/UPDATE), so they survive updates.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "ClamAV GUI"
    nsExec::Exec 'powershell -NoProfile -NonInteractive -Command "Get-ScheduledTask -TaskName ''ClamAV GUI - *'' -ErrorAction SilentlyContinue | Unregister-ScheduledTask -Confirm:$$false"'
  ${EndIf}
!macroend
