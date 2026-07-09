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

function Test-Command([string]$Name) {
    return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

function Invoke-WingetInstall([string]$Id) {
    if (-not (Test-Command winget)) {
        return $false
    }

    Write-Host "Installing dependency with winget: $Id"
    winget install --id $Id -e --silent --accept-package-agreements --accept-source-agreements
    return ($LASTEXITCODE -eq 0)
}

function Install-RustIfMissing {
    if (Test-Command cargo) {
        return
    }

    Write-Host "cargo not found; installing Rust toolchain..."
    if (-not (Invoke-WingetInstall "Rustlang.Rustup")) {
        $rustup = Join-Path $env:TEMP "rustup-init.exe"
        Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $rustup
        & $rustup -y --profile minimal
        if ($LASTEXITCODE -ne 0) {
            throw "rustup-init failed with exit code $LASTEXITCODE"
        }
    }

    Prepend-PathIfExists (Join-Path $env:USERPROFILE ".cargo\bin")
    if (-not (Test-Command cargo)) {
        throw "cargo is still not available after Rust installation"
    }
}

function Install-GitIfMissing {
    if (Test-Command git) {
        return
    }

    Write-Host "git not found; installing Git..."
    if (-not (Invoke-WingetInstall "Git.Git")) {
        Write-Warning "Git could not be installed automatically. Continuing because local install does not require git."
    }
    Prepend-PathIfExists "D:\software\git\Git\cmd"
    Prepend-PathIfExists "C:\Program Files\Git\cmd"
}

function Find-Gcc {
    $candidates = @(
        "D:\software\gcc\mingw64\bin\gcc.exe",
        "D:\software\msys\msys2\mingw64\bin\gcc.exe",
        "C:\msys64\mingw64\bin\gcc.exe"
    )
    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate) {
            return $candidate
        }
    }

    $cmd = Get-Command gcc -ErrorAction SilentlyContinue
    if ($cmd) {
        return $cmd.Source
    }
    return $null
}

function Find-Ar([string]$GccPath) {
    $sameDir = Join-Path (Split-Path -Parent $GccPath) "ar.exe"
    if (Test-Path -LiteralPath $sameDir) {
        return $sameDir
    }

    $cmd = Get-Command ar -ErrorAction SilentlyContinue
    if ($cmd) {
        return $cmd.Source
    }
    return $null
}

function Install-MingwIfMissing {
    $gcc = Find-Gcc
    if (-not $gcc) {
        Write-Host "MinGW gcc not found; trying to install it..."
        $pacmanCandidates = @(
            "D:\software\msys\msys2\usr\bin\pacman.exe",
            "C:\msys64\usr\bin\pacman.exe"
        )
        $pacman = $pacmanCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
        if ($pacman) {
            & $pacman -Sy --needed --noconfirm mingw-w64-x86_64-gcc
        } else {
            Invoke-WingetInstall "MSYS2.MSYS2" | Out-Null
        }
        Prepend-PathIfExists "D:\software\gcc\mingw64\bin"
        Prepend-PathIfExists "D:\software\msys\msys2\mingw64\bin"
        Prepend-PathIfExists "C:\msys64\mingw64\bin"
        $gcc = Find-Gcc
    }

    if (-not $gcc) {
        throw "MinGW gcc is required for this repository's x86_64-pc-windows-gnu build target and could not be installed automatically"
    }

    $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = $gcc
    $ar = Find-Ar $gcc
    if ($ar) {
        $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_AR = $ar
    }
}

function Use-GnuRustToolchain {
    if (-not (Test-Command rustup)) {
        return
    }

    rustup toolchain install stable-x86_64-pc-windows-gnu --profile minimal --force-non-host | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "failed to install stable-x86_64-pc-windows-gnu toolchain"
    }
    $env:RUSTUP_TOOLCHAIN = "stable-x86_64-pc-windows-gnu"
    $env:CARGO_BUILD_TARGET = "x86_64-pc-windows-gnu"
    rustup target add x86_64-pc-windows-gnu | Out-Null
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

Prepend-PathIfExists (Join-Path $env:USERPROFILE ".cargo\bin")
if ($env:CARGO_HOME) {
    Prepend-PathIfExists (Join-Path $env:CARGO_HOME "bin")
}
Prepend-PathIfExists "D:\software\gcc\mingw64\bin"
Prepend-PathIfExists "D:\software\git\Git\cmd"
Prepend-PathIfExists "C:\Program Files\Git\cmd"

Install-GitIfMissing
Install-RustIfMissing
Install-MingwIfMissing
Use-GnuRustToolchain

$cargo = Get-Command cargo -ErrorAction Stop

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
