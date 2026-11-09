param(
    [Parameter(Mandatory = $true)][string]$Binary,
    [Parameter(Mandatory = $true)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).ProviderPath
$binaryPath = (Resolve-Path -LiteralPath $Binary).ProviderPath
if (-not (Test-Path -LiteralPath $binaryPath -PathType Leaf)) {
    throw 'Binary must be an existing Windows Orchester executable.'
}
$assets = Join-Path $repoRoot 'apps/mobile/dist'
$apk = Join-Path $repoRoot 'apps/mobile/android/app/build/outputs/apk/debug/app-debug.apk'
foreach ($file in @((Join-Path $assets 'index.html'), $apk)) {
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) {
        throw "Build the backend and mobile artifacts before packaging. Missing: $file"
    }
}
$outputPath = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if (Test-Path -LiteralPath $outputPath) {
    throw "OutputDirectory already exists; choose a new directory. Nothing was overwritten: $outputPath"
}
$parent = Split-Path -Parent $outputPath
New-Item -ItemType Directory -Path $parent -Force | Out-Null
$stage = Join-Path $parent ('.orchester-package-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $stage | Out-Null
$utf8 = New-Object System.Text.UTF8Encoding($false)
try {
    Copy-Item -LiteralPath $binaryPath -Destination (Join-Path $stage 'orchester.exe')
    Copy-Item -LiteralPath $assets -Destination (Join-Path $stage 'web') -Recurse
    Copy-Item -LiteralPath $apk -Destination (Join-Path $stage 'orchester-mobile-debug.apk')
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'portable-start.ps1') -Destination (Join-Path $stage 'start.ps1')
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'allow-lan.ps1') -Destination $stage
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'codex-provider.py') -Destination $stage
    foreach ($license in @('LICENSE-APACHE', 'LICENSE-MIT')) {
        Copy-Item -LiteralPath (Join-Path $repoRoot $license) -Destination $stage
    }
    $readme = @'
# Orchester Remote - Windows portable test build

This folder contains the Windows x64 backend, its browser UI and an Android
debug-signed APK. Copy the whole folder to another Windows 10/11 x64 computer.
No Rust, pnpm or Node is required by the backend itself. Install and sign in to
the Agent CLI you want to control (Codex, OpenCode, etc.) on that computer; its
own runtime requirements still apply. Agent credentials are not included.

Start from PowerShell with an existing project directory:

    powershell -ExecutionPolicy Bypass -File .\start.ps1 -Workspace C:\work\project

The default 127.0.0.1 listener is local to the computer. To connect from phones:

    powershell -ExecutionPolicy Bypass -File .\start.ps1 -Workspace C:\work\project -Bind 0.0.0.0 -Port 7337

Use -AllowRoot C:\work to let the phone browse and select other existing projects
under that directory. PowerShell callers can pass an array of additional roots.
Each Agent task starts in the project selected on the phone; the initial
-Workspace remains the default for older clients. The phone can open separate
work panels for different computers, projects and Agents, and attach file paths
selected from that computer's allowed folders.

Open http://COMPUTER_LAN_IP:7337 in a browser, or install
orchester-mobile-debug.apk on Android and enter that computer's reachable IP
and the pairing code shown by the backend. Codes expire after 10 minutes and
work once. Type `pair` in the computer's terminal for another phone; `devices`,
`revoke DEVICE_ID` and `quit` manage access and stop the backend. A phone can
save more than one computer. Each computer runs its own backend and Agent CLI.

Pairing and task data are stored under
%LOCALAPPDATA%\OrchesterRemote\state-home, never in this package. `-StateRoot`
selects an isolated directory; `-NoPairing` restarts without a new pairing code.
Only one server can use a state directory at a time. `-Json` gives structured
startup messages. UNC project paths are accepted.

Optional: when Codex on this computer uses a compatible API-key model provider,
add -OpenCodeFromCodex to run OpenCode with that provider instead of its own
default model. This option requires Python 3.11+ and reads the local Codex
config/auth; credentials are never included in this package or written to argv.
-CodexConfig and -CodexAuth can select the files. Codex's own login is unchanged.

The launcher does not change firewall settings. If Windows blocks incoming LAN
connections, inspect your network settings and, in an Administrator PowerShell,
use the included helper with the actual IP of this computer:

    .\allow-lan.ps1 -Executable .\orchester.exe -LocalAddress 192.168.1.20 -Port 7337

The rule is restricted to this binary, local IP, port and local subnet. Add
`-Remove` to remove that exact rule. HTTP is for a trusted LAN or VPN; use an
HTTPS reverse proxy for untrusted networks. A local smoke test does not verify
another computer's network or firewall configuration.

Prerequisite: install the Microsoft Visual C++ x64 Redistributable on the target
computer if it is not already installed: https://aka.ms/vc14/vc_redist.x64.exe
This package does not include or install the Visual C++ runtime.

SHA256SUMS.txt lists hashes of all delivered files except itself. For example:

    Get-FileHash .\orchester.exe -Algorithm SHA256

This is a test distribution, not a signed Windows release or Android store
release. Android uses its debug signing key. iOS is not included in this folder.
'@
    [System.IO.File]::WriteAllText((Join-Path $stage 'README.md'), $readme + "`n", $utf8)
    $metadata = [ordered]@{
        format = 'orchester-remote-portable-v1'
        created_at = [DateTime]::UtcNow.ToString('o')
        platform = 'windows-x64'
        mobile_artifact = 'orchester-mobile-debug.apk'
        mobile_signing = 'debug'
        app_local_vcruntime = $false
    } | ConvertTo-Json
    [System.IO.File]::WriteAllText((Join-Path $stage 'package-info.json'), $metadata + "`n", $utf8)
    $hashes = @(Get-ChildItem -LiteralPath $stage -Recurse -File | Sort-Object FullName | ForEach-Object {
        $relative = $_.FullName.Substring($stage.Length + 1).Replace('\', '/')
        $hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        "$hash  $relative"
    })
    [System.IO.File]::WriteAllText((Join-Path $stage 'SHA256SUMS.txt'), ($hashes -join "`n") + "`n", $utf8)
    # Directory.Move fails atomically if another caller created the destination.
    [System.IO.Directory]::Move($stage, $outputPath)
    Write-Output $outputPath
} finally {
    # This directory was created by this invocation only; never remove output.
    if (Test-Path -LiteralPath $stage) {
        Remove-Item -LiteralPath $stage -Recurse -Force
    }
}
