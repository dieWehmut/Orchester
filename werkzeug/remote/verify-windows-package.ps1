[CmdletBinding(DefaultParameterSetName = 'Package')]
param(
    [Parameter(ParameterSetName = 'Package')][string]$PackageDirectory,
    [Parameter(Mandatory = $true, ParameterSetName = 'Artifacts')][string]$AssetDirectory,
    [Parameter(Mandatory = $true, ParameterSetName = 'Artifacts')][string]$Apk
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

function Get-FileInventory([string]$Directory) {
    $root = (Resolve-Path -LiteralPath $Directory).ProviderPath.TrimEnd('\', '/')
    $files = New-Object 'System.Collections.Generic.Dictionary[string,string]' ([StringComparer]::Ordinal)
    foreach ($file in Get-ChildItem -LiteralPath $root -Recurse -File -Force) {
        $relative = $file.FullName.Substring($root.Length + 1).Replace('\', '/')
        $files.Add($relative, $file.FullName)
    }
    return ,$files
}

function Assert-ApkWebParity([string]$Directory, [string]$ApkPath) {
    if (-not (Test-Path -LiteralPath (Join-Path $Directory 'index.html') -PathType Leaf)) {
        throw "Web assets must contain index.html: $Directory"
    }
    $files = Get-FileInventory $Directory
    $seen = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
    Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
    $archive = [System.IO.Compression.ZipFile]::OpenRead($ApkPath)
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        foreach ($entry in $archive.Entries) {
            if (-not $entry.FullName.StartsWith('assets/public/', [StringComparison]::Ordinal) -or
                $entry.FullName.EndsWith('/', [StringComparison]::Ordinal)) {
                continue
            }
            $relative = $entry.FullName.Substring('assets/public/'.Length)
            if (-not $seen.Add($relative)) {
                throw "APK/Web assets differ: duplicate APK asset '$relative'. Rebuild and sync the mobile app before packaging."
            }
            if (-not $files.ContainsKey($relative)) {
                # Capacitor adds these empty Cordova compatibility stubs during sync.
                # Real Cordova content must also be present in the browser build.
                if (($relative -ceq 'cordova.js' -or $relative -ceq 'cordova_plugins.js') -and $entry.Length -eq 0) {
                    continue
                }
                throw "APK/Web assets differ: unexpected APK asset '$relative'. Rebuild and sync the mobile app before packaging."
            }
            $stream = $entry.Open()
            try {
                $apkHash = [BitConverter]::ToString($sha256.ComputeHash($stream)).Replace('-', '')
            } finally {
                $stream.Dispose()
            }
            $webHash = (Get-FileHash -LiteralPath $files[$relative] -Algorithm SHA256).Hash
            if ($apkHash -cne $webHash) {
                throw "APK/Web assets differ: SHA256 mismatch for '$relative'. Rebuild and sync the mobile app before packaging."
            }
        }
        foreach ($relative in $files.Keys) {
            if (-not $seen.Contains($relative)) {
                throw "APK/Web assets differ: missing APK asset '$relative'. Rebuild and sync the mobile app before packaging."
            }
        }
        return $files.Count
    } finally {
        $sha256.Dispose()
        $archive.Dispose()
    }
}

if ($PSCmdlet.ParameterSetName -eq 'Artifacts') {
    $webFiles = Assert-ApkWebParity (Resolve-Path -LiteralPath $AssetDirectory).ProviderPath (Resolve-Path -LiteralPath $Apk).ProviderPath
    Write-Output "Verified APK/Web parity: $webFiles files."
    return
}

if ([string]::IsNullOrWhiteSpace($PackageDirectory)) { $PackageDirectory = $PSScriptRoot }
$package = (Resolve-Path -LiteralPath $PackageDirectory).ProviderPath
$manifest = Join-Path $package 'SHA256SUMS.txt'
if (-not (Test-Path -LiteralPath $manifest -PathType Leaf)) {
    throw "Package hash inventory is missing: $manifest"
}
$files = Get-FileInventory $package
$files.Remove('SHA256SUMS.txt') | Out-Null
$seen = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
foreach ($line in [System.IO.File]::ReadAllLines($manifest)) {
    if ($line -notmatch '\A([0-9a-fA-F]{64})  (.+)\z') {
        throw "Malformed SHA256SUMS.txt entry: $line"
    }
    $expectedHash = $Matches[1]
    $relative = $Matches[2]
    if ($relative.Contains('\') -or ($relative.Split('/') | Where-Object { $_ -eq '' -or $_ -eq '.' -or $_ -eq '..' })) {
        throw "Invalid path in SHA256SUMS.txt: $relative"
    }
    if (-not $seen.Add($relative)) {
        throw "Duplicate path in SHA256SUMS.txt: $relative"
    }
    if (-not $files.ContainsKey($relative)) {
        throw "Inventoried package file is missing: $relative"
    }
    $actualHash = (Get-FileHash -LiteralPath $files[$relative] -Algorithm SHA256).Hash
    if ($actualHash -ine $expectedHash) {
        throw "Package SHA256 mismatch: $relative"
    }
}
foreach ($relative in $files.Keys) {
    if (-not $seen.Contains($relative)) {
        throw "Package file is absent from SHA256SUMS.txt: $relative"
    }
}
$webFiles = Assert-ApkWebParity (Join-Path $package 'web') (Join-Path $package 'orchester-mobile-debug.apk')
Write-Output "Verified portable package: $($files.Count) SHA256 entries and $webFiles APK/Web files."
