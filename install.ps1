[CmdletBinding()]
param(
    # Cargo install root. The binary lands in <InstallRoot>\bin\orchester.exe.
    [string]$InstallRoot,

    # Do not update the current user's PATH.
    [switch]$NoPathUpdate,

    # Git ref (branch, tag, or commit) to build.
    [string]$Ref
)

# Thin bootstrapper for the Orchester installer.
#
# One-line install:
#   irm https://raw.githubusercontent.com/dieWehmut/Orchester/main/install.ps1 | iex
#
# Piping into `iex` cannot bind parameters, so the same settings are also read
# from the environment:
#   $env:ORCHESTER_INSTALL_ROOT   = "$env:USERPROFILE\.cargo"
#   $env:ORCHESTER_NO_PATH_UPDATE = "1"
#   $env:ORCHESTER_REPO           = "https://github.com/dieWehmut/Orchester"
#   $env:ORCHESTER_REF            = "main"

$ErrorActionPreference = "Stop"
Set-StrictMode -Version 2.0

if (-not $InstallRoot -and $env:ORCHESTER_INSTALL_ROOT) {
    $InstallRoot = $env:ORCHESTER_INSTALL_ROOT
}
if (-not $NoPathUpdate -and $env:ORCHESTER_NO_PATH_UPDATE) {
    $NoPathUpdate = $true
}
if (-not $Ref) {
    $Ref = if ($env:ORCHESTER_REF) { $env:ORCHESTER_REF } else { "main" }
}
$Repo = if ($env:ORCHESTER_REPO) {
    $env:ORCHESTER_REPO.TrimEnd("/")
} else {
    "https://github.com/dieWehmut/Orchester"
}

function Get-ForwardedArguments {
    $arguments = @{}
    if ($InstallRoot) {
        $arguments.InstallRoot = $InstallRoot
    }
    if ($NoPathUpdate) {
        $arguments.NoPathUpdate = $true
    }
    return $arguments
}

$forwarded = Get-ForwardedArguments

# A checkout already has the full source tree, so build it directly.
if (-not [string]::IsNullOrWhiteSpace($PSScriptRoot)) {
    $localInstaller = Join-Path $PSScriptRoot "werkzeug\install.ps1"
    if (Test-Path -LiteralPath $localInstaller -PathType Leaf) {
        & $localInstaller @forwarded
        return
    }
}

# `werkzeug\install.ps1` compiles the workspace with `cargo install --path`, so
# the whole repository is required, not just that one file. Fetching the source
# archive keeps the one-liner working on a machine without git.
if ($PSVersionTable.PSVersion.Major -lt 5) {
    throw "Windows PowerShell 5.0 or newer is required; found $($PSVersionTable.PSVersion)"
}
try {
    [Net.ServicePointManager]::SecurityProtocol =
        [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
} catch {
    Write-Verbose "Could not raise the TLS version; continuing with the system default."
}

$temporaryDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ("orchester-install-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $temporaryDirectory | Out-Null
try {
    $archive = Join-Path $temporaryDirectory "source.zip"
    $archiveUrl = "$Repo/archive/$Ref.zip"
    Write-Host "Downloading Orchester source: $archiveUrl"
    Invoke-WebRequest -UseBasicParsing -Uri $archiveUrl -OutFile $archive

    $extracted = Join-Path $temporaryDirectory "source"
    Expand-Archive -LiteralPath $archive -DestinationPath $extracted -Force

    # GitHub wraps the tree in a single <repo>-<ref> directory whose name
    # depends on the ref, so locate it by content instead of by name.
    $installer = Get-ChildItem -LiteralPath $extracted -Directory |
        ForEach-Object { Join-Path $_.FullName "werkzeug\install.ps1" } |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
        Select-Object -First 1
    if (-not $installer) {
        throw "downloaded archive does not contain werkzeug\install.ps1 (ref '$Ref')"
    }

    & $installer @forwarded
} finally {
    if (Test-Path -LiteralPath $temporaryDirectory) {
        Remove-Item -LiteralPath $temporaryDirectory -Recurse -Force -ErrorAction SilentlyContinue
    }
}
