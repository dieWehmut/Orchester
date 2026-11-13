$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0
$expectedGuiPid = 37324
$expectedGuiCreated = [Int64]134389718070239855
$expectedGuiSha256 = 'e4aa765befa85c7fb053075844be5693685c00677dd62b4cfff4592ec68c1d05'
$expectedGuiImage = 'C:\Users\han\AppData\Local\Orchester-build\monitor-final-20261110\monitors-final21-71def64-20261111\desktop-candidate\orchester-desktop.exe'
$helperRoot = 'C:\Users\han\AppData\Local\Orchester-build\continue-20261111-monitors-final21-71def64\prepared-runtime-worker3-20261113'
$resultDirectory = 'C:\Users\han\AppData\Local\Orchester-build\continuation-native-access-20261113\owned-native-worker-attempt3-3fa131682683515d5d18a4e5\jobs\1c4ff61123a4ffbbe1fba944f6dedd80'
$helperPins = [ordered]@{
    'NativeTrayQuit.cs' = 'bcaafea8a300f153a08657a657d650eb82282f97057d46533e05b290f4b9ce75'
    'invoke-owned-tray-quit.ps1' = '610b484568af1f8b85fa0291ad7aa1a117ed69d7526b189d0f9b3a1ea50550fd'
}
$script:heldCurrent = @{}
$helperStreams = [Collections.Generic.List[IO.Stream]]::new()
$freshHeldGui = $null
$guiNativeExitCode = $null
$normalTrayHelperInvoked = $false
$normalTrayHelperReturned = $false
$stage = 'fresh-output-directory'
$exitCode = 1
$result = [ordered]@{
    schemaVersion = 1
    status = 'fresh-held-original-gui-normal-tray-failed'
    actualTargetHost = 'Windows'
    guiPid = $expectedGuiPid
    guiCreatedFileTimeExact = $expectedGuiCreated.ToString()
    guiBinarySha256 = $expectedGuiSha256
    historicalCreatorHandlesRequired = $false
    observationScope = 'currently observed descendants of the freshly held exact GUI'
    forcedTerminations = 0
    modelCalls = 0
    defaultInstallationChanged = $false
    privateValuesIncluded = $false
}

function Hash-ReadOnlyFile([string] $Path) {
    $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    $algorithm = [Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($algorithm.ComputeHash($stream))).Replace('-', '').ToLowerInvariant() }
    finally { $algorithm.Dispose(); $stream.Dispose() }
}

# The existing tray helper resolves this function in the calling scope.
function Write-JsonFile($Value, [string] $Path) {
    $temporary = $Path + '.tmp-' + [Guid]::NewGuid().ToString('N')
    $backup = $temporary + '.backup'
    $stream = $null
    try {
        $bytes = ([Text.UTF8Encoding]::new($false)).GetBytes((ConvertTo-Json -InputObject $Value -Depth 10))
        $stream = [IO.File]::Open($temporary, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Flush($true)
        $stream.Dispose()
        $stream = $null
        if ([IO.File]::Exists($Path)) { [IO.File]::Replace($temporary, $Path, $backup) }
        else { [IO.File]::Move($temporary, $Path) }
    } finally {
        if ($null -ne $stream) { $stream.Dispose() }
        if ([IO.File]::Exists($temporary)) { [IO.File]::Delete($temporary) }
        if ([IO.File]::Exists($backup)) { [IO.File]::Delete($backup) }
    }
}

function Current-Records {
    foreach ($record in @($script:heldCurrent.Values)) {
        [ordered]@{
            pid = $record.pid
            createdFileTimeExact = $record.created.ToString()
            parentPid = $record.parentPid
            parentCreatedFileTimeExact = $record.parentCreated.ToString()
            image = $record.image
            imageSha256 = $record.sha256
            firstObservedAtUtc = $record.firstObservedAtUtc
            freshHandleStillLive = (-not $record.process.HasExited)
        }
    }
}

function Current-LiveCount {
    $count = 0
    foreach ($record in @($script:heldCurrent.Values)) {
        if (-not $record.process.HasExited) { $count++ }
    }
    return $count
}

function Capture-CurrentDescendants {
    $snapshotStarted = [DateTime]::UtcNow.ToFileTimeUtc()
    # Deliberately do not request process command lines or historical creators.
    $snapshot = @(Get-CimInstance -ClassName Win32_Process -Property ProcessId,ParentProcessId,CreationDate -ErrorAction Stop)
    do {
        $changed = $false
        foreach ($item in $snapshot) {
            $parent = $null
            foreach ($record in @($script:heldCurrent.Values)) {
                if ($record.pid -eq [int]$item.ParentProcessId -and -not $record.process.HasExited) {
                    $parent = $record
                    break
                }
            }
            if ($null -eq $parent) { continue }
            $child = $null
            $retained = $false
            try {
                try { $child = [Diagnostics.Process]::GetProcessById([int]$item.ProcessId) }
                catch [ArgumentException] { continue }
                $childHandle = $child.Handle
                if ($childHandle -eq [IntPtr]::Zero) { throw [InvalidOperationException]::new('current-child-handle-unavailable') }
                if ($child.HasExited) { continue }
                $childCreated = $child.StartTime.ToUniversalTime().ToFileTimeUtc()
                if ($childCreated -lt $parent.created -or $childCreated -ge $snapshotStarted) { continue }
                if ($null -eq $item.CreationDate) { throw [InvalidOperationException]::new('current-child-cim-birth-unavailable') }
                $cimCreated = $item.CreationDate.ToUniversalTime().ToFileTimeUtc()
                if ([Math]::Abs([Int64]($cimCreated - $childCreated)) -gt 9) {
                    throw [InvalidOperationException]::new('current-child-cim-native-birth-differs')
                }
                if ($parent.process.HasExited -or $child.HasExited) { continue }
                $childKey = [string]$child.Id + ':' + $childCreated.ToString()
                if ($script:heldCurrent.ContainsKey($childKey)) { continue }
                $childImage = $child.MainModule.FileName
                $childHash = Hash-ReadOnlyFile $childImage
                if ($child.HasExited) { continue }
                $script:heldCurrent.Add($childKey, @{
                    process = $child
                    handle = $childHandle
                    pid = $child.Id
                    created = $childCreated
                    parentPid = $parent.pid
                    parentCreated = $parent.created
                    image = $childImage
                    sha256 = $childHash
                    firstObservedAtUtc = [DateTime]::UtcNow.ToString('o')
                })
                $retained = $true
                $changed = $true
            } catch {
                if ($null -ne $child -and $child.HasExited) { continue }
                throw
            } finally {
                if ($null -ne $child -and -not $retained) { $child.Dispose() }
            }
        }
    } while ($changed)
}

function Write-PendingObservation([string] $Reason) {
    Write-JsonFile ([ordered]@{
        schemaVersion = 1
        status = 'normal-tray-observation-pending'
        reason = $Reason
        observedAtUtc = [DateTime]::UtcNow.ToString('o')
        observerStillRunning = $true
        normalTrayHelperInvoked = $normalTrayHelperInvoked
        normalTrayHelperReturned = $normalTrayHelperReturned
        actualGuiNativeExitCode = $guiNativeExitCode
        currentHeldLiveCount = (Current-LiveCount)
        currentObservedIdentities = @(Current-Records)
        forcedTerminations = 0
        historicalCreatorHandlesRequired = $false
        modelCalls = 0
    }) (Join-Path $resultDirectory 'fresh-held-original-gui-tray-pending-safe.json')
}

try {
    if ($resultDirectory -cnotmatch '\A[A-Za-z]:\\') { throw [InvalidOperationException]::new('job-output-placeholder-not-expanded') }
    $resultDirectory = [IO.Path]::GetFullPath($resultDirectory)
    $attributes = [IO.File]::GetAttributes($resultDirectory)
    if (($attributes -band [IO.FileAttributes]::Directory) -eq 0 -or ($attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw [IO.IOException]::new('output-directory-not-plain')
    }
    $terminalPath = Join-Path $resultDirectory 'fresh-held-original-gui-normal-tray-terminal-safe.json'
    if ([IO.File]::Exists($terminalPath)) { throw [IO.IOException]::new('result-directory-already-used') }
    $stage = 'pinned-normal-tray-helper'
    foreach ($name in $helperPins.Keys) {
        $path = Join-Path $helperRoot $name
        if (([IO.File]::GetAttributes($path) -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw [IO.IOException]::new('helper-path-not-plain')
        }
        $helperStream = [IO.File]::Open($path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
        $helperStreams.Add($helperStream)
        if ((Hash-ReadOnlyFile $path) -cne $helperPins[$name]) { throw [UnauthorizedAccessException]::new('normal-tray-helper-pin-differs') }
    }
    $stage = 'fresh-held-exact-original-gui'
    $freshHeldGui = [Diagnostics.Process]::GetProcessById($expectedGuiPid)
    $freshGuiHandle = $freshHeldGui.Handle
    if ($freshGuiHandle -eq [IntPtr]::Zero -or $freshHeldGui.HasExited) { throw [InvalidOperationException]::new('original-gui-not-live') }
    $created = $freshHeldGui.StartTime.ToUniversalTime().ToFileTimeUtc()
    $image = $freshHeldGui.MainModule.FileName
    if ($created -ne $expectedGuiCreated -or -not [string]::Equals($image, $expectedGuiImage, [StringComparison]::OrdinalIgnoreCase) -or
        (Hash-ReadOnlyFile $image) -cne $expectedGuiSha256) {
        throw [UnauthorizedAccessException]::new('original-gui-exact-identity-differs')
    }
    $self = [Diagnostics.Process]::GetCurrentProcess()
    try {
        if ($freshHeldGui.SessionId -ne $self.SessionId) { throw [InvalidOperationException]::new('gui-and-worker-session-differ') }
    } finally { $self.Dispose() }
    $rootKey = [string]$expectedGuiPid + ':' + $expectedGuiCreated.ToString()
    $script:heldCurrent.Add($rootKey, @{
        process = $freshHeldGui; handle = $freshGuiHandle; pid = $expectedGuiPid; created = $expectedGuiCreated
        parentPid = 0; parentCreated = [Int64]0; image = $image; sha256 = $expectedGuiSha256
        firstObservedAtUtc = [DateTime]::UtcNow.ToString('o')
    })
    $stage = 'fresh-current-child-observation'
    Capture-CurrentDescendants
    $listeners = @(Get-NetTCPConnection -State Listen -ErrorAction Stop | Where-Object { $_.LocalPort -eq 19322 })
    $owners = @($listeners | ForEach-Object { [int]$_.OwningProcess } | Sort-Object -Unique)
    if ($owners.Count -ne 1 -or @($listeners | Where-Object { $_.LocalAddress -cnotin @('127.0.0.1', '::1') }).Count -ne 0) {
        throw [InvalidOperationException]::new('current-cdp-listener-not-unique-loopback')
    }
    $currentCdpPid = $owners[0]
    $currentCdpRecord = @($script:heldCurrent.Values | Where-Object {
        $_.pid -eq $currentCdpPid -and $_.parentPid -eq $expectedGuiPid -and -not $_.process.HasExited
    })
    if ($currentCdpRecord.Count -ne 1) { throw [InvalidOperationException]::new('current-cdp-owner-not-fresh-held-direct-child') }
    $result.currentCdpOwnerPid = $currentCdpPid
    $result.currentCdpOwnerCreatedFileTimeExact = $currentCdpRecord[0].created.ToString()
    $result.currentObservationStartedAtUtc = [DateTime]::UtcNow.ToString('o')
    Write-JsonFile ([ordered]@{
        schemaVersion = 1; status = 'fresh-held-original-gui-before-normal-tray'
        guiPid = $expectedGuiPid; guiCreatedFileTimeExact = $expectedGuiCreated.ToString()
        guiImageSha256 = $expectedGuiSha256; currentCdpOwnerPid = $currentCdpPid
        currentObservedIdentities = @(Current-Records); historicalCreatorHandlesRequired = $false
        forcedTerminations = 0; modelCalls = 0
    }) (Join-Path $resultDirectory 'fresh-held-original-gui-tray-before-safe.json')
    $stage = 'existing-native-normal-tray-helper'
    $normalTrayHelperInvoked = $true
    & (Join-Path $helperRoot 'invoke-owned-tray-quit.ps1') -Process $freshHeldGui -ExpectedProcessId ([uint32]$expectedGuiPid) -ExpectedCreated $expectedGuiCreated -ExpectedImage $expectedGuiImage -ExpectedSha256 $expectedGuiSha256 -ResultDirectory $resultDirectory
    $normalTrayHelperReturned = $true
    $stage = 'observe-real-gui-exit'
    $pendingAt = [DateTime]::UtcNow.AddSeconds(10)
    while (-not $freshHeldGui.HasExited) {
        Capture-CurrentDescendants
        if ([DateTime]::UtcNow -ge $pendingAt) {
            Write-PendingObservation 'gui-still-live-after-normal-tray-request'
            $pendingAt = [DateTime]::UtcNow.AddSeconds(2)
        }
        [Threading.Thread]::Sleep(500)
    }
    $freshHeldGui.WaitForExit()
    $observedGuiExitCode = $freshHeldGui.ExitCode
    if ($null -eq $observedGuiExitCode -or $observedGuiExitCode -isnot [int]) {
        throw [InvalidOperationException]::new('gui-native-exit-code-unavailable')
    }
    $guiNativeExitCode = $observedGuiExitCode
    $stage = 'observe-current-held-child-natural-exit'
    $pendingAt = [DateTime]::UtcNow.AddSeconds(5)
    while ((Current-LiveCount) -gt 0) {
        Capture-CurrentDescendants
        if ([DateTime]::UtcNow -ge $pendingAt) {
            Write-PendingObservation 'currently-held-child-still-live'
            $pendingAt = [DateTime]::UtcNow.AddSeconds(2)
        }
        [Threading.Thread]::Sleep(500)
    }
    if ($guiNativeExitCode -ne 0) { throw [InvalidOperationException]::new('normal-tray-gui-exit-nonzero') }
    $stage = 'observe-cdp-port-release'
    $portDeadline = [DateTime]::UtcNow.AddSeconds(5)
    do {
        $remainingCdpListeners = @(Get-NetTCPConnection -State Listen -ErrorAction Stop | Where-Object { $_.LocalPort -eq 19322 })
        if ($remainingCdpListeners.Count -eq 0) { break }
        [Threading.Thread]::Sleep(100)
    } while ([DateTime]::UtcNow -lt $portDeadline)
    if ($remainingCdpListeners.Count -ne 0) { throw [InvalidOperationException]::new('original-gui-cdp-port-remains-in-use') }
    $result.status = 'fresh-held-original-gui-normal-tray-exit-observed'
    $result.nativeNormalTrayQuitCompleted = $true
    $result.allCurrentlyObservedHandlesExited = $true
    $result.cdpPort19322Released = $true
    $exitCode = 0
} catch {
    $result.stage = $stage
    $result.exceptionType = $_.Exception.GetBaseException().GetType().FullName
} finally {
    $result.normalTrayHelperInvoked = $normalTrayHelperInvoked
    $result.normalTrayHelperReturned = $normalTrayHelperReturned
    $result.actualGuiNativeExitCode = $guiNativeExitCode
    $result.currentObservedIdentities = @(Current-Records)
    $result.selfReportedExitCode = $exitCode
    $result.selfReportedNotIndependentOsExit = $true
    $result.observedAtUtc = [DateTime]::UtcNow.ToString('o')
    try {
        $result.sourceSha256 = Hash-ReadOnlyFile $PSCommandPath
        Write-JsonFile $result (Join-Path $resultDirectory 'fresh-held-original-gui-normal-tray-terminal-safe.json')
        Write-Output (ConvertTo-Json -InputObject $result -Depth 10 -Compress)
    } finally {
        foreach ($record in @($script:heldCurrent.Values)) { $record.process.Dispose() }
        if ($null -ne $freshHeldGui -and $script:heldCurrent.Count -eq 0) { $freshHeldGui.Dispose() }
        foreach ($stream in $helperStreams) { $stream.Dispose() }
    }
}
exit $exitCode

