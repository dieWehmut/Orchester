[CmdletBinding()]
param(
    # Cargo install root. The binary is installed to <InstallRoot>\bin\orchester.exe.
    [string]$InstallRoot = (Join-Path $env:USERPROFILE ".cargo"),

    # Do not update the current user's PATH.
    [switch]$NoPathUpdate
)

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$BinDir = Join-Path $InstallRoot "bin"
$PackagePath = Join-Path $RepoRoot "kisten\konsole"

function Prepend-PathIfExists([string]$PathItem) {
    if ([string]::IsNullOrWhiteSpace($PathItem) -or -not (Test-Path -LiteralPath $PathItem)) {
        return
    }

    $parts = $env:Path -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
    if ($parts -notcontains $PathItem) {
        $env:Path = "$PathItem;$env:Path"
    }
}

function Ensure-UserPath([string]$PathItem) {
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $parts = @()
    if (-not [string]::IsNullOrWhiteSpace($userPath)) {
        $parts = $userPath -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
    }

    if ($parts -contains $PathItem) {
        return
    }

    $newPath = if ([string]::IsNullOrWhiteSpace($userPath)) {
        $PathItem
    } else {
        "$userPath;$PathItem"
    }
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
}

# This repo is currently configured for x86_64-pc-windows-gnu on this Windows
# host. Keep these defaults local to the install process and allow callers to
# override them if they already set a different Rust environment.
if (-not $env:RUSTUP_HOME -and (Test-Path -LiteralPath "D:\rust\rustup")) {
    $env:RUSTUP_HOME = "D:\rust\rustup"
}
if (-not $env:CARGO_HOME -and (Test-Path -LiteralPath "D:\rust\cargo")) {
    $env:CARGO_HOME = "D:\rust\cargo"
}

Prepend-PathIfExists (Join-Path $env:USERPROFILE ".cargo\bin")
if ($env:CARGO_HOME) {
    Prepend-PathIfExists (Join-Path $env:CARGO_HOME "bin")
}
Prepend-PathIfExists "D:\software\gcc\mingw64\bin"

$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $cargo) {
    throw "cargo was not found. Install Rust or add cargo.exe to PATH first."
}

New-Item -ItemType Directory -Force -Path $BinDir | Out-Null

Write-Host "Installing orchester to $BinDir ..."
& $cargo.Source install --path $PackagePath --force --root $InstallRoot
if ($LASTEXITCODE -ne 0) {
    throw "cargo install failed with exit code $LASTEXITCODE"
}

$Installed = Join-Path $BinDir "orchester.exe"
if (-not (Test-Path -LiteralPath $Installed)) {
    throw "install completed but $Installed was not found"
}

if (-not $NoPathUpdate) {
    Ensure-UserPath $BinDir
    Prepend-PathIfExists $BinDir
}

Write-Host ""
Write-Host "Installed:"
Write-Host "  $Installed"
Write-Host ""
Write-Host "Version check:"
& $Installed --version
if ($LASTEXITCODE -ne 0) {
    throw "orchester --version failed with exit code $LASTEXITCODE"
}
Write-Host ""
Write-Host "You can now run 'orchester' from any new terminal."
