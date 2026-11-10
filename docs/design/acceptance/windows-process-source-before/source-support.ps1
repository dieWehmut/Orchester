param([string]$OutputFile = "process-source-support.json", [int]$WslDeadlineMs = 3000)
$ErrorActionPreference = 'Stop'
$auditRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$known = @{ 'codex.exe' = 'codex'; 'claude.exe' = 'claude'; 'opencode.exe' = 'opencode'; 'deepseek.exe' = 'deepseek' }
$windowsCounts = @{ codex = 0; claude = 0; opencode = 0; deepseek = 0 }
$windowsStatus = 'available'
try {
  foreach ($process in Get-CimInstance Win32_Process -Property Name -ErrorAction Stop) {
    $provider = $known[$process.Name.ToLowerInvariant()]
    if ($provider) { $windowsCounts[$provider]++ }
  }
} catch { $windowsStatus = 'unavailable'; $windowsCounts = $null }

function Read-BoundedWsl([string]$arguments, [string]$name) {
  $outFile = Join-Path $auditRoot ($name + '.stdout')
  $errorFile = Join-Path $auditRoot ($name + '.stderr')
  $child = $null
  $timer = [System.Diagnostics.Stopwatch]::StartNew()
  try {
    $child = Start-Process -FilePath 'wsl.exe' -ArgumentList $arguments -NoNewWindow -RedirectStandardOutput $outFile -RedirectStandardError $errorFile -PassThru
    $child.EnableRaisingEvents = $true
    $ownedHandle = $child.Handle
    if (-not $child.WaitForExit($WslDeadlineMs)) {
      $child.Kill()
      return @{ status = 'timeout'; text = $null; elapsed_ms = $timer.ElapsedMilliseconds; exit_code = $null; stderr_codes = @() }
    }
    $child.Refresh()
    $exitCode = $child.ExitCode
    $errorText = [System.IO.File]::ReadAllText($errorFile)
    $errorCodes = @([regex]::Matches($errorText, '(?:0x[0-9a-fA-F]{8}|Wsl/[A-Za-z0-9/_]+)') | ForEach-Object { $_.Value })
    if ($null -eq $exitCode -or $exitCode -ne 0) { return @{ status = 'unavailable'; text = $null; elapsed_ms = $timer.ElapsedMilliseconds; exit_code = $exitCode; stderr_codes = $errorCodes } }
    return @{ status = 'available'; text = [System.IO.File]::ReadAllText($outFile).Replace([string][char]0, ''); elapsed_ms = $timer.ElapsedMilliseconds; exit_code = $exitCode; stderr_codes = $errorCodes }
  } catch { return @{ status = 'unavailable'; text = $null; elapsed_ms = $timer.ElapsedMilliseconds; exit_code = $null; exception_type = $_.Exception.GetType().Name } }
  finally {
    Remove-Item -LiteralPath $outFile, $errorFile -Force -ErrorAction SilentlyContinue
  }
}

$wslList = Read-BoundedWsl '--list --running --quiet' 'wsl-list'
$wslSources = @()
if ($wslList.status -eq 'available') {
  $distributionIndex = 0
  foreach ($distribution in ($wslList.text -split '[\r\n]+' | ForEach-Object { $_.Trim() } | Where-Object { $_ })) {
    $script = 'for p in /proc/[0-9]*/comm; do IFS= read -r n < "$p" 2>/dev/null || continue; case "$n" in codex|claude|opencode|deepseek) printf ''%s\n'' "$n";; esac; done'
    $arguments = '--distribution "' + $distribution.Replace('"', '\"') + '" --exec sh -c "' + $script.Replace('"', '\"') + '"'
    $sample = Read-BoundedWsl $arguments ('wsl-processes-' + $distributionIndex)
    $counts = $null
    if ($sample.status -eq 'available') {
      $counts = @{ codex = 0; claude = 0; opencode = 0; deepseek = 0 }
      foreach ($name in ($sample.text -split '[\r\n]+' | ForEach-Object { $_.Trim() } | Where-Object { $_ })) {
        if ($counts.ContainsKey($name)) { $counts[$name]++ }
      }
    }
    $wslSources += @{ source = ('WSL ' + $distribution); status = $sample.status; provider_counts = $counts; elapsed_ms = $sample.elapsed_ms; exit_code = $sample.exit_code; stderr_codes = $sample.stderr_codes; exception_type = $sample.exception_type }
    $distributionIndex++
  }
}
@{
  observed_at_utc = [DateTime]::UtcNow.ToString('o')
  windows = @{ source = 'Windows CIM executable names'; status = $windowsStatus; provider_counts = $windowsCounts }
  wsl_enumeration_status = $wslList.status
  wsl_sources = @($wslSources)
  wsl_query_deadline_ms = $WslDeadlineMs
  wsl_enumeration_diagnostics = @{ elapsed_ms = $wslList.elapsed_ms; exit_code = $wslList.exit_code; stderr_codes = $wslList.stderr_codes; exception_type = $wslList.exception_type }
  scope = 'Only existing executable names; no argv, prompts, credentials, agents or model calls'
  role = 'Supplementary source attribution; production scanner separately records aggregated provider counts'
} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $auditRoot $OutputFile) -Encoding UTF8
