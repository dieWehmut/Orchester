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
        $rustupDir = Join-Path ([System.IO.Path]::GetTempPath()) ("orchester-rustup-" + [guid]::NewGuid().ToString("N"))
        New-Item -ItemType Directory -Force -Path $rustupDir | Out-Null
        try {
            $rustup = Join-Path $rustupDir "rustup-init.exe"
            Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $rustup
            & $rustup -y --profile minimal
            if ($LASTEXITCODE -ne 0) {
                throw "rustup-init failed with exit code $LASTEXITCODE"
            }
        } finally {
            Remove-Item -LiteralPath $rustupDir -Recurse -Force -ErrorAction SilentlyContinue
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
    Prepend-PathIfExists "C:\Program Files\Git\cmd"
}

function Find-Gcc {
    $candidates = @()
    if ($env:ORCHESTER_GCC_PATH) {
        $candidates += $env:ORCHESTER_GCC_PATH
    }
    if ($env:ORCHESTER_MSYS2_ROOT) {
        $candidates += Join-Path $env:ORCHESTER_MSYS2_ROOT "mingw64\bin\gcc.exe"
    }
    $candidates += "C:\msys64\mingw64\bin\gcc.exe"
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
        $pacmanCandidates = @()
        if ($env:ORCHESTER_MSYS2_ROOT) {
            $pacmanCandidates += Join-Path $env:ORCHESTER_MSYS2_ROOT "usr\bin\pacman.exe"
        }
        $pacmanCandidates += "C:\msys64\usr\bin\pacman.exe"
        $pacman = $pacmanCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
        if ($pacman) {
            & $pacman -Sy --needed --noconfirm mingw-w64-x86_64-gcc
        } else {
            Invoke-WingetInstall "MSYS2.MSYS2" | Out-Null
        }
        Prepend-PathIfExists "C:\msys64\mingw64\bin"
        if ($env:ORCHESTER_MSYS2_ROOT) {
            Prepend-PathIfExists (Join-Path $env:ORCHESTER_MSYS2_ROOT "mingw64\bin")
        }
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

    $normalizedPathItem = $PathItem.Trim().TrimEnd('\').ToLowerInvariant()
    foreach ($part in $parts) {
        if ($part.Trim().TrimEnd('\').ToLowerInvariant() -eq $normalizedPathItem) {
            Write-Host "Windows user PATH already includes $PathItem"
            return
        }
    }

    $newPath = if ([string]::IsNullOrWhiteSpace($userPath)) {
        $PathItem
    } else {
        "$userPath;$PathItem"
    }
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    Write-Host "Added $PathItem to Windows user PATH"
}

function Test-PathInProcessPath([string]$PathItem) {
    $normalizedPathItem = $PathItem.Trim().TrimEnd('\').ToLowerInvariant()
    foreach ($part in ($env:Path -split ';')) {
        if ([string]::IsNullOrWhiteSpace($part)) {
            continue
        }
        try {
            $normalizedPart = [System.IO.Path]::GetFullPath($part).TrimEnd('\').ToLowerInvariant()
        } catch {
            $normalizedPart = $part.Trim().TrimEnd('\').ToLowerInvariant()
        }
        if ($normalizedPart -eq $normalizedPathItem) {
            return $true
        }
    }
    return $false
}

function Test-WritableDirectory([string]$PathItem) {
    try {
        if (-not (Test-Path -LiteralPath $PathItem)) {
            New-Item -ItemType Directory -Force -Path $PathItem | Out-Null
        }
        $probe = Join-Path $PathItem ".orchester-write-test-$PID"
        Set-Content -LiteralPath $probe -Value "" -Encoding ASCII
        Remove-Item -LiteralPath $probe -Force
        return $true
    } catch {
        return $false
    }
}

function Ensure-WindowsCommandShim([string]$Target) {
    $shimDir = $null

    if ($env:ORCHESTER_WINDOWS_SHIM_DIR) {
        $shimDir = $env:ORCHESTER_WINDOWS_SHIM_DIR
    } else {
        $candidates = @()
        $localAppData = $env:LOCALAPPDATA
        if ([string]::IsNullOrWhiteSpace($localAppData) -and $env:USERPROFILE) {
            $localAppData = Join-Path $env:USERPROFILE "AppData\Local"
        }
        if ($localAppData) {
            $windowsApps = Join-Path $localAppData "Microsoft\WindowsApps"
            if (Test-WritableDirectory $windowsApps) {
                $shimDir = $windowsApps
            }
        }
        if ([string]::IsNullOrWhiteSpace($shimDir)) {
            if ($env:USERPROFILE) {
                $candidates += Join-Path $env:USERPROFILE "bin"
            }
            foreach ($part in ($env:Path -split ';')) {
                if ([string]::IsNullOrWhiteSpace($part) -or [string]::IsNullOrWhiteSpace($env:USERPROFILE)) {
                    continue
                }
                try {
                    $normalizedPart = [System.IO.Path]::GetFullPath($part).TrimEnd('\').ToLowerInvariant()
                    $normalizedProfile = [System.IO.Path]::GetFullPath($env:USERPROFILE).TrimEnd('\').ToLowerInvariant()
                } catch {
                    continue
                }
                if ($normalizedPart.StartsWith($normalizedProfile)) {
                    $candidates += $part
                }
            }

            $seen = @{}
            foreach ($candidate in $candidates) {
                if ([string]::IsNullOrWhiteSpace($candidate)) {
                    continue
                }
                try {
                    $key = [System.IO.Path]::GetFullPath($candidate).TrimEnd('\').ToLowerInvariant()
                } catch {
                    $key = $candidate.Trim().TrimEnd('\').ToLowerInvariant()
                }
                if ($seen.ContainsKey($key)) {
                    continue
                }
                $seen[$key] = $true
                if ((Test-Path -LiteralPath $candidate) -and (Test-PathInProcessPath $candidate) -and (Test-WritableDirectory $candidate)) {
                    $shimDir = $candidate
                    break
                }
            }
        }
    }

    if ([string]::IsNullOrWhiteSpace($shimDir) -or -not (Test-WritableDirectory $shimDir)) {
        return $null
    }

    $shim = Join-Path $shimDir "orchester.cmd"
    Set-Content -LiteralPath $shim -Value @("@echo off", "`"$Target`" %*") -Encoding ASCII
    return $shim
}

Prepend-PathIfExists (Join-Path $env:USERPROFILE ".cargo\bin")
if ($env:CARGO_HOME) {
    Prepend-PathIfExists (Join-Path $env:CARGO_HOME "bin")
}
Prepend-PathIfExists "C:\Program Files\Git\cmd"
if ($env:ORCHESTER_GCC_PATH) {
    Prepend-PathIfExists (Split-Path -Parent $env:ORCHESTER_GCC_PATH)
}

Install-GitIfMissing
Install-RustIfMissing
Install-MingwIfMissing
Use-GnuRustToolchain

$cargo = Get-Command cargo -ErrorAction Stop

New-Item -ItemType Directory -Force -Path $BinDir | Out-Null

Write-Host "Installing orchester to $BinDir ..."
& $cargo.Source install --locked --path $PackagePath --force --root $InstallRoot
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
    $shim = Ensure-WindowsCommandShim $Installed
    if ($shim) {
        Write-Host "Added Windows command shim: $shim"
    } else {
        Write-Warning "Open a new Windows terminal before running 'orchester' if this terminal cannot find it."
    }
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
if ($NoPathUpdate) {
    Write-Host "PATH update was skipped. Run '$Installed' directly or add '$BinDir' to PATH."
} else {
    Write-Host "You can now run 'orchester' from any new terminal."
}
