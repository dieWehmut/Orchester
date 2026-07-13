[CmdletBinding()]
param(
    [string]$InstallRoot = $(
        if ($env:ORCHESTER_INSTALL_ROOT) {
            $env:ORCHESTER_INSTALL_ROOT
        } elseif ($env:USERPROFILE) {
            Join-Path $env:USERPROFILE ".cargo"
        } else {
            throw "could not determine install root; set ORCHESTER_INSTALL_ROOT or use -InstallRoot"
        }
    ),

    [switch]$Purge,

    [switch]$NoPathUpdate
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version 2.0

function Write-UninstallInfo([string]$Message) {
    Write-Host "orchester uninstall: $Message"
}

function Assert-SafeText([string]$Value, [string]$Name) {
    if ($null -eq $Value) {
        throw "receipt value '$Name' is missing"
    }
    foreach ($character in $Value.ToCharArray()) {
        if ([char]::IsControl($character)) {
            throw "receipt value '$Name' contains a control character"
        }
    }
}

function Assert-ReceiptPath([string]$Value, [string]$Name) {
    $nativeValue = Convert-PosixDrivePath $Value
    Assert-SafeText $nativeValue $Name
    if ([string]::IsNullOrWhiteSpace($nativeValue) -or -not [System.IO.Path]::IsPathRooted($nativeValue)) {
        throw "receipt path '$Name' is not absolute"
    }
    if (($nativeValue -split '[\\/]') -contains '..') {
        throw "receipt path '$Name' contains an unsafe parent component"
    }
}

function Convert-PosixDrivePath([string]$Path) {
    if ($Path -match '^/(?<Drive>[A-Za-z])(?:/(?<Rest>.*))?$') {
        $prefix = $Matches.Drive.ToUpperInvariant() + ":\"
        if ([string]::IsNullOrEmpty($Matches.Rest)) {
            return $prefix
        }
        return $prefix + ($Matches.Rest -replace '/', '\')
    }
    return $Path
}

function Get-NormalizedPath([string]$Path) {
    $Path = Convert-PosixDrivePath $Path
    if ([string]::IsNullOrWhiteSpace($Path)) {
        throw "path cannot be empty"
    }
    $full = [System.IO.Path]::GetFullPath($Path)
    $volumeRoot = [System.IO.Path]::GetPathRoot($full)
    if ($full.Length -gt $volumeRoot.Length) {
        $full = $full.TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar)
    }
    return $full
}

function Test-PathEqual([string]$Left, [string]$Right) {
    return [string]::Equals(
        (Get-NormalizedPath $Left),
        (Get-NormalizedPath $Right),
        [System.StringComparison]::OrdinalIgnoreCase
    )
}

function Test-PathWithin([string]$Path, [string]$Boundary) {
    $normalizedPath = Get-NormalizedPath $Path
    $normalizedBoundary = Get-NormalizedPath $Boundary
    if (Test-PathEqual $normalizedPath $normalizedBoundary) {
        return $true
    }
    $prefix = $normalizedBoundary
    if (-not $prefix.EndsWith([System.IO.Path]::DirectorySeparatorChar.ToString())) {
        $prefix += [System.IO.Path]::DirectorySeparatorChar
    }
    return $normalizedPath.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)
}

function Get-PathItem([string]$Path) {
    try {
        return Get-Item -Force -LiteralPath $Path -ErrorAction Stop
    } catch {
        if ($_.CategoryInfo.Category -eq [System.Management.Automation.ErrorCategory]::ObjectNotFound) {
            return $null
        }
        throw
    }
}

function Test-ReparsePoint($Item) {
    return $null -ne $Item -and [bool]($Item.Attributes -band [System.IO.FileAttributes]::ReparsePoint)
}

function Assert-NoReparseComponents([string]$Path, [string]$Boundary, [string]$Name) {
    if (-not (Test-PathWithin $Path $Boundary)) {
        throw "$Name is outside its allowed boundary"
    }

    $current = Get-NormalizedPath $Path
    $stop = Get-NormalizedPath $Boundary
    while ($true) {
        $item = Get-PathItem $current
        if (Test-ReparsePoint $item) {
            throw "$Name uses a reparse point: $current"
        }
        if (Test-PathEqual $current $stop) {
            break
        }
        $parent = Split-Path -Parent $current
        if ([string]::IsNullOrWhiteSpace($parent) -or (Test-PathEqual $parent $current)) {
            throw "could not validate $Name path boundary"
        }
        $current = $parent
    }
}

function Assert-LowerSha256([string]$Hash, [string]$Name) {
    if ($Hash -notmatch '^[0-9a-f]{64}$') {
        throw "receipt hash '$Name' is not a lowercase SHA-256 digest"
    }
}

function Get-ExpectedConfigDirectory {
    if ($env:ORCHESTER_HOME) {
        return Get-NormalizedPath $env:ORCHESTER_HOME
    }
    if ($env:USERPROFILE) {
        return Get-NormalizedPath (Join-Path $env:USERPROFILE ".orchester")
    }
    return $null
}

function Complete-ProfileRecord($Current, $Profiles) {
    if ($null -eq $Current) {
        return
    }
    if (-not $Current.ContainsKey("path_added")) {
        throw "path_profile is missing path_added"
    }
    if ($Current.path_added -notin @("0", "1")) {
        throw "path_added must be 0 or 1"
    }
    if ($Current.path_added -eq "1" -and -not $Current.ContainsKey("path_line")) {
        throw "path_added=1 is missing path_line"
    }
    if ($Current.ContainsKey("path_marker") -and $Current.path_added -ne "1") {
        throw "path_marker requires path_added=1"
    }
    $Profiles.Add([pscustomobject]@{
        Profile = $Current.path_profile
        Added = $Current.path_added
        Line = if ($Current.ContainsKey("path_line")) { $Current.path_line } else { "" }
        Marker = if ($Current.ContainsKey("path_marker")) { $Current.path_marker } else { "" }
    }) | Out-Null
}

function Read-InstallReceipt([string]$ReceiptPath) {
    $receiptItem = Get-PathItem $ReceiptPath
    if ($null -eq $receiptItem) {
        throw "install receipt does not exist: $ReceiptPath"
    }
    if (Test-ReparsePoint $receiptItem) {
        throw "receipt is a reparse point; refusing to use it: $ReceiptPath"
    }
    if ($receiptItem.PSIsContainer) {
        throw "receipt is not a regular file: $ReceiptPath"
    }
    if ($receiptItem.Length -gt 1048576) {
        throw "receipt is too large"
    }

    $singletons = @{}
    $profiles = [System.Collections.Generic.List[object]]::new()
    $currentProfile = $null
    $lines = [System.IO.File]::ReadAllLines($ReceiptPath)
    if ($lines.Count -gt 1024) {
        throw "receipt contains too many records"
    }

    foreach ($line in $lines) {
        if ([string]::IsNullOrEmpty($line)) {
            throw "receipt contains an empty record"
        }
        $separator = $line.IndexOf("`t", [System.StringComparison]::Ordinal)
        if ($separator -le 0) {
            throw "receipt line is not a TSV key/value pair"
        }
        if ($line.IndexOf("`t", $separator + 1) -ge 0) {
            throw "receipt value contains an extra tab"
        }
        $key = $line.Substring(0, $separator)
        $value = $line.Substring($separator + 1)
        Assert-SafeText $key "key"
        Assert-SafeText $value $key
        if ($key -notmatch '^[a-z_]+$') {
            throw "invalid receipt key: $key"
        }

        if ($key -eq "path_profile") {
            Complete-ProfileRecord $currentProfile $profiles
            if ([string]::IsNullOrWhiteSpace($value)) {
                throw "path_profile cannot be empty"
            }
            $currentProfile = @{ path_profile = $value }
            continue
        }
        if ($key -in @("path_line", "path_added", "path_marker")) {
            if ($null -eq $currentProfile) {
                throw "$key has no path_profile"
            }
            if ($currentProfile.ContainsKey($key)) {
                throw "duplicate $key field"
            }
            $currentProfile[$key] = $value
            continue
        }

        Complete-ProfileRecord $currentProfile $profiles
        $currentProfile = $null
        if ($key -notin @(
            "schema", "install_root", "bin", "binary_hash", "shim", "shim_hash",
            "version", "config_dir", "windows_path_item", "windows_path_added"
        )) {
            throw "unknown receipt field: $key"
        }
        if ($singletons.ContainsKey($key)) {
            throw "duplicate $key field"
        }
        $singletons[$key] = $value
    }
    Complete-ProfileRecord $currentProfile $profiles

    foreach ($required in @("schema", "install_root", "bin", "binary_hash")) {
        if (-not $singletons.ContainsKey($required)) {
            throw "receipt is missing $required"
        }
    }
    if ($singletons.schema -ne "1") {
        throw "unsupported receipt schema"
    }
    Assert-LowerSha256 $singletons.binary_hash "binary_hash"

    $hasShim = $singletons.ContainsKey("shim")
    $hasShimHash = $singletons.ContainsKey("shim_hash")
    if ($hasShim -ne $hasShimHash) {
        throw "shim and shim_hash must be written together"
    }
    if (-not $hasShim) {
        $singletons.shim = ""
        $singletons.shim_hash = ""
    } elseif ($singletons.shim) {
        if (-not $singletons.shim_hash) {
            throw "shim requires a matching shim_hash"
        }
        Assert-LowerSha256 $singletons.shim_hash "shim_hash"
    } elseif ($singletons.shim_hash) {
        throw "non-empty shim_hash requires shim"
    }

    $hasWindowsPath = $singletons.ContainsKey("windows_path_item")
    $hasWindowsPathAdded = $singletons.ContainsKey("windows_path_added")
    if ($hasWindowsPath -ne $hasWindowsPathAdded) {
        throw "windows_path_item and windows_path_added must be written together"
    }
    if (-not $hasWindowsPath) {
        $singletons.windows_path_item = ""
        $singletons.windows_path_added = "0"
    } elseif ($singletons.windows_path_added -notin @("0", "1")) {
        throw "windows_path_added must be 0 or 1"
    } elseif ($singletons.windows_path_added -eq "1" -and -not $singletons.windows_path_item) {
        throw "windows_path_item cannot be empty when added"
    }

    return [pscustomobject]@{
        Values = $singletons
        Profiles = $profiles
    }
}

function Assert-RegularOwnedFile([string]$Path, [string]$ExpectedHash, [string]$Name) {
    $item = Get-PathItem $Path
    if ($null -eq $item) {
        return
    }
    if (Test-ReparsePoint $item) {
        throw "$Name is a reparse point; refusing to remove it: $Path"
    }
    if ($item.PSIsContainer) {
        throw "$Name is not a regular file: $Path"
    }
    $actualHash = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualHash -ne $ExpectedHash) {
        throw "$Name was modified; refusing to remove it: $Path"
    }
}

function Remove-OwnedFile([string]$Path, [string]$ExpectedHash, [string]$Name) {
    Assert-RegularOwnedFile $Path $ExpectedHash $Name
    if ($null -ne (Get-PathItem $Path)) {
        Remove-Item -LiteralPath $Path -Force
    }
}

function Get-SupportedProfilePaths {
    if (-not $env:USERPROFILE) {
        return @()
    }
    return @(
        (Get-NormalizedPath (Join-Path $env:USERPROFILE ".profile")),
        (Get-NormalizedPath (Join-Path $env:USERPROFILE ".bashrc")),
        (Get-NormalizedPath (Join-Path $env:USERPROFILE ".zshrc"))
    )
}

function Assert-ProfileRecord($Profile, [string]$ExpectedBinDirectory) {
    Assert-ReceiptPath $Profile.Profile "path_profile"
    $profilePath = Get-NormalizedPath $Profile.Profile
    $supported = @(Get-SupportedProfilePaths)
    if (-not ($supported | Where-Object { Test-PathEqual $_ $profilePath })) {
        throw "receipt profile is outside the supported user profile set: $profilePath"
    }
    Assert-NoReparseComponents $profilePath (Get-NormalizedPath $env:USERPROFILE) "profile"
    if ($Profile.Added -eq "1") {
        $expectedLine = 'export PATH="' + $ExpectedBinDirectory + ':$PATH"'
        if ($Profile.Line -ne $expectedLine) {
            throw "receipt profile PATH line does not match this install root"
        }
        if ($Profile.Marker -and $Profile.Marker -ne "# Orchester CLI") {
            throw "receipt profile marker is not recognized"
        }
    }
    $item = Get-PathItem $profilePath
    if ($null -ne $item -and $item.PSIsContainer) {
        throw "profile is not a regular file: $profilePath"
    }
}

function Rewrite-Profile($Profile) {
    if ($Profile.Added -ne "1") {
        return
    }
    $path = Get-NormalizedPath $Profile.Profile
    if ($null -eq (Get-PathItem $path)) {
        return
    }
    $lines = [System.Collections.Generic.List[string]]::new()
    foreach ($line in [System.IO.File]::ReadAllLines($path)) {
        $lines.Add($line)
    }
    $targetIndex = $lines.IndexOf($Profile.Line)
    if ($targetIndex -lt 0) {
        return
    }
    if ($Profile.Marker -and $targetIndex -gt 0 -and $lines[$targetIndex - 1] -eq $Profile.Marker) {
        $lines.RemoveAt($targetIndex)
        $lines.RemoveAt($targetIndex - 1)
    } else {
        $lines.RemoveAt($targetIndex)
    }
    $directory = Split-Path -Parent $path
    $temporary = Join-Path $directory ("." + [System.IO.Path]::GetFileName($path) + ".orchester-uninstall." + [guid]::NewGuid().ToString("N"))
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    $contents = if ($lines.Count -gt 0) {
        ($lines -join [Environment]::NewLine) + [Environment]::NewLine
    } else {
        ""
    }
    try {
        [System.IO.File]::WriteAllText($temporary, $contents, $utf8)
        Move-Item -LiteralPath $temporary -Destination $path -Force
    } finally {
        if (Test-Path -LiteralPath $temporary) {
            Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue
        }
    }
}

function Get-NormalizedPathEntry([string]$Entry) {
    $trimmed = $Entry.Trim().TrimEnd('\', '/')
    if ([string]::IsNullOrWhiteSpace($trimmed)) {
        return ""
    }
    if ([System.IO.Path]::IsPathRooted($trimmed) -and -not $trimmed.Contains('%')) {
        try {
            return (Get-NormalizedPath $trimmed).ToLowerInvariant()
        } catch {
            return $trimmed.ToLowerInvariant()
        }
    }
    return $trimmed.ToLowerInvariant()
}

function Remove-WindowsUserPathItem([string]$PathItem) {
    $expected = Get-NormalizedPathEntry $PathItem
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ([string]::IsNullOrWhiteSpace($userPath)) {
        return
    }
    $kept = [System.Collections.Generic.List[string]]::new()
    $removed = $false
    foreach ($part in $userPath.Split([char]';')) {
        if ((Get-NormalizedPathEntry $part) -eq $expected) {
            $removed = $true
        } else {
            $kept.Add($part)
        }
    }
    if (-not $removed) {
        return
    }
    $newPath = $kept -join ';'
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    Write-UninstallInfo "removed $PathItem from the Windows user PATH"
}

function Assert-PurgeTargets([string]$ConfigDirectory) {
    $expected = Get-ExpectedConfigDirectory
    if (-not $expected -or -not (Test-PathEqual $ConfigDirectory $expected)) {
        throw "receipt config_dir is outside the current Orchester home"
    }
    if ($env:USERPROFILE -and (Test-PathWithin $ConfigDirectory $env:USERPROFILE)) {
        Assert-NoReparseComponents $ConfigDirectory $env:USERPROFILE "configuration directory"
    } else {
        $configItem = Get-PathItem $ConfigDirectory
        if (Test-ReparsePoint $configItem) {
            throw "configuration directory is a reparse point"
        }
    }
    $directoryItem = Get-PathItem $ConfigDirectory
    if ($null -ne $directoryItem -and -not $directoryItem.PSIsContainer) {
        throw "configuration path is not a directory: $ConfigDirectory"
    }
    foreach ($name in @("orchester.jsonc", "sessions.jsonl")) {
        $candidate = Join-Path $ConfigDirectory $name
        $item = Get-PathItem $candidate
        if (Test-ReparsePoint $item) {
            throw "configuration file is a reparse point: $candidate"
        }
        if ($null -ne $item -and $item.PSIsContainer) {
            throw "configuration file path is not a regular file: $candidate"
        }
    }
}

function Remove-KnownConfiguration([string]$ConfigDirectory) {
    Assert-PurgeTargets $ConfigDirectory
    foreach ($name in @("orchester.jsonc", "sessions.jsonl")) {
        $candidate = Join-Path $ConfigDirectory $name
        if ($null -ne (Get-PathItem $candidate)) {
            Remove-Item -LiteralPath $candidate -Force
        }
    }
    $directoryItem = Get-PathItem $ConfigDirectory
    if ($null -ne $directoryItem -and $directoryItem.PSIsContainer) {
        $remaining = Get-ChildItem -Force -LiteralPath $ConfigDirectory | Select-Object -First 1
        if ($null -eq $remaining) {
            Remove-Item -LiteralPath $ConfigDirectory -Force
        }
    }
}

function Remove-EmptyDirectory([string]$Path) {
    $item = Get-PathItem $Path
    if ($null -eq $item -or -not $item.PSIsContainer -or (Test-ReparsePoint $item)) {
        return
    }
    $remaining = Get-ChildItem -Force -LiteralPath $Path | Select-Object -First 1
    if ($null -eq $remaining) {
        Remove-Item -LiteralPath $Path -Force
    }
}

function Invoke-OrchesterUninstall {
    $root = Get-NormalizedPath $InstallRoot
    $volumeRoot = Get-NormalizedPath ([System.IO.Path]::GetPathRoot($root))
    if (Test-PathEqual $root $volumeRoot) {
        throw "refusing to operate on a filesystem root"
    }

    $rootItem = Get-PathItem $root
    if (Test-ReparsePoint $rootItem) {
        throw "install root is a reparse point; refusing to use it"
    }
    if ($null -ne $rootItem -and -not $rootItem.PSIsContainer) {
        throw "install root is not a directory: $root"
    }

    $metadataDirectory = Join-Path $root ".orchester"
    $receiptPath = Join-Path $metadataDirectory "install.receipt"
    $receiptItem = Get-PathItem $receiptPath
    if ($null -eq $receiptItem) {
        foreach ($candidate in @(
            (Join-Path $root "bin\orchester.exe"),
            (Join-Path $root "bin\orchester")
        )) {
            if ($null -ne (Get-PathItem $candidate)) {
                throw "installation receipt is missing; refusing to remove $candidate"
            }
        }
        Write-UninstallInfo "no Orchester receipt or installed binary found; nothing to do"
        return
    }

    $metadataItem = Get-PathItem $metadataDirectory
    if (Test-ReparsePoint $metadataItem) {
        throw "receipt directory is a reparse point; refusing to use it"
    }
    Assert-NoReparseComponents $receiptPath $root "receipt"
    $receipt = Read-InstallReceipt $receiptPath
    $values = $receipt.Values

    Assert-ReceiptPath $values.install_root "install_root"
    Assert-ReceiptPath $values.bin "bin"
    $receiptRoot = Get-NormalizedPath $values.install_root
    $binary = Get-NormalizedPath $values.bin
    if (-not (Test-PathEqual $receiptRoot $root)) {
        throw "receipt install_root does not match -InstallRoot"
    }
    $expectedBinary = Get-NormalizedPath (Join-Path $root "bin\orchester.exe")
    if (-not (Test-PathEqual $binary $expectedBinary)) {
        throw "receipt bin is outside the expected Orchester bin path"
    }
    Assert-NoReparseComponents (Split-Path -Parent $binary) $root "binary directory"

    $shim = ""
    if ($values.shim) {
        Assert-ReceiptPath $values.shim "shim"
        if (-not $env:USERPROFILE) {
            throw "USERPROFILE is required to validate the command shim location"
        }
        $shim = Get-NormalizedPath $values.shim
        if ([System.IO.Path]::GetFileName($shim) -ne "orchester.cmd") {
            throw "receipt shim is not an Orchester command shim"
        }
        if (-not (Test-PathWithin $shim $env:USERPROFILE)) {
            throw "receipt shim is outside the current user profile"
        }
        Assert-NoReparseComponents $shim $env:USERPROFILE "shim"
        if (Test-PathEqual $shim $binary) {
            throw "receipt shim and binary paths collide"
        }
    }

    $binDirectory = Get-NormalizedPath (Join-Path $root "bin")
    if ($values.windows_path_item) {
        Assert-ReceiptPath $values.windows_path_item "windows_path_item"
        if (-not (Test-PathEqual $values.windows_path_item $binDirectory)) {
            throw "recorded Windows PATH entry does not point at this install root"
        }
    }

    $seenProfiles = @{}
    foreach ($profile in $receipt.Profiles) {
        $profileKey = (Get-NormalizedPath $profile.Profile).ToLowerInvariant()
        if ($seenProfiles.ContainsKey($profileKey)) {
            throw "duplicate path_profile field: $($profile.Profile)"
        }
        $seenProfiles[$profileKey] = $true
        Assert-ProfileRecord $profile $binDirectory
    }

    $configDirectory = Get-ExpectedConfigDirectory
    if ($values.ContainsKey("config_dir")) {
        Assert-ReceiptPath $values.config_dir "config_dir"
        $configDirectory = Get-NormalizedPath $values.config_dir
        $expectedConfig = Get-ExpectedConfigDirectory
        if (-not $expectedConfig -or -not (Test-PathEqual $configDirectory $expectedConfig)) {
            throw "receipt config_dir is outside the current Orchester home"
        }
    }

    # Finish all validation before changing PATH or deleting files.
    Assert-RegularOwnedFile $binary $values.binary_hash "binary"
    if ($shim) {
        Assert-RegularOwnedFile $shim $values.shim_hash "shim"
    }
    if ($Purge) {
        if (-not $configDirectory) {
            throw "cannot determine configuration directory for -Purge"
        }
        Assert-PurgeTargets $configDirectory
    }

    if ($NoPathUpdate) {
        Write-UninstallInfo "-NoPathUpdate set; leaving recorded PATH entries unchanged"
    } else {
        if ($values.windows_path_added -eq "1") {
            Remove-WindowsUserPathItem $values.windows_path_item
        }
        foreach ($profile in $receipt.Profiles) {
            Rewrite-Profile $profile
        }
    }

    Remove-OwnedFile $binary $values.binary_hash "binary"
    if ($shim) {
        Remove-OwnedFile $shim $values.shim_hash "shim"
    }
    if ($Purge) {
        Remove-KnownConfiguration $configDirectory
    }

    $currentReceipt = Get-PathItem $receiptPath
    if ($null -ne $currentReceipt) {
        if (Test-ReparsePoint $currentReceipt) {
            throw "receipt changed into a reparse point during uninstall"
        }
        Remove-Item -LiteralPath $receiptPath -Force
    }
    Remove-EmptyDirectory $metadataDirectory
    Write-UninstallInfo "Orchester installation removed"
}

try {
    Invoke-OrchesterUninstall
} catch {
    [Console]::Error.WriteLine("orchester uninstall: " + $_.Exception.Message)
    exit 1
}
