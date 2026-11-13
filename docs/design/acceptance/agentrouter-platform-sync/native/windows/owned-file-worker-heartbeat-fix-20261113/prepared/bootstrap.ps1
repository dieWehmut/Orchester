Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$root = [IO.Path]::GetFullPath($PSScriptRoot)
$stage = 'initializing'
$nonce = [Guid]::NewGuid().ToString('N')
$selfProcess = $null
$child = $null
$lockStream = $null
$bootstrapIdentity = $null
$childIdentity = $null
$exitCode = 1
$nativeExitCode = $null
$failureType = $null
$resultState = 'failed'

function Assert-PlainPath([string] $Path, [bool] $Directory) {
    $attributes = [IO.File]::GetAttributes($Path)
    if (($attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or
        ((($attributes -band [IO.FileAttributes]::Directory) -ne 0) -ne $Directory)) {
        throw [IO.IOException]::new('Unexpected owned path.')
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
    $rules = @($actual.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier]))
    if (-not $actual.AreAccessRulesProtected -or $rules.Count -ne $allowed.Count -or
        $actual.GetOwner([Security.Principal.SecurityIdentifier]).Value -ne $sid.Value) {
        throw [UnauthorizedAccessException]::new('Owned root ACL verification failed.')
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

function Write-SafeJson([string] $Path, $Value) {
    $temporary = $Path + '.tmp-' + [Guid]::NewGuid().ToString('N')
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
        [IO.File]::Move($temporary, $Path)
    } finally {
        if ($null -ne $stream) { $stream.Dispose() }
        if ([IO.File]::Exists($temporary)) { [IO.File]::Delete($temporary) }
    }
}

function Get-FileSha256([string] $Path) {
    $stream = [IO.File]::Open($Path, [IO.FileMode]::Open,
        [IO.FileAccess]::Read, [IO.FileShare]::Read)
    $algorithm = [Security.Cryptography.SHA256]::Create()
    try {
        return ([BitConverter]::ToString($algorithm.ComputeHash($stream))).Replace('-', '').ToLowerInvariant()
    } finally { $algorithm.Dispose(); $stream.Dispose() }
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

try {
    $stage = 'owned_paths'
    Assert-PlainPath $root $true
    Assert-PlainPath $PSCommandPath $false
    $workerPath = Join-Path $root 'worker.ps1'
    Assert-PlainPath $workerPath $false
    foreach ($name in @('requests', 'sources', 'jobs')) {
        Assert-PlainPath (Join-Path $root $name) $true
    }
    $stage = 'owned_root_acl'
    Set-OwnedRootAcl
    $stage = 'bootstrap_lock'
    $lockStream = [IO.File]::Open((Join-Path $root 'bootstrap.lock'), [IO.FileMode]::CreateNew,
        [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
    $stage = 'existing_worker'
    if ([IO.File]::Exists((Join-Path $root 'worker.lock')) -or
        [IO.File]::Exists((Join-Path $root 'worker.ready-safe.json')) -or
        [IO.File]::Exists((Join-Path $root 'worker.terminal-safe.json'))) {
        throw [IO.IOException]::new('Existing worker cannot be replaced.')
    }
    $stage = 'script_ast_validation'
    foreach ($scriptPath in @($PSCommandPath, $workerPath)) {
        $parseTokens = $null
        $parseErrors = $null
        [void] [Management.Automation.Language.Parser]::ParseFile($scriptPath, [ref] $parseTokens, [ref] $parseErrors)
        if (@($parseErrors).Count -ne 0) {
            throw [IO.InvalidDataException]::new('Script AST validation failed.')
        }
    }
    $stage = 'bootstrap_identity'
    $selfProcess = [Diagnostics.Process]::GetCurrentProcess()
    $selfHandle = $selfProcess.Handle
    $bootstrapIdentity = [ordered]@{
        pid = $selfProcess.Id; exactBirthUtc = $selfProcess.StartTime.ToUniversalTime().ToString('o')
        source = [IO.Path]::GetFileName($PSCommandPath); sourceSha256 = (Get-FileSha256 $PSCommandPath)
        nonce = $nonce
    }
    $lockBytes = ([Text.UTF8Encoding]::new($false)).GetBytes((ConvertTo-Json -InputObject $bootstrapIdentity -Compress))
    $lockStream.Write($lockBytes, 0, $lockBytes.Length)
    $lockStream.Flush($true)
    $workerHash = Get-FileSha256 $workerPath
    $stage = 'worker_launch'
    $arguments = '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "' + $workerPath + '" -BootstrapNonce ' + $nonce
    $child = Start-Process -FilePath (Join-Path $PSHOME 'powershell.exe') -Verb Open `
        -WindowStyle Hidden -PassThru -ArgumentList $arguments -WorkingDirectory $root
    $childHandle = $child.Handle
    $stage = 'worker_identity'
    $childIdentity = [ordered]@{
        pid = $child.Id; exactBirthUtc = $child.StartTime.ToUniversalTime().ToString('o')
        source = 'worker.ps1'; sourceSha256 = $workerHash; nonce = $nonce
    }
    $stage = 'bootstrap_start_record'
    Write-SafeJson (Join-Path $root 'bootstrap.started-safe.json') ([ordered]@{
        schema = 1; state = 'started'; bootstrap = $bootstrapIdentity; worker = $childIdentity
        timeUtc = [DateTime]::UtcNow.ToString('o')
    })
    $stage = 'worker_ready_wait'
    $readyPath = Join-Path $root 'worker.ready-safe.json'
    $waitClock = [Diagnostics.Stopwatch]::StartNew()
    while ($true) {
        if ($child.HasExited) {
            $child.WaitForExit()
            $nativeExitCode = [int] $child.ExitCode
            $exitCode = $nativeExitCode
            if ($exitCode -eq 0) { $exitCode = 3 }
            $resultState = 'worker_exited_before_ready'
            break
        }
        if ([IO.File]::Exists($readyPath)) {
            Assert-PlainPath $readyPath $false
            $ready = ConvertFrom-Json -InputObject ([IO.File]::ReadAllText($readyPath))
            if ($ready.state -cne 'ready' -or $ready.pid -ne $childIdentity.pid -or
                $ready.exactBirthUtc -cne $childIdentity.exactBirthUtc -or
                $ready.source -cne $childIdentity.source -or
                $ready.sourceSha256 -cne $childIdentity.sourceSha256 -or $ready.nonce -cne $nonce) {
                throw [IO.InvalidDataException]::new('Worker readiness identity mismatch.')
            }
            # Check the held native process again after reading its authenticated startup identity.
            if ($child.HasExited) { continue }
            $exitCode = 0
            $resultState = 'worker_ready'
            break
        }
        if ($waitClock.Elapsed.TotalSeconds -ge 60) {
            $exitCode = 2
            $resultState = 'ready_timeout_worker_retained'
            break
        }
        [Threading.Thread]::Sleep(100)
    }
} catch {
    $failureType = $_.Exception.GetType().FullName
    $exitCode = Get-ExceptionCode $_.Exception 1
    $resultState = 'failed'
    try {
        if ($null -ne $child -and $child.HasExited) {
            $child.WaitForExit()
            $nativeExitCode = [int] $child.ExitCode
            if ($nativeExitCode -ne 0) { $exitCode = $nativeExitCode }
        }
    } catch { }
} finally {
    if ($null -ne $lockStream) {
        try {
            Write-SafeJson (Join-Path $root 'bootstrap.terminal-safe.json') ([ordered]@{
                schema = 1; state = $resultState; bootstrap = $bootstrapIdentity; worker = $childIdentity
                exitCode = $exitCode; workerNativeExitCode = $nativeExitCode
                stage = $stage; exceptionType = $failureType; finishedAtUtc = [DateTime]::UtcNow.ToString('o')
            })
        } catch {
            if ($exitCode -eq 0) { $exitCode = Get-ExceptionCode $_.Exception 1 }
            $stage = 'bootstrap_terminal'; $failureType = $_.Exception.GetType().FullName
        }
        try { $lockStream.Dispose() } catch {
            if ($exitCode -eq 0) {
                $exitCode = Get-ExceptionCode $_.Exception 1
                $stage = 'bootstrap_lock_dispose'; $failureType = $_.Exception.GetType().FullName
            }
        }
    }
    # Dispose releases this observer's handle without terminating the independent worker.
    foreach ($observer in @($child, $selfProcess)) {
        if ($null -ne $observer) {
            try { $observer.Dispose() } catch {
                if ($exitCode -eq 0) {
                    $exitCode = Get-ExceptionCode $_.Exception 1
                    $stage = 'bootstrap_observer_dispose'; $failureType = $_.Exception.GetType().FullName
                }
            }
        }
    }
}
[Console]::WriteLine((ConvertTo-Json -InputObject ([ordered]@{
    state = $resultState; exitCode = $exitCode; workerNativeExitCode = $nativeExitCode
    stage = $stage; exceptionType = $failureType; bootstrap = $bootstrapIdentity; worker = $childIdentity
}) -Depth 8 -Compress))
exit $exitCode
