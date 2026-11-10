$ErrorActionPreference = "Stop"
$auditDirectory = "C:\Users\han\AppData\Local\Orchester-build\activity-source-audit-20261110"
$child = Start-Process -FilePath "cmd.exe" -ArgumentList "/d", "/c", (Join-Path $auditDirectory "wsl-diagnostics.cmd") -WorkingDirectory $auditDirectory -WindowStyle Hidden -PassThru
@{ audit_diagnostic_launcher_pid = $child.Id; observed_at_utc = [DateTime]::UtcNow.ToString("o"); audit_directory = $auditDirectory } | ConvertTo-Json -Compress
