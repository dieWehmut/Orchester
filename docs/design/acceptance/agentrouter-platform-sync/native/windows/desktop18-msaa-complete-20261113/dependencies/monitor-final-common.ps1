$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0
$script:BuildHome = 'C:\Users\han\AppData\Local\Orchester-build'
$script:FinalHome = Join-Path $script:BuildHome 'monitor-final-20261110'
$script:RunnerSource = Join-Path $script:BuildHome 'native-adb-transport\NativeFileRunner.cs'
$script:ScriptNames = @('common', 'freeze', 'launch', 'worker', 'promote', 'selfcheck') | ForEach-Object { 'monitor-final-' + $_ + '.ps1' }
$script:ScriptNames += 'monitor-final-cargo.cmd'
$script:GateNames = @('toolchain', 'helper-contract', 'helper-wsl', 'memory-initialization', 'monitor-process', 'monitor-activity', 'monitor-http', 'remote-cli', 'root-clippy', 'root-fmt', 'cli-build', 'chrome-test', 'desktop-test', 'desktop-clippy', 'desktop-fmt', 'desktop-build')

function Write-JsonFile($Value, [string]$Path) {
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 12) + "`n", $utf8)
}
function File-Hash([string]$Path) { (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() }
function Full-Path([string]$Path) { [IO.Path]::GetFullPath($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Path)).TrimEnd('\') }
function Assert-Below([string]$Path, [string]$Parent) {
    if (-not (Full-Path $Path).StartsWith((Full-Path $Parent) + '\', [StringComparison]::OrdinalIgnoreCase)) { throw "Path must be below $Parent : $Path" }
}
function Assert-CmdPath([string]$Path) {
    # This value is passed to cmd.exe as a quoted literal, never executable text.
    if ($Path -match '["%&|<>^!\r\n]') { throw 'Unsupported command path characters.' }
}
function Quote-Native([string]$Value) {
    if ($Value.Contains('"') -or $Value.EndsWith('\') -or $Value -match '[\r\n]') { throw 'Unsupported native argument.' }
    '"' + $Value + '"'
}
function Directory-Inventory([string]$Directory, [switch]$ExcludeBuildDirectories) {
    $root = Full-Path $Directory
    $result = @()
    $pending = New-Object 'System.Collections.Generic.Stack[string]'
    $pending.Push($root)
    while ($pending.Count -gt 0) {
        foreach ($item in Get-ChildItem -LiteralPath $pending.Pop() -Force) {
            if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw "Reparse point is not a frozen input: $($item.FullName)" }
            if ($item.PSIsContainer) {
                if (-not $ExcludeBuildDirectories -or $item.Name -notin @('target', 'node_modules', '.git')) { $pending.Push($item.FullName) }
            } else {
                $result += [pscustomobject]@{ path = $item.FullName.Substring($root.Length + 1).Replace('\', '/'); sha256 = File-Hash $item.FullName }
            }
        }
    }
    @($result | Sort-Object path -CaseSensitive)
}
function Same-Inventory($Expected, $Actual, [string]$Label) {
    $a = @($Expected | ForEach-Object { $_.sha256 + '  ' + $_.path })
    $b = @($Actual | ForEach-Object { $_.sha256 + '  ' + $_.path })
    if ($a.Count -ne $b.Count -or ($a -join "`n") -cne ($b -join "`n")) { throw "Inventory changed: $Label" }
}
function Source-Inventory([string]$Root) {
    $rootPath = Full-Path $Root
    $result = @()
    $leaves = @('SOURCE_REVISION', '.cargo/config.toml', 'Cargo.toml', 'Cargo.lock', 'LICENSE-APACHE', 'LICENSE-MIT',
        'apps/desktop/src-tauri/Cargo.toml', 'apps/desktop/src-tauri/Cargo.lock', 'apps/desktop/src-tauri/build.rs', 'apps/desktop/src-tauri/tauri.conf.json',
        'SOURCE_EXPORT_AUDIT.json')
    foreach ($relative in $leaves) { $result += [pscustomobject]@{ path = $relative; sha256 = File-Hash (Join-Path $rootPath $relative) } }
    foreach ($relative in @('kisten', 'manifeste', 'apps/desktop/native-chrome', 'apps/desktop/src-tauri/src', 'apps/desktop/src-tauri/icons', 'apps/desktop/src-tauri/capabilities', 'apps/web/dist', 'apps/mobile/dist', 'werkzeug/remote', 'apps/web/src', 'apps/web/test', 'apps/mobile/src', 'packages/design', 'packages/protokoll')) {
        foreach ($file in @(Directory-Inventory (Join-Path $rootPath $relative) -ExcludeBuildDirectories)) { $result += [pscustomobject]@{ path = $relative + '/' + $file.path; sha256 = $file.sha256 } }
    }
    @($result | Sort-Object path -CaseSensitive)
}
function Read-Plan([string]$PlanFile, [string]$ExpectedPlanSha256) {
    $file = Full-Path $PlanFile
    Assert-Below $file $script:FinalHome
    if ((File-Hash $file) -cne $ExpectedPlanSha256.ToLowerInvariant()) { throw 'Pinned plan SHA256 differs.' }
    $plan = Get-Content -Raw -LiteralPath $file | ConvertFrom-Json
    if ($plan.format -cne 'orchester-windows-monitors-final-plan-v3' -or $plan.sourceRevision -notmatch '\A[0-9a-f]{40}\z') { throw 'Unsupported plan or source revision.' }
    if ($plan.apkBinding -cne 'deferred-fresh-native-android-source-receipt' -or $null -ne $plan.apkSha256) { throw 'Producer must defer APK without an old artifact hash.' }
    if ((File-Hash (Join-Path $plan.sourceRoot 'SOURCE_EXPORT_AUDIT.json')) -cne $plan.sourceExportAuditSha256) { throw 'Exact Git source/export receipt changed.' }
    if ((Full-Path $plan.runDirectory) -ine (Split-Path -Parent $file) -or $plan.buildJobs -ne 1) { throw 'Plan directory or build concurrency differs.' }
    Assert-Below $plan.runDirectory $script:FinalHome
    if ((Full-Path $plan.desktopCandidate) -ine (Join-Path $plan.runDirectory 'desktop-candidate') -or (Full-Path $plan.remoteCandidate) -ine (Join-Path $plan.runDirectory 'remote-candidate')) { throw 'Candidate paths differ from the owned run directory.' }
    if ($plan.cliTarget -ine (Join-Path $script:BuildHome '8d6db06\target') -or $plan.desktopTarget -ine (Join-Path $script:BuildHome '8d6db06\apps\desktop\src-tauri\target')) { throw 'Expected Windows target cache paths differ.' }
    if ($plan.timeoutSecondsPerStep -lt 1 -or $plan.timeoutSecondsPerStep -gt 3600 -or $plan.desktopProfile -cne 'debug') { throw 'Build timeout or profile differs.' }
    if ((@($plan.commands) -join ',') -cne ($script:GateNames -join ',')) { throw 'Pinned native gate names differ.' }
    Assert-CmdPath $plan.sourceRoot
    foreach ($fileRecord in @($plan.scripts)) {
        if ($fileRecord.path -notin $script:ScriptNames) { throw 'Unknown script in the frozen plan.' }
        if ((File-Hash (Join-Path $PSScriptRoot $fileRecord.path)) -cne $fileRecord.sha256) { throw "Prepared script changed: $($fileRecord.path)" }
    }
    if (@($plan.scripts).Count -ne $script:ScriptNames.Count -or @($plan.scripts.path | Select-Object -Unique).Count -ne $script:ScriptNames.Count) { throw 'Prepared script inventory is incomplete.' }
    if ((File-Hash $script:RunnerSource) -cne $plan.runnerSha256) { throw 'Native runner source changed.' }
    if (Test-Path -LiteralPath (Join-Path $plan.sourceRoot '.git')) { throw 'Build only a frozen Git export, with no .git file or directory.' }
    if ((Get-Content -Raw -LiteralPath (Join-Path $plan.sourceRoot 'SOURCE_REVISION')).Trim() -cne $plan.sourceRevision) { throw 'SOURCE_REVISION differs.' }
    Same-Inventory $plan.inputs (Source-Inventory $plan.sourceRoot) 'frozen source and assets'
    return $plan
}
function Load-NativeRunner {
    # Reuse the file-backed, restricted-handle runner as source only. This does
    # not use, signal, stop, or communicate with the live Android/V2 helper.
    if (-not ('NativeFileRunner' -as [type])) { Add-Type -Path $script:RunnerSource }
}

function Require-TrueBool($Value,[string]$Label) { if($Value -isnot [bool] -or $Value -cne $true){throw ('Required true boolean: '+$Label)} }
function Require-FalseBool($Value,[string]$Label) { if($Value -isnot [bool] -or $Value -cne $false){throw ('Required false boolean: '+$Label)} }
function Require-ExactInt($Value,[long]$Expected,[string]$Label) { if(($Value -isnot [int] -and $Value -isnot [long]) -or $Value -ne $Expected){throw ('Required exact integer: '+$Label)} }
