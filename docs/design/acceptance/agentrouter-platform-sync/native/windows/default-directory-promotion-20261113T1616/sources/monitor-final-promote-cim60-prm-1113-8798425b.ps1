param(
    [Parameter(Mandatory = $true)][ValidateSet('Desktop', 'Remote')][string]$Kind,
    [Parameter(Mandatory = $true)][string]$PlanFile,
    [Parameter(Mandatory = $true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedPlanSha256,
    [Parameter(Mandatory = $true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedInstalledExeSha256,
    [Parameter(Mandatory = $true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedFinalResultSha256,
    [Parameter(Mandatory = $true)][string]$RuntimeResultFile,
    [Parameter(Mandatory = $true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedRuntimeResultSha256,
    [Parameter(Mandatory = $true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedCleanupResultSha256,
    [Parameter(Mandatory = $true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedRuntimeInputsGuardSha256,
    [Parameter(Mandatory = $true)][string]$RuntimeInputsFile,
    [Parameter(Mandatory = $true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedRuntimeInputsSha256,
    [Parameter(Mandatory = $true)][string]$BackupDirectory
)
. (Join-Path $PSScriptRoot 'monitor-final-common.ps1')
$plan = Read-Plan $PlanFile $ExpectedPlanSha256
$runtimeInputsPath=Full-Path $RuntimeInputsFile
Assert-Below $runtimeInputsPath $plan.runDirectory
if ((File-Hash $runtimeInputsPath) -cne $ExpectedRuntimeInputsSha256) { throw 'Pinned additional runtime inputs changed.' }
$runtimeGuardFile=Join-Path $PSScriptRoot 'prepared-runtime\runtime-input-guards.ps1'
if((File-Hash $runtimeGuardFile) -cne $ExpectedRuntimeInputsGuardSha256){throw 'Reviewed runtime typed guard source changed.'}
. $runtimeGuardFile
$runtimeInputs=Get-Content -Raw -LiteralPath $runtimeInputsPath | ConvertFrom-Json
if ($runtimeInputs.format -cne 'orchester-fresh-android-runtime-inputs-v1' -or $runtimeInputs.status -cne 'verified' -or $runtimeInputs.sourceRevision -cne $plan.sourceRevision -or $runtimeInputs.expectedPlanSha256 -cne $ExpectedPlanSha256 -or $runtimeInputs.expectedFinalResultSha256 -cne $ExpectedFinalResultSha256) { throw 'Additional runtime source binding is incomplete.' }
$androidReceiptFile=Full-Path $runtimeInputs.androidAcceptanceReceiptFile
if ((File-Hash $androidReceiptFile) -cne $runtimeInputs.androidAcceptanceReceiptSha256) { throw 'Reviewed fresh Android receipt changed.' }
$androidReceipt=Get-Content -Raw -LiteralPath $androidReceiptFile | ConvertFrom-Json
if ($androidReceipt.format -cne 'orchester-fresh-native-android-acceptance-v1' -or $androidReceipt.status -cne 'passed' -or $androidReceipt.sourceRevision -cne $plan.sourceRevision -or $androidReceipt.apkSha256 -cne $runtimeInputs.apkSha256 -or -not $androidReceipt.sourceAndDistVerified -or -not $androidReceipt.originalInstalledCertificateMatched -or $androidReceipt.nativeGradleExitCode -ne 0 -or $androidReceipt.nativeInstallExitCode -ne 0 -or -not $androidReceipt.nativeBridgeAccepted -or -not $androidReceipt.nativeUiAccepted) { throw 'Fresh Android native acceptance is incomplete.' }
if ((File-Hash $runtimeInputs.apkFile) -cne $runtimeInputs.apkSha256 -or (Get-Item -LiteralPath $runtimeInputs.apkFile).Length -ne $runtimeInputs.apkBytes) { throw 'Fresh Android actual bytes changed.' }
Same-Inventory $runtimeInputs.mobileDistInventory (Directory-Inventory (Join-Path $plan.sourceRoot 'apps/mobile/dist')) 'fresh Android/mobile distribution binding'
Same-Inventory $androidReceipt.mobileDistInventory $runtimeInputs.mobileDistInventory 'Android native receipt distribution'

$resultFile = Join-Path $plan.runDirectory 'final-result.json'
if ((File-Hash $resultFile) -cne $ExpectedFinalResultSha256) { throw 'Reviewed final result changed.' }
$result = Get-Content -Raw -LiteralPath $resultFile | ConvertFrom-Json
if ($result.status -cne 'build-and-file-gates-passed' -or $result.sourceRevision -cne $plan.sourceRevision -or $result.expectedPlanSha256 -cne $ExpectedPlanSha256 -or (@($result.gates.phase) -join ',') -cne (@($plan.commands) -join ',') -or @($result.gates | Where-Object { $_.exitCode -ne 0 -or $_.timedOut -or -not $_.processExited }).Count -ne 0) { throw 'Final build gates are incomplete.' }
if($runtimeInputs.runtimeInputsGuardSha256 -cne $ExpectedRuntimeInputsGuardSha256){throw 'Runtime inputs guard is not source-pinned.'}
$runtimeInputs=Read-FreshRuntimeInputs $plan $result $ExpectedPlanSha256 $ExpectedFinalResultSha256 $RuntimeInputsFile $ExpectedRuntimeInputsSha256
foreach($gate in @($result.gates)){Require-ExactInt $gate.exitCode 0 'gate.exitCode';Require-FalseBool $gate.timedOut 'gate.timedOut';Require-TrueBool $gate.processExited 'gate.processExited'}
$runtimeFile = Full-Path $RuntimeResultFile
Assert-Below $runtimeFile $plan.runDirectory
if ((File-Hash $runtimeFile) -cne $ExpectedRuntimeResultSha256) { throw 'Reviewed runtime result changed.' }
$runtime = Get-Content -Raw -LiteralPath $runtimeFile | ConvertFrom-Json
$cleanupFile = Join-Path (Split-Path -Parent $runtimeFile) 'cleanup-result.json'
if ((File-Hash $cleanupFile) -cne $ExpectedCleanupResultSha256) { throw 'Reviewed runtime cleanup changed.' }
$cleanup = Get-Content -Raw -LiteralPath $cleanupFile | ConvertFrom-Json
if ($runtime.expectedRuntimeInputsSha256 -cne $ExpectedRuntimeInputsSha256 -or $runtime.expectedPlanSha256 -cne $ExpectedPlanSha256 -or $runtime.expectedFinalResultSha256 -cne $ExpectedFinalResultSha256 -or -not $runtime.exactOwnedHandlesExited -or -not $runtime.portsReleased) { throw 'Runtime source and exit proof are incomplete.' }
Require-TrueBool $runtime.exactOwnedHandlesExited 'runtime.owned-exit'
Require-TrueBool $runtime.portsReleased 'runtime.portsReleased'
if ($Kind -eq 'Desktop') {
    Require-TrueBool $runtime.nativeTrayQuitNatural 'runtime.nativeTrayQuitNatural'
    Require-TrueBool $cleanup.nativeTrayQuitNatural 'cleanup.nativeTrayQuitNatural'
    Require-TrueBool $cleanup.allExactGuiHandlesExited 'cleanup.gui-exit'
    Require-TrueBool $cleanup.allExactNodeHandlesExited 'cleanup.node-exit'
    Require-TrueBool $cleanup.debugPortReleased 'cleanup.portsReleased'
    Require-ExactInt $cleanup.unexpectedDescendants 0 'cleanup.unexpectedDescendants'
    Require-ExactInt $runtime.checks 18 'runtime.checks'
    Require-ExactInt $runtime.writes 7 'runtime.writes'
    Require-ExactInt $runtime.screenshotsReviewed 2 'runtime.screenshotsReviewed'
    $nativeFile=Join-Path (Split-Path -Parent $runtimeFile) 'native-windows-result.json'
    if ((File-Hash $nativeFile) -cne $runtime.nativeWindowsResultSha256) { throw 'Pinned native monitor proof changed.' }
    $native=Get-Content -Raw -LiteralPath $nativeFile | ConvertFrom-Json
    Require-TrueBool $native.passed 'native.passed'
    $providerIds=@('codex','claude','deepseek','opencode','gemini','antigravity','cursor','workbuddy','workbuddy_global','qwen','openclaw','cline','roo','kilo','hermes','pi','crush','claude_app','trae','zcode','craft')
    $serviceIds=@('openclaw','workbuddy','workbuddy_global','claude_app','trae','zcode','craft')
    if (-not $native.passed -or $native.sourceRevision -cne $plan.sourceRevision -or $native.expectedPlanSha256 -cne $ExpectedPlanSha256 -or $native.expectedFinalResultSha256 -cne $ExpectedFinalResultSha256 -or $native.binarySha256 -cne $result.desktopExeSha256 -or @($native.checks).Count -ne 18 -or @($native.writes).Count -ne 7 -or @($native.pageErrors).Count -ne 0 -or (@($native.monitorProof.roster.id) -join ',') -cne ($providerIds -join ',') -or (@($native.monitorProof.units.id) -join ',') -cne ($providerIds -join ',') -or (@($native.monitorProof.sharedEditors.id) -join ',') -cne 'cline,roo,kilo') { throw 'Native twenty-one-provider acceptance is incomplete.' }
    foreach($provider in @($native.monitorProof.roster)) {
        Require-TrueBool $provider.officialSourceBoundByBuildInputs 'provider.officialSource'
        Require-TrueBool $provider.loaded 'provider.loaded'
        Require-TrueBool $provider.darkAndLightSwitchObserved 'provider.darkAndLightSwitchObserved'
        Require-ExactInt $provider.themeImagesLoaded 2 'provider.themeImagesLoaded'
        Require-PositiveInt $provider.instanceCount 'provider.instanceCount'
    }
    foreach($unit in @($native.monitorProof.units)) {
        $expectedUnit=if($unit.id -in $serviceIds){'services'}else{'processes'}
        $roster=@($native.monitorProof.roster | Where-Object {$_.id -ceq $unit.id})
        if($roster.Count -ne 1) { throw 'Native unit has no unique matching provider roster.' }
        Require-NativeProviderInstances $unit.id $roster[0] $unit $expectedUnit
    }
    foreach($editor in @($native.monitorProof.sharedEditors)) {
        Require-TrueBool $editor.unavailableSourceVisible 'sharedEditor.unavailableSource'
        if(-not $editor.unavailableSourceVisible -or $editor.countKind -notin @('unknown','at-least-positive')) { throw 'Shared editor sources must remain incomplete.' }
    }
    if (-not $runtime.nativeTrayQuitNatural -or -not $cleanup.nativeTrayQuitNatural -or @($cleanup.guiOutcomes).Count -ne 0 -or @($cleanup.nodeOutcomes).Count -ne 0) { throw 'Desktop native tray quit must exit without forced cleanup.' }
    $trayFile=Join-Path (Split-Path -Parent $runtimeFile) 'tray-quit-result.json'
    if ((File-Hash $trayFile) -cne $runtime.trayQuitResultSha256) { throw 'Pinned native tray quit result changed.' }
    $tray=Get-Content -Raw -LiteralPath $trayFile | ConvertFrom-Json
    foreach($field in @('allExactGuiHandlesExited','allExactNodeHandlesExited','debugAndEmbeddedPortsReleased')){Require-TrueBool $tray.$field ('tray.'+$field)}
    foreach($field in @('guiExitCode','forcedTerminations','unexpectedDescendants')){Require-ExactInt $tray.$field 0 ('tray.'+$field)}
    Require-ExactInt $tray.passiveWaitSeconds 5 'tray.passiveWaitSeconds'
    if ($tray.event -cne 'owned_native_tray_quit_verified' -or $tray.sourceRevision -cne $plan.sourceRevision -or $tray.expectedPlanSha256 -cne $ExpectedPlanSha256 -or $tray.expectedFinalResultSha256 -cne $ExpectedFinalResultSha256 -or $tray.binarySha256 -cne $result.desktopExeSha256 -or $tray.guiExitCode -ne 0 -or -not $tray.allExactGuiHandlesExited -or -not $tray.allExactNodeHandlesExited -or -not $tray.debugAndEmbeddedPortsReleased -or $tray.forcedTerminations -ne 0 -or $tray.unexpectedDescendants -ne 0 -or $tray.passiveWaitSeconds -ne 5) { throw 'Natural native tray quit acceptance is incomplete.' }
    if ((Split-Path -Leaf $runtimeFile) -cne 'desktop-runtime-result.json' -or $runtime.status -cne 'desktop-native-eighteen-checks-passed' -or $runtime.sourceRevision -cne $plan.sourceRevision -or $runtime.binarySha256 -cne $result.desktopExeSha256 -or $runtime.checks -ne 18 -or $runtime.writes -ne 7 -or $runtime.screenshotsReviewed -ne 2 -or $cleanup.status -cne 'exact-owned-cleanup-passed' -or -not $cleanup.allExactGuiHandlesExited -or -not $cleanup.allExactNodeHandlesExited -or -not $cleanup.debugPortReleased -or $cleanup.unexpectedDescendants -ne 0) { throw 'Desktop native runtime acceptance is incomplete.' }
} else {
    Require-ExactInt $runtime.backend_exit 0 'portable.backend_exit'
    Require-TrueBool $cleanup.allExactOwnedHandlesExited 'portable.owned-exit'
    Require-ExactInt $cleanup.unexpectedDescendants 0 'portable.unexpectedDescendants'
    $portableChecks = @('packaged_launcher','html_matches_package','unauthenticated_401','one_time_pairing','project_add_list','selected_unicode_space_cwd','windows_conpty_launch','set_p_interactive_input','ordered_websocket_output','resize_96x31','delete_drained_exit','shutdown_unconnected_terminal','backend_exited','port_released')
    if ((Split-Path -Leaf $runtimeFile) -cne 'runtime-result.json' -or $runtime.event -cne 'portable_runtime_verified' -or $runtime.source_revision -cne $plan.sourceRevision -or $runtime.binary_sha256 -cne $result.remoteExeSha256 -or $runtime.apk_sha256 -cne $runtimeInputs.apkSha256 -or (@($runtime.checks) -join ',') -cne ($portableChecks -join ',') -or $runtime.backend_exit -ne 0 -or -not $cleanup.allExactOwnedHandlesExited -or $cleanup.unexpectedDescendants -ne 0 -or @($cleanup.fallbackOutcomes).Count -ne 0) { throw 'Portable native runtime acceptance is incomplete.' }
}
$dest = if ($Kind -eq 'Desktop') { 'C:\Users\han\AppData\Local\Orchester' } else { 'C:\Users\han\Downloads\OrchesterRemote' }
$candidate = if ($Kind -eq 'Desktop') { $plan.desktopCandidate } else { $runtimeInputs.remotePackageCandidate }
$name = if ($Kind -eq 'Desktop') { 'orchester-desktop.exe' } else { 'orchester.exe' }
$expectedInventory = if ($Kind -eq 'Desktop') { $result.desktopInventory } else { $runtimeInputs.remotePackageInventory }
Assert-Below $candidate $plan.runDirectory
if ($Kind -eq 'Remote') {
    if ((File-Hash (Join-Path $candidate 'orchester.exe')) -cne $result.remoteExeSha256 -or (File-Hash (Join-Path $candidate 'orchester-mobile-debug.apk')) -cne $runtimeInputs.apkSha256) { throw 'Derived Remote package EXE or APK changed.' }
    Same-Inventory (Directory-Inventory (Join-Path $plan.remoteCandidate 'web')) (Directory-Inventory (Join-Path $candidate 'web')) 'immutable producer mobile Web'
}
Same-Inventory $expectedInventory (Directory-Inventory $candidate) 'reviewed candidate'
$exe = Join-Path $dest $name
if ((File-Hash $exe) -cne $ExpectedInstalledExeSha256) { throw 'Installed executable changed since review.' }
$running = @(Get-CimInstance Win32_Process -Filter ("Name='" + $name + "'") -OperationTimeoutSec 60 | Where-Object { $_.ExecutablePath -ieq $exe })
if ($running.Count -gt 0) { throw 'The original user process must exit normally before promotion; this script does not stop it.' }
$backup = Full-Path $BackupDirectory
if ((Split-Path -Parent $backup) -ine (Split-Path -Parent $dest)) { throw 'Backup must be a new sibling of the default installation.' }
if (Test-Path -LiteralPath $backup) { throw 'Backup already exists; nothing replaced.' }
$stateRoot = if ($Kind -eq 'Remote') { 'C:\Users\han\AppData\Local\OrchesterRemote\state-home' } else { 'C:\Users\han\.orchester' }
$stateBefore = if (Test-Path -LiteralPath $stateRoot) { @(Directory-Inventory $stateRoot) } else { @() }
$before = @(Directory-Inventory $dest)
$stage = Join-Path (Split-Path -Parent $dest) ('.orchester-monitor-final-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $stage | Out-Null
$movedOld = $false
$movedNew = $false
try {
    if ($Kind -eq 'Desktop') {
        foreach ($item in Get-ChildItem -LiteralPath $dest -Force) { Copy-Item -LiteralPath $item.FullName -Destination $stage -Recurse }
        Remove-Item -LiteralPath (Join-Path $stage 'web') -Recurse -Force
        Copy-Item -LiteralPath (Join-Path $candidate $name) -Destination (Join-Path $stage $name) -Force
        Copy-Item -LiteralPath (Join-Path $candidate 'web') -Destination (Join-Path $stage 'web') -Recurse
        $outsideBefore = @($before | Where-Object { $_.path -cne $name -and -not $_.path.StartsWith('web/', [StringComparison]::Ordinal) })
        $outsideStage = @(Directory-Inventory $stage | Where-Object { $_.path -cne $name -and -not $_.path.StartsWith('web/', [StringComparison]::Ordinal) })
        Same-Inventory $outsideBefore $outsideStage 'desktop uninstaller and other installed files'
        if ((File-Hash (Join-Path $stage $name)) -cne $result.desktopExeSha256) { throw 'Staged desktop executable differs.' }
        Same-Inventory (Directory-Inventory (Join-Path $candidate 'web')) (Directory-Inventory (Join-Path $stage 'web')) 'staged desktop Web'
    } else {
        foreach ($item in Get-ChildItem -LiteralPath $candidate -Force) { Copy-Item -LiteralPath $item.FullName -Destination $stage -Recurse }
        & (Join-Path $stage 'verify.ps1') -PackageDirectory $stage | Out-Null
        Same-Inventory $expectedInventory (Directory-Inventory $stage) 'staged Remote package'
    }
    # Check again after staging, before moving the original directory.
    if (@(Get-CimInstance Win32_Process -Filter ("Name='" + $name + "'") -OperationTimeoutSec 60 | Where-Object { $_.ExecutablePath -ieq $exe }).Count -gt 0) { throw 'User process restarted during staging; aborting.' }
    Same-Inventory $before (Directory-Inventory $dest) 'original installation before backup'
    [IO.Directory]::Move($dest, $backup)
    $movedOld = $true
    [IO.Directory]::Move($stage, $dest)
    $movedNew = $true
    Same-Inventory $before (Directory-Inventory $backup) 'complete preserved installation backup'
    $after = @(Directory-Inventory $dest)
    if ($Kind -eq 'Remote') { Same-Inventory $expectedInventory $after 'promoted Remote package' }
    else {
        if ((File-Hash (Join-Path $dest $name)) -cne $result.desktopExeSha256) { throw 'Promoted desktop executable differs.' }
        Same-Inventory (Directory-Inventory (Join-Path $candidate 'web')) (Directory-Inventory (Join-Path $dest 'web')) 'promoted desktop Web'
        Same-Inventory $outsideBefore @($after | Where-Object { $_.path -cne $name -and -not $_.path.StartsWith('web/', [StringComparison]::Ordinal) }) 'preserved installed files'
    }
    $stateAfter = if (Test-Path -LiteralPath $stateRoot) { @(Directory-Inventory $stateRoot) } else { @() }
    Same-Inventory $stateBefore $stateAfter 'external user state'
    Write-JsonFile ([ordered]@{kind = $Kind; promoted = $true; destination = $dest; backup = $backup; sourceRevision = $plan.sourceRevision; expectedPlanSha256 = $ExpectedPlanSha256; expectedRuntimeInputsSha256 = $ExpectedRuntimeInputsSha256; freshAndroidAcceptanceReceiptSha256 = $runtimeInputs.androidAcceptanceReceiptSha256; previousExeSha256 = $ExpectedInstalledExeSha256; currentExeSha256 = File-Hash $exe; oldDirectoryPreserved = $true; externalStateRoot = $stateRoot; externalStateFiles = $stateBefore.Count; externalStateUnchanged = $true; uninstallerAndOtherDesktopFilesPreserved = ($Kind -eq 'Desktop'); userProcessStoppedByScript = $false; userProcessRestartedByScript = $false; runtimeAndTaskRetentionValidation = 'pending-root'; modelCalls = 0}) (Join-Path $plan.runDirectory ('promotion-' + $Kind.ToLowerInvariant() + '.json'))
} catch {
    # Roll back only directories moved by this invocation, retaining the failed
    # candidate for inspection. No process, registry, shortcut, or state mutation.
    if ($movedNew) { [IO.Directory]::Move($dest, $stage) }
    if ($movedOld -and -not (Test-Path -LiteralPath $dest)) { [IO.Directory]::Move($backup, $dest) }
    throw
} finally {
    if (-not $movedOld -and (Test-Path -LiteralPath $stage)) { Remove-Item -LiteralPath $stage -Recurse -Force }
}
