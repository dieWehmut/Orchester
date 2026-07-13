[CmdletBinding()]
param(
    [string]$InstallRoot,
    [switch]$Purge,
    [switch]$NoPathUpdate
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version 2.0
$UninstallerUrl = if ($env:ORCHESTER_UNINSTALL_SCRIPT_URL) {
    $env:ORCHESTER_UNINSTALL_SCRIPT_URL
} else {
    "https://raw.githubusercontent.com/dieWehmut/Orchester/main/werkzeug/uninstall.ps1"
}

function Get-ForwardedArguments {
    $arguments = @{}
    if ($InstallRoot) {
        $arguments.InstallRoot = $InstallRoot
    }
    if ($Purge) {
        $arguments.Purge = $true
    }
    if ($NoPathUpdate) {
        $arguments.NoPathUpdate = $true
    }
    return $arguments
}

$localUninstaller = $null
if (-not [string]::IsNullOrWhiteSpace($PSScriptRoot)) {
    $localUninstaller = Join-Path $PSScriptRoot "werkzeug\uninstall.ps1"
}
$forwarded = Get-ForwardedArguments
if ($localUninstaller -and (Test-Path -LiteralPath $localUninstaller -PathType Leaf)) {
    & $localUninstaller @forwarded
    return
}

$temporaryDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ("orchester-uninstall-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $temporaryDirectory | Out-Null
$temporaryScript = Join-Path $temporaryDirectory "uninstall.ps1"
try {
    Invoke-WebRequest -UseBasicParsing -Uri $UninstallerUrl -OutFile $temporaryScript
    $engine = (Get-Process -Id $PID).Path
    $childArguments = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $temporaryScript)
    if ($InstallRoot) {
        $childArguments += @("-InstallRoot", $InstallRoot)
    }
    if ($Purge) {
        $childArguments += "-Purge"
    }
    if ($NoPathUpdate) {
        $childArguments += "-NoPathUpdate"
    }
    & $engine @childArguments
    if ($LASTEXITCODE -ne 0) {
        throw "downloaded Orchester uninstaller failed with exit code $LASTEXITCODE"
    }
} finally {
    if (Test-Path -LiteralPath $temporaryDirectory) {
        Remove-Item -LiteralPath $temporaryDirectory -Recurse -Force -ErrorAction SilentlyContinue
    }
}
