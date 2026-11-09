[CmdletBinding()]
param([string]$FixtureRoot = [System.IO.Path]::GetTempPath())

# Isolated file/ZIP fixtures only. No backend, Agent CLI or network is started.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0
Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
$powerShell = (Get-Process -Id $PID).Path
$tempRoot = Join-Path $FixtureRoot ('orchester-package-test-' + [Guid]::NewGuid().ToString('N'))
$utf8 = New-Object System.Text.UTF8Encoding($false)
$passed = 0

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw "package test: $Message" }
}

function Write-Bytes([string]$Path, [byte[]]$Bytes) {
    New-Item -ItemType Directory -Path (Split-Path -Parent $Path) -Force | Out-Null
    [System.IO.File]::WriteAllBytes($Path, $Bytes)
}

function New-Apk([string]$Path, [System.Collections.IDictionary]$Files, [string[]]$Duplicate = @()) {
    New-Item -ItemType Directory -Path (Split-Path -Parent $Path) -Force | Out-Null
    $archive = [System.IO.Compression.ZipFile]::Open($Path, [System.IO.Compression.ZipArchiveMode]::Create)
    try {
        $archive.CreateEntry('assets/public/assets/') | Out-Null
        foreach ($relative in @($Files.Keys) + $Duplicate) {
            $entry = $archive.CreateEntry('assets/public/' + $relative)
            $stream = $entry.Open()
            try {
                [byte[]]$bytes = $Files[$relative]
                $stream.Write($bytes, 0, $bytes.Length)
            } finally { $stream.Dispose() }
        }
        $archive.CreateEntry('AndroidManifest.xml') | Out-Null
    } finally { $archive.Dispose() }
}

function New-Fixture([string]$Name, [hashtable]$Changes = @{}, [string[]]$Removed = @(), [string[]]$Duplicate = @()) {
    $root = Join-Path $tempRoot $Name
    $remote = Join-Path $root 'werkzeug/remote'
    New-Item -ItemType Directory -Path $remote -Force | Out-Null
    foreach ($script in @('package-windows.ps1', 'verify-windows-package.ps1')) {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $script) -Destination $remote
    }
    foreach ($file in @('portable-start.ps1', 'allow-lan.ps1', 'codex-provider.py')) {
        Write-Bytes (Join-Path $remote $file) ($utf8.GetBytes('fixture; never executed'))
    }
    foreach ($license in @('LICENSE-APACHE', 'LICENSE-MIT')) {
        Write-Bytes (Join-Path $root $license) ($utf8.GetBytes('fixture license'))
    }
    $binary = Join-Path $root 'binary fixture.exe'
    Write-Bytes $binary ([byte[]]@(77, 90, 0, 255, 13, 10))
    $assets = Join-Path $root 'apps/mobile/dist'
    $files = [ordered]@{
        'index.html' = $utf8.GetBytes('<html><script src="assets/main.js"></script></html>')
        'assets/main.js' = $utf8.GetBytes('abc')
        'assets/nested app.bin' = [byte[]]@(0, 1, 13, 10, 127, 128, 255)
        'build-marker' = $utf8.GetBytes('hidden asset')
    }
    foreach ($relative in $files.Keys) { Write-Bytes (Join-Path $assets $relative) $files[$relative] }
    [System.IO.File]::SetAttributes((Join-Path $assets 'build-marker'), [System.IO.FileAttributes]::Hidden)
    $apkFiles = [ordered]@{}
    foreach ($relative in $files.Keys) {
        if ($Removed -notcontains $relative) { $apkFiles[$relative] = $files[$relative] }
    }
    $apkFiles['cordova.js'] = [byte[]]@()
    $apkFiles['cordova_plugins.js'] = [byte[]]@()
    foreach ($relative in $Changes.Keys) { $apkFiles[$relative] = $Changes[$relative] }
    $apk = Join-Path $root 'apps/mobile/android/app/build/outputs/apk/debug/app-debug.apk'
    New-Apk $apk $apkFiles $Duplicate
    return @{
        Root = $root; Binary = $binary; Assets = $assets; Apk = $apk
        Packager = Join-Path $remote 'package-windows.ps1'
        Verifier = Join-Path $remote 'verify-windows-package.ps1'
        Output = Join-Path $root 'new parent/package with spaces'
    }
}

function Invoke-Script([string]$Script, [string[]]$Arguments = @()) {
    $previousPreference = $ErrorActionPreference
    try {
        # Native stderr is an ErrorRecord in Windows PowerShell 5.1.
        $ErrorActionPreference = 'Continue'
        $output = & $powerShell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $Script @Arguments 2>&1 | Out-String
        $exitCode = $LASTEXITCODE
    } finally { $ErrorActionPreference = $previousPreference }
    return @{ ExitCode = $exitCode; Output = $output }
}

function Invoke-Packager($Fixture) {
    return Invoke-Script $Fixture.Packager @('-Binary', $Fixture.Binary, '-OutputDirectory', $Fixture.Output)
}

function Assert-Rejected($Result, [string]$Expected, [string]$Name) {
    Assert-True ($Result.ExitCode -ne 0) "$Name was accepted"
    Assert-True ($Result.Output.Contains($Expected)) "$Name failed for the wrong reason: $($Result.Output)"
    $script:passed++
}

function Test-InputRejection([string]$Name, [hashtable]$Changes = @{}, [string[]]$Removed = @(), [string[]]$Duplicate = @(), [string]$Expected) {
    $fixture = New-Fixture $Name $Changes $Removed $Duplicate
    $result = Invoke-Packager $fixture
    Assert-Rejected $result $Expected $Name
    Assert-True (-not (Test-Path -LiteralPath (Split-Path -Parent $fixture.Output))) "$Name created an output parent or staging directory"
}

function Test-PackageRejection([string]$Name, [scriptblock]$Mutate, [string]$Expected) {
    $candidate = Join-Path $tempRoot ('delivered-' + $Name)
    Copy-Item -LiteralPath $script:valid.Output -Destination $candidate -Recurse -Force
    & $Mutate $candidate
    # Invoke the delivered verifier, with its default package directory.
    $result = Invoke-Script (Join-Path $candidate 'verify.ps1')
    Assert-Rejected $result $Expected $Name
}

try {
    New-Item -ItemType Directory -Path $tempRoot | Out-Null
    foreach ($script in @('package-windows.ps1', 'verify-windows-package.ps1', 'test_package_windows.ps1')) {
        $tokens = $null; $parseErrors = $null
        [System.Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $script), [ref]$tokens, [ref]$parseErrors) | Out-Null
        Assert-True ($parseErrors.Count -eq 0) "$script failed to parse on PowerShell $($PSVersionTable.PSVersion)"
    }

    $script:valid = New-Fixture 'valid'
    $result = Invoke-Packager $valid
    Assert-True ($result.ExitCode -eq 0) "valid package failed: $($result.Output)"
    Assert-True ($result.Output.Trim() -eq $valid.Output) 'packager emitted unexpected output'
    Assert-True ((@(Get-ChildItem -LiteralPath (Split-Path -Parent $valid.Output) -Force)).Count -eq 1) 'packager left staging files'
    $inventory = [System.IO.File]::ReadAllLines((Join-Path $valid.Output 'SHA256SUMS.txt'))
    $expectedFiles = @('orchester.exe', 'start.ps1', 'allow-lan.ps1', 'codex-provider.py', 'verify.ps1', 'LICENSE-APACHE', 'LICENSE-MIT', 'README.md', 'package-info.json', 'orchester-mobile-debug.apk', 'web/index.html', 'web/assets/main.js', 'web/assets/nested app.bin', 'web/build-marker')
    Assert-True ($inventory.Count -eq $expectedFiles.Count) 'inventory omitted or added a delivered file'
    foreach ($relative in $expectedFiles) {
        Assert-True (@($inventory | Where-Object { $_.EndsWith('  ' + $relative, [StringComparison]::Ordinal) }).Count -eq 1) "inventory does not uniquely contain $relative"
    }
    Assert-True ($inventory -contains 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  web/assets/main.js') 'SHA256 inventory failed the abc known vector'
    Assert-True ([Convert]::ToBase64String([System.IO.File]::ReadAllBytes((Join-Path $valid.Output 'orchester.exe'))) -ceq [Convert]::ToBase64String([System.IO.File]::ReadAllBytes($valid.Binary))) 'binary changed during packaging'
    $result = Invoke-Script (Join-Path $valid.Output 'verify.ps1')
    Assert-True ($result.ExitCode -eq 0 -and $result.Output.Contains('14 SHA256 entries and 4 APK/Web files')) "delivered verifier failed: $($result.Output)"
    $passed++

    # Exercise parent segments in the public verifier arguments. Running this
    # suite with an UNC FixtureRoot also verifies the packager's inferred root.
    $result = Invoke-Script $valid.Verifier @('-AssetDirectory', (Join-Path $valid.Assets 'assets/..'), '-Apk', $valid.Apk)
    Assert-True ($result.ExitCode -eq 0 -and $result.Output.Contains('4 files')) "artifact parent-segment path failed: $($result.Output)"
    $passed++
    $result = Invoke-Script (Join-Path $valid.Output 'verify.ps1') @('-PackageDirectory', (Join-Path $valid.Output 'web/..'))
    Assert-True ($result.ExitCode -eq 0 -and $result.Output.Contains('14 SHA256 entries and 4 APK/Web files')) "package parent-segment path failed: $($result.Output)"
    $passed++

    Test-InputRejection -Name 'stale-byte' -Changes @{'assets/main.js' = $utf8.GetBytes('abd')} -Expected 'SHA256 mismatch'
    Test-InputRejection -Name 'missing-file' -Removed @('assets/nested app.bin') -Expected 'missing APK asset'
    Test-InputRejection -Name 'extra-file' -Changes @{'old.js' = $utf8.GetBytes('old bundle')} -Expected 'unexpected APK asset'
    Test-InputRejection -Name 'hidden-mismatch' -Changes @{'build-marker' = $utf8.GetBytes('stale hidden asset')} -Expected 'SHA256 mismatch'
    Test-InputRejection -Name 'nonempty-cordova' -Changes @{'cordova.js' = $utf8.GetBytes('stale plugin')} -Expected 'unexpected APK asset'
    Test-InputRejection -Name 'duplicate-entry' -Duplicate @('index.html') -Expected 'duplicate APK asset'
    Test-InputRejection -Name 'case-mismatch' -Changes @{'Index.html' = $utf8.GetBytes('old index')} -Removed @('index.html') -Expected 'unexpected APK asset'
    $corrupt = New-Fixture 'corrupt-apk'
    [System.IO.File]::WriteAllText($corrupt.Apk, 'not a ZIP archive', $utf8)
    Assert-Rejected (Invoke-Packager $corrupt) 'InvalidDataException' 'corrupt APK'
    Assert-True (-not (Test-Path -LiteralPath (Split-Path -Parent $corrupt.Output))) 'corrupt APK created output directories'

    $existing = New-Fixture 'existing-output'
    Write-Bytes (Join-Path $existing.Output 'keep.txt') ($utf8.GetBytes('keep unchanged'))
    Assert-Rejected (Invoke-Packager $existing) 'OutputDirectory already exists' 'existing output'
    Assert-True ([System.IO.File]::ReadAllText((Join-Path $existing.Output 'keep.txt')) -ceq 'keep unchanged') 'existing output was altered'
    Assert-True (@(Get-ChildItem -LiteralPath $existing.Output -Force).Count -eq 1) 'existing output gained files'
    Assert-True (@(Get-ChildItem -LiteralPath (Split-Path -Parent $existing.Output) -Force).Count -eq 1) 'existing output rejection left staging files'

    Test-PackageRejection 'changed-byte' { param($p) [System.IO.File]::WriteAllText((Join-Path $p 'web/assets/main.js'), 'abd', $utf8) } 'Package SHA256 mismatch'
    Test-PackageRejection 'missing-file' { param($p) Remove-Item -LiteralPath (Join-Path $p 'web/assets/nested app.bin') } 'Inventoried package file is missing'
    Test-PackageRejection 'extra-hidden-file' {
        param($p)
        $path = Join-Path $p 'unlisted.txt'
        [System.IO.File]::WriteAllText($path, 'unlisted', $utf8)
        [System.IO.File]::SetAttributes($path, [System.IO.FileAttributes]::Hidden)
    } 'absent from SHA256SUMS.txt'
    Test-PackageRejection 'missing-inventory-entry' {
        param($p)
        $path = Join-Path $p 'SHA256SUMS.txt'
        [System.IO.File]::WriteAllLines($path, @([System.IO.File]::ReadAllLines($path) | Where-Object { -not $_.EndsWith('  web/build-marker') }), $utf8)
    } 'absent from SHA256SUMS.txt'
    Test-PackageRejection 'duplicate-inventory-entry' {
        param($p)
        $path = Join-Path $p 'SHA256SUMS.txt'
        [System.IO.File]::AppendAllText($path, [System.IO.File]::ReadAllLines($path)[0] + "`n", $utf8)
    } 'Duplicate path'
    Test-PackageRejection 'malformed-inventory' { param($p) [System.IO.File]::AppendAllText((Join-Path $p 'SHA256SUMS.txt'), "bad entry`n", $utf8) } 'Malformed SHA256SUMS.txt'
    Test-PackageRejection 'traversal-inventory' { param($p) [System.IO.File]::AppendAllText((Join-Path $p 'SHA256SUMS.txt'), ('0' * 64) + "  ../outside`n", $utf8) } 'Invalid path'
    Test-PackageRejection 'stale-apk-with-updated-inventory' {
        param($p)
        $apkPath = Join-Path $p 'orchester-mobile-debug.apk'
        Remove-Item -LiteralPath $apkPath
        Copy-Item -LiteralPath (Join-Path $tempRoot 'stale-byte/apps/mobile/android/app/build/outputs/apk/debug/app-debug.apk') -Destination $apkPath
        $newHash = (Get-FileHash -LiteralPath $apkPath -Algorithm SHA256).Hash.ToLowerInvariant()
        $path = Join-Path $p 'SHA256SUMS.txt'
        $lines = @([System.IO.File]::ReadAllLines($path) | ForEach-Object {
            if ($_.EndsWith('  orchester-mobile-debug.apk')) { "$newHash  orchester-mobile-debug.apk" } else { $_ }
        })
        [System.IO.File]::WriteAllLines($path, $lines, $utf8)
    } 'APK/Web assets differ: SHA256 mismatch'

    Write-Output "Windows package tests passed: $passed cases on PowerShell $($PSVersionTable.PSVersion)."
} finally {
    if (Test-Path -LiteralPath $tempRoot) { Remove-Item -LiteralPath $tempRoot -Recurse -Force }
}
