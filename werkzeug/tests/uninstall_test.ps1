[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version 2.0

$ScriptRoot = Split-Path -Parent $PSScriptRoot
$Uninstaller = Join-Path $ScriptRoot "uninstall.ps1"
$Bootstrap = Join-Path (Split-Path -Parent $ScriptRoot) "uninstall.ps1"
$PowerShell = (Get-Process -Id $PID).Path
$TempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("orchester-uninstall-test-" + [guid]::NewGuid().ToString("N"))
$OriginalUserProfile = $env:USERPROFILE
$OriginalOrchesterHome = $env:ORCHESTER_HOME
$OriginalUninstallerUrl = $env:ORCHESTER_UNINSTALL_SCRIPT_URL

function Fail([string]$Message) {
    throw "uninstall test: $Message"
}

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) {
        Fail $Message
    }
}

function Assert-Exists([string]$Path) {
    Assert-True (Test-Path -LiteralPath $Path) "expected path to exist: $Path"
}

function Assert-Missing([string]$Path) {
    Assert-True (-not (Test-Path -LiteralPath $Path)) "expected path to be absent: $Path"
}

function Write-Utf8File([string]$Path, [string]$Contents) {
    $parent = Split-Path -Parent $Path
    if ($parent) {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Contents, $utf8)
}

function Get-LowerHash([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Convert-ToMsysPath([string]$Path) {
    return "/" + $Path.Substring(0, 1).ToLowerInvariant() + ($Path.Substring(2) -replace '\\', '/')
}

function New-Fixture([string]$Name) {
    $root = Join-Path $TempRoot $Name
    $bin = Join-Path $root "bin\orchester.exe"
    $receipt = Join-Path $root ".orchester\install.receipt"
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $bin) | Out-Null
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $receipt) | Out-Null
    Write-Utf8File $bin "owned binary`r`n"
    return @{
        Root = $root
        Bin = $bin
        Receipt = $receipt
        Hash = Get-LowerHash $bin
    }
}

function New-ReceiptLines($Fixture) {
    return @(
        "schema`t1"
        "install_root`t$($Fixture.Root)"
        "bin`t$($Fixture.Bin)"
        "binary_hash`t$($Fixture.Hash)"
        "shim`t"
        "shim_hash`t"
    )
}

function Write-Receipt($Fixture, [string[]]$Lines) {
    Write-Utf8File $Fixture.Receipt (($Lines -join "`r`n") + "`r`n")
}

function Invoke-Uninstaller {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [switch]$Purge,
        [switch]$NoPathUpdate,
        [switch]$UseBootstrap
    )

    $script = if ($UseBootstrap) { $Bootstrap } else { $Uninstaller }
    $arguments = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $script, "-InstallRoot", $Root)
    if ($Purge) {
        $arguments += "-Purge"
    }
    if ($NoPathUpdate) {
        $arguments += "-NoPathUpdate"
    }
    $previousPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        $output = & $PowerShell @arguments 2>&1 | Out-String
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousPreference
    }
    return @{
        ExitCode = $exitCode
        Output = $output
    }
}

function Assert-Rejected($Result, [string]$Message) {
    Assert-True ($Result.ExitCode -ne 0) $Message
}

function Test-MalformedReceipt([string]$Name, [scriptblock]$Mutate) {
    $fixture = New-Fixture $Name
    $lines = @(New-ReceiptLines $fixture)
    $lines = @(& $Mutate $lines)
    Write-Receipt $fixture $lines
    $result = Invoke-Uninstaller -Root $fixture.Root -NoPathUpdate
    Assert-Rejected $result "malformed receipt was accepted: $Name"
    Assert-Exists $fixture.Bin
    Assert-Exists $fixture.Receipt
}

try {
    New-Item -ItemType Directory -Force -Path $TempRoot | Out-Null
    $env:USERPROFILE = Join-Path $TempRoot "home"
    $env:ORCHESTER_HOME = Join-Path $env:USERPROFILE ".orchester"
    New-Item -ItemType Directory -Force -Path $env:USERPROFILE | Out-Null

    if (-not (Test-Path -LiteralPath $Uninstaller)) {
        Fail "uninstaller does not exist: $Uninstaller"
    }

    # Valid receipts remove only hash-verified owned artifacts.
    $valid = New-Fixture "valid"
    $otherBinary = Join-Path $valid.Root "bin\other-tool.exe"
    $shim = Join-Path $env:USERPROFILE "bin\orchester.cmd"
    Write-Utf8File $otherBinary "foreign cargo binary`r`n"
    Write-Utf8File $shim "@echo off`r`n"
    $validLines = @(New-ReceiptLines $valid)
    $validLines[4] = "shim`t$shim"
    $validLines[5] = "shim_hash`t$(Get-LowerHash $shim)"
    Write-Receipt $valid $validLines
    $result = Invoke-Uninstaller -Root $valid.Root -NoPathUpdate
    Assert-True ($result.ExitCode -eq 0) "valid receipt was rejected: $($result.Output)"
    Assert-Missing $valid.Bin
    Assert-Missing $shim
    Assert-Missing $valid.Receipt
    Assert-Missing (Split-Path -Parent $valid.Receipt)
    Assert-Exists $otherBinary

    # Git Bash receipts use /c/... paths; PowerShell must normalize them to
    # the same native files instead of treating them as D:\c\... paths.
    $msys = New-Fixture "msys-path"
    $msysLines = @(New-ReceiptLines $msys)
    $msysLines[1] = "install_root`t$(Convert-ToMsysPath $msys.Root)"
    $msysLines[2] = "bin`t$(Convert-ToMsysPath $msys.Bin)"
    Write-Receipt $msys $msysLines
    $result = Invoke-Uninstaller -Root $msys.Root -NoPathUpdate
    Assert-True ($result.ExitCode -eq 0) "MSYS receipt paths were rejected: $($result.Output)"
    Assert-Missing $msys.Bin
    Assert-Missing $msys.Receipt

    # Native profile records remove only the exact installer line and marker.
    $profileFixture = New-Fixture "profile"
    $profile = Join-Path $env:USERPROFILE ".profile"
    $profileBinDirectory = Split-Path -Parent $profileFixture.Bin
    $profileLine = 'export PATH="' + $profileBinDirectory + ':$PATH"'
    Write-Utf8File $profile ("keep-before`r`n# Orchester CLI`r`n" + $profileLine + "`r`nkeep-after`r`n")
    $profileLines = @(New-ReceiptLines $profileFixture) + @(
        "path_profile`t$profile"
        "path_added`t1"
        "path_line`t$profileLine"
        "path_marker`t# Orchester CLI"
    )
    Write-Receipt $profileFixture $profileLines
    $result = Invoke-Uninstaller -Root $profileFixture.Root
    Assert-True ($result.ExitCode -eq 0) "profile cleanup failed: $($result.Output)"
    $profileContents = [System.IO.File]::ReadAllText($profile)
    Assert-True $profileContents.Contains("keep-before") "profile prefix was lost"
    Assert-True $profileContents.Contains("keep-after") "profile suffix was lost"
    Assert-True (-not $profileContents.Contains($profileLine)) "installer PATH line remained in profile"
    Assert-True (-not $profileContents.Contains("# Orchester CLI")) "installer marker remained in profile"

    # Repeating a completed uninstall is an idempotent no-op.
    $result = Invoke-Uninstaller -Root $valid.Root -NoPathUpdate
    Assert-True ($result.ExitCode -eq 0) "idempotent uninstall failed: $($result.Output)"

    # The root bootstrap delegates to the repository-local implementation.
    $boot = New-Fixture "bootstrap"
    Write-Receipt $boot (New-ReceiptLines $boot)
    $env:ORCHESTER_UNINSTALL_SCRIPT_URL = "http://127.0.0.1:1/must-not-download"
    $result = Invoke-Uninstaller -Root $boot.Root -NoPathUpdate -UseBootstrap
    Assert-True ($result.ExitCode -eq 0) "bootstrap uninstall failed: $($result.Output)"
    Assert-Missing $boot.Bin
    $env:ORCHESTER_UNINSTALL_SCRIPT_URL = $OriginalUninstallerUrl

    # Purge removes known files but preserves unknown user data.
    $purge = New-Fixture "purge"
    $knownConfig = Join-Path $env:ORCHESTER_HOME "orchester.jsonc"
    $knownSessions = Join-Path $env:ORCHESTER_HOME "sessions.jsonl"
    $unknownConfig = Join-Path $env:ORCHESTER_HOME "notes.txt"
    Write-Utf8File $knownConfig "{}"
    Write-Utf8File $knownSessions "{}"
    Write-Utf8File $unknownConfig "keep"
    $purgeLines = @(New-ReceiptLines $purge) + @("config_dir`t$env:ORCHESTER_HOME")
    Write-Receipt $purge $purgeLines
    $result = Invoke-Uninstaller -Root $purge.Root -Purge -NoPathUpdate
    Assert-True ($result.ExitCode -eq 0) "purge failed: $($result.Output)"
    Assert-Missing $knownConfig
    Assert-Missing $knownSessions
    Assert-Exists $unknownConfig

    # Purge validates every known target before removing the binary or receipt.
    $purgePreflight = New-Fixture "purge-preflight"
    New-Item -ItemType Directory -Force -Path $knownConfig | Out-Null
    $purgePreflightLines = @(New-ReceiptLines $purgePreflight) + @("config_dir`t$env:ORCHESTER_HOME")
    Write-Receipt $purgePreflight $purgePreflightLines
    $result = Invoke-Uninstaller -Root $purgePreflight.Root -Purge -NoPathUpdate
    Assert-Rejected $result "directory at a known purge file path was accepted"
    Assert-Exists $purgePreflight.Bin
    Assert-Exists $purgePreflight.Receipt
    Assert-Exists $knownConfig
    Remove-Item -LiteralPath $knownConfig -Force

    # A foreign binary without a receipt is never guessed or deleted.
    $foreign = New-Fixture "foreign"
    Remove-Item -LiteralPath $foreign.Receipt -Force -ErrorAction SilentlyContinue
    $result = Invoke-Uninstaller -Root $foreign.Root -NoPathUpdate
    Assert-Rejected $result "binary without receipt was accepted"
    Assert-Exists $foreign.Bin

    # Hash mismatches block the entire uninstall before any owned file changes.
    $modifiedBinary = New-Fixture "modified-binary"
    Write-Receipt $modifiedBinary (New-ReceiptLines $modifiedBinary)
    Write-Utf8File $modifiedBinary.Bin "user modification`r`n"
    $result = Invoke-Uninstaller -Root $modifiedBinary.Root -NoPathUpdate
    Assert-Rejected $result "modified binary was accepted"
    Assert-Exists $modifiedBinary.Bin
    Assert-Exists $modifiedBinary.Receipt

    $modifiedShim = New-Fixture "modified-shim"
    $modifiedShimPath = Join-Path $env:USERPROFILE "modified-shim\orchester.cmd"
    Write-Utf8File $modifiedShimPath "owned shim`r`n"
    $modifiedShimLines = @(New-ReceiptLines $modifiedShim)
    $modifiedShimLines[4] = "shim`t$modifiedShimPath"
    $modifiedShimLines[5] = "shim_hash`t$(Get-LowerHash $modifiedShimPath)"
    Write-Receipt $modifiedShim $modifiedShimLines
    Write-Utf8File $modifiedShimPath "changed shim`r`n"
    $result = Invoke-Uninstaller -Root $modifiedShim.Root -NoPathUpdate
    Assert-Rejected $result "modified shim was accepted"
    Assert-Exists $modifiedShim.Bin
    Assert-Exists $modifiedShimPath

    # Receipt paths cannot escape the selected root or current user profile.
    $rootMismatch = New-Fixture "root-mismatch"
    $rootMismatchLines = @(New-ReceiptLines $rootMismatch)
    $rootMismatchLines[1] = "install_root`t$(Join-Path $TempRoot 'elsewhere')"
    Write-Receipt $rootMismatch $rootMismatchLines
    $result = Invoke-Uninstaller -Root $rootMismatch.Root -NoPathUpdate
    Assert-Rejected $result "root mismatch was accepted"
    Assert-Exists $rootMismatch.Bin

    $unsafeBin = New-Fixture "unsafe-bin"
    $outsideBin = Join-Path $TempRoot "outside\orchester.exe"
    Write-Utf8File $outsideBin "outside"
    $unsafeBinLines = @(New-ReceiptLines $unsafeBin)
    $unsafeBinLines[2] = "bin`t$outsideBin"
    $unsafeBinLines[3] = "binary_hash`t$(Get-LowerHash $outsideBin)"
    Write-Receipt $unsafeBin $unsafeBinLines
    $result = Invoke-Uninstaller -Root $unsafeBin.Root -NoPathUpdate
    Assert-Rejected $result "binary outside root was accepted"
    Assert-Exists $outsideBin
    Assert-Exists $unsafeBin.Bin

    $unsafeShim = New-Fixture "unsafe-shim"
    $outsideShim = Join-Path $TempRoot "outside-shim\orchester.cmd"
    Write-Utf8File $outsideShim "outside shim"
    $unsafeShimLines = @(New-ReceiptLines $unsafeShim)
    $unsafeShimLines[4] = "shim`t$outsideShim"
    $unsafeShimLines[5] = "shim_hash`t$(Get-LowerHash $outsideShim)"
    Write-Receipt $unsafeShim $unsafeShimLines
    $result = Invoke-Uninstaller -Root $unsafeShim.Root -NoPathUpdate
    Assert-Rejected $result "shim outside USERPROFILE was accepted"
    Assert-Exists $unsafeShim.Bin
    Assert-Exists $outsideShim

    $unsafePath = New-Fixture "unsafe-path"
    $unsafePathLines = @(New-ReceiptLines $unsafePath) + @(
        "windows_path_item`t$(Join-Path $TempRoot 'another-bin')"
        "windows_path_added`t1"
    )
    Write-Receipt $unsafePath $unsafePathLines
    $result = Invoke-Uninstaller -Root $unsafePath.Root -NoPathUpdate
    Assert-Rejected $result "unrelated Windows PATH item was accepted"
    Assert-Exists $unsafePath.Bin

    $validPath = New-Fixture "valid-path"
    $validPathItem = Split-Path -Parent $validPath.Bin
    $validPathLines = @(New-ReceiptLines $validPath) + @(
        "windows_path_item`t$validPathItem"
        "windows_path_added`t1"
    )
    Write-Receipt $validPath $validPathLines
    $userPathBefore = [Environment]::GetEnvironmentVariable("Path", "User")
    $result = Invoke-Uninstaller -Root $validPath.Root -NoPathUpdate
    Assert-True ($result.ExitCode -eq 0) "valid Windows PATH record was rejected: $($result.Output)"
    $userPathAfter = [Environment]::GetEnvironmentVariable("Path", "User")
    Assert-True ($userPathAfter -eq $userPathBefore) "-NoPathUpdate changed the Windows user PATH"

    $unsafeConfig = New-Fixture "unsafe-config"
    $unsafeConfigLines = @(New-ReceiptLines $unsafeConfig) + @(
        "config_dir`t$(Join-Path $TempRoot 'outside-config')"
    )
    Write-Receipt $unsafeConfig $unsafeConfigLines
    $result = Invoke-Uninstaller -Root $unsafeConfig.Root -Purge -NoPathUpdate
    Assert-Rejected $result "config directory outside ORCHESTER_HOME was accepted"
    Assert-Exists $unsafeConfig.Bin

    # Strict TSV parsing rejects ambiguity before deleting anything.
    Test-MalformedReceipt "duplicate-field" {
        param($Lines)
        return @($Lines) + @("schema`t1")
    }
    Test-MalformedReceipt "unknown-field" {
        param($Lines)
        return @($Lines) + @("unexpected`tvalue")
    }
    Test-MalformedReceipt "extra-tab" {
        param($Lines)
        $Lines[1] = $Lines[1] + "`textra"
        return $Lines
    }
    Test-MalformedReceipt "control-value" {
        param($Lines)
        $Lines[1] = $Lines[1] + [char]0
        return $Lines
    }
    Test-MalformedReceipt "duplicate-profile" {
        param($Lines)
        $profile = Join-Path $env:USERPROFILE ".bashrc"
        $binDirectory = Split-Path -Parent (($Lines | Where-Object { $_.StartsWith("bin`t") }) -replace '^bin\t', '')
        $pathLine = 'export PATH="' + $binDirectory + ':$PATH"'
        return @($Lines) + @(
            "path_profile`t$profile"
            "path_added`t1"
            "path_line`t$pathLine"
            "path_profile`t$profile"
            "path_added`t1"
            "path_line`t$pathLine"
        )
    }

    # A bin directory junction cannot redirect deletion outside the install root.
    $junctionRoot = Join-Path $TempRoot "junction"
    $junctionTarget = Join-Path $TempRoot "junction-target"
    $junctionBinDirectory = Join-Path $junctionRoot "bin"
    $junctionBinary = Join-Path $junctionBinDirectory "orchester.exe"
    $junctionReceipt = Join-Path $junctionRoot ".orchester\install.receipt"
    New-Item -ItemType Directory -Force -Path $junctionRoot, $junctionTarget, (Split-Path -Parent $junctionReceipt) | Out-Null
    Write-Utf8File (Join-Path $junctionTarget "orchester.exe") "junction target`r`n"
    try {
        New-Item -ItemType Junction -Path $junctionBinDirectory -Target $junctionTarget -ErrorAction Stop | Out-Null
        $junctionFixture = @{
            Root = $junctionRoot
            Bin = $junctionBinary
            Receipt = $junctionReceipt
            Hash = Get-LowerHash $junctionBinary
        }
        Write-Receipt $junctionFixture (New-ReceiptLines $junctionFixture)
        $result = Invoke-Uninstaller -Root $junctionRoot -NoPathUpdate
        Assert-Rejected $result "binary directory junction was accepted"
        Assert-Exists (Join-Path $junctionTarget "orchester.exe")
        Assert-Exists $junctionReceipt
    } catch [System.Management.Automation.PSNotSupportedException] {
        Write-Host "junction test skipped: unsupported host"
    }

    # A receipt symlink is untrusted. Skip only when the host denies symlinks.
    $link = New-Fixture "receipt-link"
    $realReceipt = Join-Path $link.Root ".orchester\real.receipt"
    Write-Receipt $link (New-ReceiptLines $link)
    Move-Item -LiteralPath $link.Receipt -Destination $realReceipt
    try {
        New-Item -ItemType SymbolicLink -Path $link.Receipt -Target $realReceipt -ErrorAction Stop | Out-Null
        $result = Invoke-Uninstaller -Root $link.Root -NoPathUpdate
        Assert-Rejected $result "receipt symlink was accepted"
        Assert-Exists $link.Bin
    } catch [System.UnauthorizedAccessException] {
        Write-Host "receipt symlink test skipped: privilege unavailable"
    } catch [System.Management.Automation.PSNotSupportedException] {
        Write-Host "receipt symlink test skipped: unsupported host"
    }

    Write-Host "PowerShell uninstall tests passed"
} finally {
    $env:USERPROFILE = $OriginalUserProfile
    $env:ORCHESTER_HOME = $OriginalOrchesterHome
    $env:ORCHESTER_UNINSTALL_SCRIPT_URL = $OriginalUninstallerUrl
    if (Test-Path -LiteralPath $TempRoot) {
        Remove-Item -LiteralPath $TempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
