param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[a-f0-9]{32}$')]
    [string] $BootstrapNonce
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$root = [IO.Path]::GetFullPath($PSScriptRoot)
$stage = 'initializing'
$lockStream = $null
$selfProcess = $null
$capabilityKey = $null
$activeJobs = @{}
$workerIdentity = $null
$exitReason = 'failed'
$exitCode = 1
$failureType = $null
$failureStage = $null
$clock = [Diagnostics.Stopwatch]::StartNew()
$heartbeatAt = -2000L
$scanAt = -500L

function Assert-PlainPath([string] $Path, [bool] $Directory) {
    $attributes = [IO.File]::GetAttributes($Path)
    if (($attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw [IO.IOException]::new('Reparse paths are not allowed.')
    }
    if ((($attributes -band [IO.FileAttributes]::Directory) -ne 0) -ne $Directory) {
        throw [IO.IOException]::new('Unexpected path kind.')
    }
}

function Set-OwnedRootAcl {
    $sid = [Security.Principal.WindowsIdentity]::GetCurrent().User
    $security = [Security.AccessControl.DirectorySecurity]::new()
    $security.SetOwner($sid)
    $security.SetAccessRuleProtection($true, $false)
    $inheritance = [Security.AccessControl.InheritanceFlags]'ContainerInherit, ObjectInherit'
    $allowed = @($sid.Value, 'S-1-5-18', 'S-1-5-32-544') | Select-Object -Unique
    foreach ($identity in $allowed) {
        $principal = [Security.Principal.SecurityIdentifier]::new($identity)
        $rule = [Security.AccessControl.FileSystemAccessRule]::new($principal,
            [Security.AccessControl.FileSystemRights]::FullControl, $inheritance,
            [Security.AccessControl.PropagationFlags]::None,
            [Security.AccessControl.AccessControlType]::Allow)
        [void] $security.AddAccessRule($rule)
    }
    ([IO.DirectoryInfo]::new($root)).SetAccessControl($security)
    $actual = ([IO.DirectoryInfo]::new($root)).GetAccessControl()
    if (-not $actual.AreAccessRulesProtected -or
        $actual.GetOwner([Security.Principal.SecurityIdentifier]).Value -ne $sid.Value) {
        throw [UnauthorizedAccessException]::new('Owned root ACL verification failed.')
    }
    $rules = @($actual.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier]))
    if ($rules.Count -ne $allowed.Count) {
        throw [UnauthorizedAccessException]::new('Unexpected owned root ACL entries.')
    }
    foreach ($rule in $rules) {
        if ($allowed -notcontains $rule.IdentityReference.Value -or $rule.IsInherited -or
            $rule.AccessControlType -ne [Security.AccessControl.AccessControlType]::Allow -or
            $rule.FileSystemRights -ne [Security.AccessControl.FileSystemRights]::FullControl -or
            $rule.InheritanceFlags -ne $inheritance -or
            $rule.PropagationFlags -ne [Security.AccessControl.PropagationFlags]::None) {
            throw [UnauthorizedAccessException]::new('Unexpected owned root ACL rule.')
        }
    }
}

function Write-SafeJson([string] $Path, $Value, [bool] $CreateOnly = $false) {
    $temporary = $Path + '.tmp-' + [Guid]::NewGuid().ToString('N')
    $backup = $temporary + '.backup'
    $stream = $null
    try {
        $bytes = ([Text.UTF8Encoding]::new($false)).GetBytes(
            (ConvertTo-Json -InputObject $Value -Depth 8 -Compress))
        $stream = [IO.File]::Open($temporary, [IO.FileMode]::CreateNew,
            [IO.FileAccess]::Write, [IO.FileShare]::None)
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Flush($true)
        $stream.Dispose()
        $stream = $null
        if ($CreateOnly -or -not [IO.File]::Exists($Path)) {
            [IO.File]::Move($temporary, $Path)
        } else {
            [IO.File]::Replace($temporary, $Path, $backup)
        }
    } finally {
        if ($null -ne $stream) { $stream.Dispose() }
        if ([IO.File]::Exists($temporary)) { [IO.File]::Delete($temporary) }
        if ([IO.File]::Exists($backup)) { [IO.File]::Delete($backup) }
    }
}

function Get-StreamSha256([IO.Stream] $Stream) {
    $algorithm = [Security.Cryptography.SHA256]::Create()
    try {
        $Stream.Position = 0
        return ([BitConverter]::ToString($algorithm.ComputeHash($Stream))).Replace('-', '').ToLowerInvariant()
    } finally {
        $algorithm.Dispose()
        $Stream.Position = 0
    }
}

function Get-FileSha256([string] $Path) {
    $stream = [IO.File]::Open($Path, [IO.FileMode]::Open,
        [IO.FileAccess]::Read, [IO.FileShare]::Read)
    try { return Get-StreamSha256 $stream } finally { $stream.Dispose() }
}

function Get-ExceptionCode($Exception, [int] $Fallback) {
    $cursor = $Exception
    while ($null -ne $cursor) {
        if ($cursor -is [ComponentModel.Win32Exception] -and $cursor.NativeErrorCode -ne 0) {
            return [int] $cursor.NativeErrorCode
        }
        $cursor = $cursor.InnerException
    }
    return $Fallback
}

function Write-Heartbeat {
    $value = [ordered]@{
        schema = 1; state = 'running'; pid = $workerIdentity.pid
        exactBirthUtc = $workerIdentity.exactBirthUtc; source = $workerIdentity.source
        sourceSha256 = $workerIdentity.sourceSha256; nonce = $BootstrapNonce
        timeUtc = [DateTime]::UtcNow.ToString('o'); liveJobs = $activeJobs.Count
        acceptingRequests = ($clock.Elapsed.TotalMinutes -lt 90 -and $null -eq $failureType)
    }
    Write-SafeJson (Join-Path $root 'worker.heartbeat-safe.json') $value
    $script:heartbeatAt = $clock.ElapsedMilliseconds
}

function Observe-Jobs {
    foreach ($jobId in @($activeJobs.Keys)) {
        $job = $activeJobs[$jobId]
        try {
            if ($null -eq $job.handle) { $job.handle = $job.process.Handle }
            if ($null -eq $job.pid) { $job.pid = $job.process.Id }
            if ($null -eq $job.exactBirthUtc) {
                $job.exactBirthUtc = $job.process.StartTime.ToUniversalTime().ToString('o')
            }
        } catch {
            $job.metadataExceptionType = $_.Exception.GetType().FullName
        }
        if ($null -ne $job.handle -and $null -ne $job.pid -and $null -ne $job.exactBirthUtc -and
            -not $job.startRecorded) {
            Write-JobStart $jobId $job
        }
        if ($null -eq $job.exitCode) {
            if (-not $job.process.HasExited) { continue }
            $job.process.WaitForExit()
            $job.exitCode = [int] $job.process.ExitCode
            $job.finishedAtUtc = [DateTime]::UtcNow.ToString('o')
        }
        if ($null -eq $job.handle -or $null -eq $job.pid -or $null -eq $job.exactBirthUtc) {
            continue
        }
        $terminal = [ordered]@{
            schema = 1; id = $jobId; state = 'exited'; pid = $job.pid
            exactBirthUtc = $job.exactBirthUtc; source = $job.source
            sourceSha256 = $job.sourceSha256; nonce = $BootstrapNonce
            startedAtUtc = $job.startedAtUtc; finishedAtUtc = $job.finishedAtUtc
            nativeExitCode = $job.exitCode
            initialObservationExceptionType = $job.metadataExceptionType
        }
        # A failed terminal write keeps the real process handle for a later retry.
        Write-SafeJson (Join-Path $job.directory 'terminal-safe.json') $terminal $true
        try { $job.process.Dispose() }
        finally {
            try { $job.sourceStream.Dispose() }
            finally { $activeJobs.Remove($jobId) }
        }
    }
}

function Write-JobStart([string] $Id, $Job) {
    Write-SafeJson (Join-Path $Job.directory 'start-safe.json') ([ordered]@{
        schema = 1; id = $Id; state = 'started'; pid = $Job.pid
        exactBirthUtc = $Job.exactBirthUtc; source = $Job.source; sourceSha256 = $Job.sourceSha256
        nonce = $BootstrapNonce; startedAtUtc = $Job.startedAtUtc
        initialObservationExceptionType = $Job.metadataExceptionType
    }) $true
    $Job.startRecorded = $true
}

function Read-AuthenticatedRequest($Item) {
    $requestStream = $null
    $sourceStream = $null
    $requestStage = 'request_path'
    try {
        Assert-PlainPath $requestsDirectory $true
        Assert-PlainPath $sourcesDirectory $true
        Assert-PlainPath $Item.FullName $false
        $requestStage = 'request_read'
        $requestStream = [IO.File]::Open($Item.FullName, [IO.FileMode]::Open,
            [IO.FileAccess]::Read, [IO.FileShare]::Read)
        if ($requestStream.Length -gt 16384) { throw [IO.InvalidDataException]::new('Request too large.') }
        $reader = [IO.StreamReader]::new($requestStream, [Text.UTF8Encoding]::new($false, $true), $true, 1024, $true)
        try { $request = ConvertFrom-Json -InputObject $reader.ReadToEnd() }
        finally { $reader.Dispose() }
        $requestStage = 'request_shape'
        foreach ($field in @('id', 'operation', 'filename', 'sourceSha256', 'expiresAtUtc', 'mac')) {
            if ($null -eq $request.PSObject.Properties[$field] -or $request.$field -isnot [string]) {
                throw [IO.InvalidDataException]::new('Invalid request fields.')
            }
        }
        if ($request.id -cnotmatch '^[a-f0-9]{32}$' -or
            $request.operation -cnotmatch '^(run|stop)$' -or
            $request.filename -cne ($request.id + '.ps1') -or
            $Item.Name -cne ('request-' + $request.id + '.json') -or
            $request.sourceSha256 -cnotmatch '^[a-f0-9]{64}$' -or
            $request.mac -cnotmatch '^[a-f0-9]{64}$' -or
            $request.expiresAtUtc.Length -gt 64) {
            throw [IO.InvalidDataException]::new('Invalid request values.')
        }
        $requestStage = 'request_expiry'
        $expires = [DateTimeOffset]::MinValue
        if ($request.expiresAtUtc -cnotmatch '^\d{4}-\d{2}-\d{2}T.+(?:Z|\+00:00)$' -or
            -not [DateTimeOffset]::TryParse($request.expiresAtUtc,
                [Globalization.CultureInfo]::InvariantCulture,
                [Globalization.DateTimeStyles]::None, [ref] $expires) -or
            $expires.Offset -ne [TimeSpan]::Zero -or
            $expires.UtcDateTime -le [DateTime]::UtcNow -or
            $expires.UtcDateTime -gt [DateTime]::UtcNow.AddMinutes(15)) {
            throw [IO.InvalidDataException]::new('Invalid request expiry.')
        }
        $requestStage = 'request_mac'
        $canonical = $request.id + "`n" + $request.operation + "`n" + $request.filename + "`n" +
            $request.sourceSha256 + "`n" + $request.expiresAtUtc
        $hmac = [Security.Cryptography.HMACSHA256]::new()
        try {
            $hmac.Key = $capabilityKey
            $expectedMac = $hmac.ComputeHash(([Text.UTF8Encoding]::new($false)).GetBytes($canonical))
        } finally { $hmac.Dispose() }
        $difference = 0
        for ($index = 0; $index -lt 32; $index++) {
            $provided = [Convert]::ToByte($request.mac.Substring($index * 2, 2), 16)
            $difference = $difference -bor ($expectedMac[$index] -bxor $provided)
        }
        if ($difference -ne 0) { throw [UnauthorizedAccessException]::new('Request MAC mismatch.') }
        $requestStage = 'request_source'
        $sourcePath = [IO.Path]::GetFullPath((Join-Path $sourcesDirectory $request.filename))
        if (-not [string]::Equals([IO.Path]::GetDirectoryName($sourcePath), $sourcesDirectory,
                [StringComparison]::OrdinalIgnoreCase)) {
            throw [UnauthorizedAccessException]::new('Source outside owned directory.')
        }
        Assert-PlainPath $sourcePath $false
        $sourceStream = [IO.File]::Open($sourcePath, [IO.FileMode]::Open,
            [IO.FileAccess]::Read, [IO.FileShare]::Read)
        if ((Get-StreamSha256 $sourceStream) -cne $request.sourceSha256) {
            throw [UnauthorizedAccessException]::new('Source hash mismatch.')
        }
        if ($expires.UtcDateTime -le [DateTime]::UtcNow) {
            throw [IO.InvalidDataException]::new('Request expired before claim.')
        }
        # The permanent CreateNew destination is the replay ledger. No cleanup removes it.
        $requestStage = 'request_claim'
        $claimPath = Join-Path $requestsDirectory ('claimed-' + $request.id + '.json')
        $requestStream.Dispose()
        $requestStream = $null
        [IO.File]::Move($Item.FullName, $claimPath)
        $accepted = @{ request = $request; sourcePath = $sourcePath; sourceStream = $sourceStream }
        $sourceStream = $null
        return $accepted
    } catch {
        $_.Exception.Data['safeStage'] = $requestStage
        throw
    } finally {
        if ($null -ne $requestStream) { $requestStream.Dispose() }
        if ($null -ne $sourceStream) { $sourceStream.Dispose() }
    }
}

function Reject-Request($Item, $Exception) {
    $nameId = $Item.Name.Substring(8, 32)
    $safeStage = $Exception.Data['safeStage']
    if ($null -eq $safeStage) { $safeStage = 'request_processing' }
    Write-SafeJson (Join-Path $requestsDirectory ('rejection-' + $nameId + '-safe.json')) ([ordered]@{
        schema = 1; id = $nameId; state = 'rejected'; stage = $safeStage
        exceptionType = $Exception.GetType().FullName; timeUtc = [DateTime]::UtcNow.ToString('o')
        nonce = $BootstrapNonce
    })
    if ([IO.File]::Exists($Item.FullName)) {
        [IO.File]::Move($Item.FullName, (Join-Path $requestsDirectory ('rejected-' + $nameId + '-' + [Guid]::NewGuid().ToString('N') + '.json')))
    }
}

function Start-OwnedJob($Accepted) {
    $request = $Accepted.request
    $jobDirectory = Join-Path $jobsDirectory $request.id
    $process = $null
    $jobStage = 'job_directory'
    $registered = $false
    try {
        Assert-PlainPath $jobsDirectory $true
        if ([IO.Directory]::Exists($jobDirectory) -or [IO.File]::Exists($jobDirectory)) {
            throw [IO.IOException]::new('Job directory already exists.')
        }
        [void] [IO.Directory]::CreateDirectory($jobDirectory)
        Assert-PlainPath $jobDirectory $true
        $jobStage = 'job_start'
        $arguments = '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "' + $Accepted.sourcePath + '"'
        $process = Start-Process -FilePath $nativePowerShell -ArgumentList $arguments -WorkingDirectory $root `
            -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $jobDirectory 'stdout-native.log') `
            -RedirectStandardError (Join-Path $jobDirectory 'stderr-native.log')
        # Register immediately so every returned native child survives metadata failures.
        $job = @{ process = $process; handle = $null; pid = $null; exactBirthUtc = $null
            source = $request.filename; sourceSha256 = $request.sourceSha256
            sourceStream = $Accepted.sourceStream; directory = $jobDirectory
            startedAtUtc = [DateTime]::UtcNow.ToString('o'); exitCode = $null; finishedAtUtc = $null
            startRecorded = $false; metadataExceptionType = $null }
        $activeJobs.Add($request.id, $job)
        $registered = $true
        # Access and retain the actual kernel process handle before any polling.
        $jobStage = 'job_native_identity'
        $job.handle = $process.Handle
        $job.pid = $process.Id
        $job.exactBirthUtc = $process.StartTime.ToUniversalTime().ToString('o')
        $jobStage = 'job_start_record'
        Write-JobStart $request.id $job
    } catch {
        $_.Exception.Data['safeStage'] = $jobStage
        if ($registered) {
            # Execution has already started. Preserve it for observation retries;
            # a metadata/write failure is not an authentication rejection.
            $job.metadataExceptionType = $_.Exception.GetType().FullName
            return
        }
        if (-not $registered) {
            if ([IO.Directory]::Exists($jobDirectory)) {
                Write-SafeJson (Join-Path $jobDirectory 'terminal-safe.json') ([ordered]@{
                    schema = 1; id = $request.id; state = 'failed_to_start'; pid = $null
                    exactBirthUtc = $null; source = $request.filename; sourceSha256 = $request.sourceSha256
                    nonce = $BootstrapNonce; nativeExitCode = $null; stage = $jobStage
                    exceptionType = $_.Exception.GetType().FullName
                    failureCode = (Get-ExceptionCode $_.Exception 1); finishedAtUtc = [DateTime]::UtcNow.ToString('o')
                }) $true
            }
            $Accepted.sourceStream.Dispose()
        }
        throw
    }
}

try {
    $stage = 'owned_paths'
    Assert-PlainPath $root $true
    $requestsDirectory = [IO.Path]::GetFullPath((Join-Path $root 'requests'))
    $sourcesDirectory = [IO.Path]::GetFullPath((Join-Path $root 'sources'))
    $jobsDirectory = [IO.Path]::GetFullPath((Join-Path $root 'jobs'))
    foreach ($directory in @($requestsDirectory, $sourcesDirectory, $jobsDirectory)) {
        Assert-PlainPath $directory $true
    }
    Assert-PlainPath $PSCommandPath $false
    $stage = 'owned_root_acl'
    Set-OwnedRootAcl
    $stage = 'worker_lock'
    $lockStream = [IO.File]::Open((Join-Path $root 'worker.lock'), [IO.FileMode]::CreateNew,
        [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
    $stage = 'worker_identity'
    $selfProcess = [Diagnostics.Process]::GetCurrentProcess()
    $selfHandle = $selfProcess.Handle
    $workerIdentity = [ordered]@{
        schema = 1; pid = $selfProcess.Id; exactBirthUtc = $selfProcess.StartTime.ToUniversalTime().ToString('o')
        source = [IO.Path]::GetFileName($PSCommandPath); sourceSha256 = (Get-FileSha256 $PSCommandPath)
        nonce = $BootstrapNonce
    }
    $lockBytes = ([Text.UTF8Encoding]::new($false)).GetBytes((ConvertTo-Json -InputObject $workerIdentity -Compress))
    $lockStream.Write($lockBytes, 0, $lockBytes.Length)
    $lockStream.Flush($true)
    $stage = 'capability_key'
    $keyPath = Join-Path $root 'bridge-key-private.bin'
    Assert-PlainPath $keyPath $false
    $capabilityKey = [IO.File]::ReadAllBytes($keyPath)
    if ($capabilityKey.Length -ne 32) { throw [IO.InvalidDataException]::new('Invalid capability key length.') }
    $nativePowerShell = Join-Path $PSHOME 'powershell.exe'
    Assert-PlainPath $nativePowerShell $false
    $stage = 'worker_ready'
    Write-Heartbeat
    Write-SafeJson (Join-Path $root 'worker.ready-safe.json') ([ordered]@{
        schema = 1; state = 'ready'; pid = $workerIdentity.pid; exactBirthUtc = $workerIdentity.exactBirthUtc
        source = $workerIdentity.source; sourceSha256 = $workerIdentity.sourceSha256
        nonce = $BootstrapNonce; readyAtUtc = [DateTime]::UtcNow.ToString('o'); lifetimeMinutes = 90
    }) $true
    $exitCode = 0
    $exitReason = 'lifetime_elapsed'
    while ($true) {
        $stage = 'observe_jobs'
        Observe-Jobs
        if ($clock.ElapsedMilliseconds - $heartbeatAt -ge 2000) {
            $stage = 'heartbeat'
            Write-Heartbeat
        }
        if ($clock.Elapsed.TotalMinutes -ge 90) {
            if ($activeJobs.Count -eq 0) { break }
        } elseif ($clock.ElapsedMilliseconds - $scanAt -ge 500) {
            $scanAt = $clock.ElapsedMilliseconds
            $stage = 'request_scan'
            foreach ($item in @(Get-ChildItem -LiteralPath $requestsDirectory -Filter 'request-*.json' -File)) {
                if ($clock.Elapsed.TotalMinutes -ge 90) { break }
                if ($item.Name -cnotmatch '^request-[a-f0-9]{32}\.json$') { continue }
                try {
                    $accepted = Read-AuthenticatedRequest $item
                    if ($accepted.request.operation -ceq 'stop') {
                        $accepted.sourceStream.Dispose()
                        $canStop = ($activeJobs.Count -eq 0)
                        $stopState = 'refused_live_jobs'
                        if ($canStop) { $stopState = 'accepted' }
                        Write-SafeJson (Join-Path $requestsDirectory ('stop-' + $accepted.request.id + '-safe.json')) ([ordered]@{
                            schema = 1; id = $accepted.request.id; state = $stopState
                            liveJobs = $activeJobs.Count; nonce = $BootstrapNonce; timeUtc = [DateTime]::UtcNow.ToString('o')
                        }) $true
                        if ($canStop) { $exitReason = 'stop_requested'; break }
                    } else {
                        Start-OwnedJob $accepted
                    }
                } catch {
                    Reject-Request $item $_.Exception
                }
            }
            if ($exitReason -eq 'stop_requested') { break }
        }
        [Threading.Thread]::Sleep(100)
    }
} catch {
    $failureStage = $stage
    $failureType = $_.Exception.GetType().FullName
    $exitCode = Get-ExceptionCode $_.Exception 1
    $exitReason = 'failed'
} finally {
    # Never kill children on shutdown or elapsed lifetime; keep handles and observe them to actual exit.
    while ($activeJobs.Count -gt 0) {
        try { Observe-Jobs } catch {
            if ($null -eq $failureType) {
                $failureStage = 'drain_jobs'; $failureType = $_.Exception.GetType().FullName
                $exitCode = Get-ExceptionCode $_.Exception 1; $exitReason = 'failed'
            }
        }
        if ($null -ne $workerIdentity -and $clock.ElapsedMilliseconds - $heartbeatAt -ge 2000) {
            try { Write-Heartbeat } catch { $heartbeatAt = $clock.ElapsedMilliseconds }
        }
        if ($activeJobs.Count -gt 0) { [Threading.Thread]::Sleep(100) }
    }
    if ($null -ne $lockStream) {
        try {
            $terminalPid = $null; $terminalBirth = $null; $terminalSource = 'worker.ps1'; $terminalHash = $null
            if ($null -ne $workerIdentity) {
                $terminalPid = $workerIdentity.pid; $terminalBirth = $workerIdentity.exactBirthUtc
                $terminalSource = $workerIdentity.source; $terminalHash = $workerIdentity.sourceSha256
            }
            Write-SafeJson (Join-Path $root 'worker.terminal-safe.json') ([ordered]@{
                schema = 1; state = $exitReason; pid = $terminalPid
                exactBirthUtc = $terminalBirth; source = $terminalSource
                sourceSha256 = $terminalHash; nonce = $BootstrapNonce
                selfReportedExitCode = $exitCode; selfReportedNotIndependentOsExit = $true
                stage = $failureStage; exceptionType = $failureType
                finishedAtUtc = [DateTime]::UtcNow.ToString('o')
            }) $true
        } catch {
            if ($exitCode -eq 0) { $exitCode = Get-ExceptionCode $_.Exception 1 }
            $failureStage = 'worker_terminal'; $failureType = $_.Exception.GetType().FullName
        }
        $lockStream.Dispose()
    }
    if ($null -ne $capabilityKey) { [Array]::Clear($capabilityKey, 0, $capabilityKey.Length) }
    if ($null -ne $selfProcess) { $selfProcess.Dispose() }
}
if ($exitCode -ne 0) {
    [Console]::WriteLine((ConvertTo-Json -InputObject ([ordered]@{
        state = 'failed'; stage = $failureStage; exceptionType = $failureType; exitCode = $exitCode
    }) -Compress))
}
exit $exitCode
