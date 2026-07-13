[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version 2.0

$ScriptRoot = Split-Path -Parent $PSScriptRoot
$PowerShellInstaller = Join-Path $ScriptRoot "install.ps1"
$ShellInstaller = Join-Path $ScriptRoot "install.sh"
$PowerShell = (Get-Process -Id $PID).Path
$TempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("orchester-install-shim-test-" + [guid]::NewGuid().ToString("N"))
$OriginalUserProfile = $env:USERPROFILE
$OriginalLocalAppData = $env:LOCALAPPDATA
$OriginalShimDirectory = $env:ORCHESTER_WINDOWS_SHIM_DIR
$OriginalTarget = $env:WIN_ORCHESTER_TARGET
$OriginalRequestedDirectory = $env:WIN_ORCHESTER_SHIM_DIR

function Fail([string]$Message) {
    throw "install shim test: $Message"
}

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) {
        Fail $Message
    }
}

function Write-Utf8File([string]$Path, [string]$Contents) {
    $parent = Split-Path -Parent $Path
    if ($parent) {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Contents, $utf8)
}

function Invoke-EmbeddedShim([string]$Script, [string]$Target, [string]$Directory) {
    $env:WIN_ORCHESTER_TARGET = $Target
    $env:WIN_ORCHESTER_SHIM_DIR = $Directory
    $output = @(& $PowerShell -NoProfile -ExecutionPolicy Bypass -File $Script 2>&1)
    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0) {
        Fail "embedded shim helper failed with exit code $exitCode`: $($output -join ' ')"
    }
    return $output
}

try {
    New-Item -ItemType Directory -Force -Path $TempRoot | Out-Null
    $env:USERPROFILE = Join-Path $TempRoot "home"
    $env:LOCALAPPDATA = Join-Path $env:USERPROFILE "AppData\Local"
    New-Item -ItemType Directory -Force -Path $env:USERPROFILE | Out-Null

    $tokens = $null
    $parseErrors = $null
    $ast = [System.Management.Automation.Language.Parser]::ParseFile(
        $PowerShellInstaller,
        [ref]$tokens,
        [ref]$parseErrors
    )
    Assert-True ($parseErrors.Count -eq 0) "PowerShell installer did not parse"
    $requiredFunctions = @(
        "Test-PathInProcessPath",
        "Get-NormalizedPathText",
        "Test-PathWithinUserProfile",
        "Test-NoReparseComponents",
        "Test-SafeShimDirectory",
        "Test-OwnedCommandShim",
        "Test-WritableDirectory",
        "Ensure-WindowsCommandShim"
    )
    $functionAsts = @($ast.FindAll({
        param($node)
        return (
            ($node -is [System.Management.Automation.Language.FunctionDefinitionAst]) -and
            ($requiredFunctions -contains $node.Name)
        )
    }, $true) | Sort-Object { $_.Extent.StartOffset })
    Assert-True ($functionAsts.Count -eq $requiredFunctions.Count) "could not load installer shim helpers"
    foreach ($functionAst in $functionAsts) {
        Invoke-Expression $functionAst.Extent.Text
    }

    $target = Join-Path $TempRoot "install\bin\orchester.exe"
    Write-Utf8File $target "binary"

    $safeDirectory = Join-Path $env:USERPROFILE "bin"
    $env:ORCHESTER_WINDOWS_SHIM_DIR = $safeDirectory
    $shim = Ensure-WindowsCommandShim $target
    $expectedShim = Join-Path $safeDirectory "orchester.cmd"
    Assert-True ($shim -eq $expectedShim) "safe custom shim directory was rejected"
    Assert-True (Test-Path -LiteralPath $expectedShim -PathType Leaf) "safe shim was not created"

    $prefixCollision = $env:USERPROFILE + "-evil\bin"
    $env:ORCHESTER_WINDOWS_SHIM_DIR = $prefixCollision
    $shim = Ensure-WindowsCommandShim $target
    Assert-True ([string]::IsNullOrWhiteSpace($shim)) "USERPROFILE prefix collision was accepted"
    Assert-True (-not (Test-Path -LiteralPath $prefixCollision)) "unsafe prefix directory was created"

    $unrelatedDirectory = Join-Path $env:USERPROFILE "unrelated"
    $unrelatedShim = Join-Path $unrelatedDirectory "orchester.cmd"
    Write-Utf8File $unrelatedShim "do not replace`r`n"
    $env:ORCHESTER_WINDOWS_SHIM_DIR = $unrelatedDirectory
    $shim = Ensure-WindowsCommandShim $target
    Assert-True ([string]::IsNullOrWhiteSpace($shim)) "unrelated existing shim was overwritten"
    Assert-True ([System.IO.File]::ReadAllText($unrelatedShim).StartsWith("do not replace")) "unrelated shim contents changed"

    [System.IO.File]::WriteAllLines($unrelatedShim, @("@echo off", '"C:\old\orchester.exe" %*'))
    $shim = Ensure-WindowsCommandShim $target
    Assert-True ($shim -eq $unrelatedShim) "recognized Orchester shim could not be updated"
    Assert-True ([System.IO.File]::ReadAllText($unrelatedShim).Contains($target)) "recognized shim target was not updated"

    $outside = Join-Path $TempRoot "outside"
    $junction = Join-Path $env:USERPROFILE "junction"
    New-Item -ItemType Directory -Force -Path $outside | Out-Null
    try {
        New-Item -ItemType Junction -Path $junction -Target $outside -ErrorAction Stop | Out-Null
        $env:ORCHESTER_WINDOWS_SHIM_DIR = $junction
        $shim = Ensure-WindowsCommandShim $target
        Assert-True ([string]::IsNullOrWhiteSpace($shim)) "junction-backed shim directory was accepted"
        Assert-True (-not (Test-Path -LiteralPath (Join-Path $outside "orchester.cmd"))) "shim escaped through junction"
    } catch [System.Management.Automation.PSNotSupportedException] {
        Write-Host "shim junction test skipped: unsupported host"
    } catch [System.UnauthorizedAccessException] {
        Write-Host "shim junction test skipped: privilege unavailable"
    }

    $shellLines = [System.IO.File]::ReadAllLines($ShellInstaller)
    $start = [Array]::IndexOf($shellLines, "# ORCHESTER_EMBEDDED_SHIM_BEGIN")
    $end = [Array]::IndexOf($shellLines, "# ORCHESTER_EMBEDDED_SHIM_END")
    Assert-True ($start -ge 0 -and $end -gt $start) "embedded shim helper markers are missing"
    $embeddedScript = Join-Path $TempRoot "embedded-shim.ps1"
    $embeddedContents = ($shellLines[($start + 1)..($end - 1)] -join [Environment]::NewLine) + [Environment]::NewLine
    Write-Utf8File $embeddedScript $embeddedContents

    $embeddedSafe = Join-Path $env:USERPROFILE "embedded-safe"
    $output = @(Invoke-EmbeddedShim $embeddedScript $target $embeddedSafe)
    Assert-True ($output.Count -eq 1) "embedded helper emitted extra stdout"
    Assert-True ($output[0].ToString() -eq (Join-Path $embeddedSafe "orchester.cmd")) "embedded helper returned the wrong shim path"

    $embeddedUnsafe = $env:USERPROFILE + "-evil\embedded"
    $output = @(Invoke-EmbeddedShim $embeddedScript $target $embeddedUnsafe)
    Assert-True ($output.Count -eq 1 -and $output[0].ToString() -eq "SKIPPED") "embedded helper accepted a profile prefix collision"
    Assert-True (-not (Test-Path -LiteralPath $embeddedUnsafe)) "embedded helper created an unsafe directory"

    $embeddedExisting = Join-Path $env:USERPROFILE "embedded-existing"
    $embeddedExistingShim = Join-Path $embeddedExisting "orchester.cmd"
    Write-Utf8File $embeddedExistingShim "do not replace`r`n"
    $output = @(Invoke-EmbeddedShim $embeddedScript $target $embeddedExisting)
    Assert-True ($output.Count -eq 1 -and $output[0].ToString() -eq "SKIPPED") "embedded helper overwrote an unrelated shim"
    Assert-True ([System.IO.File]::ReadAllText($embeddedExistingShim).StartsWith("do not replace")) "embedded unrelated shim contents changed"

    Write-Host "install shim tests passed"
} finally {
    $env:USERPROFILE = $OriginalUserProfile
    $env:LOCALAPPDATA = $OriginalLocalAppData
    $env:ORCHESTER_WINDOWS_SHIM_DIR = $OriginalShimDirectory
    $env:WIN_ORCHESTER_TARGET = $OriginalTarget
    $env:WIN_ORCHESTER_SHIM_DIR = $OriginalRequestedDirectory
    if (Test-Path -LiteralPath $TempRoot) {
        Remove-Item -LiteralPath $TempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
