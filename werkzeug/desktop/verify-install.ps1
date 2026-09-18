# Verifies the produced NSIS installer on a clean Windows runner:
# install silently, prove the desktop and Start-menu shortcuts exist, launch the
# installed executable, then uninstall and confirm the traces are gone.
[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)][ValidateSet('x64', 'arm64')][string]$Arch,
  [Parameter(Mandatory = $true)][string]$Version,
  [string]$Installers = 'desktop-installers'
)

$ErrorActionPreference = 'Stop'

$repositoryRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$installerPath = Join-Path $repositoryRoot "$Installers/Orchester_${Version}_${Arch}-setup.exe"
if (-not (Test-Path -LiteralPath $installerPath)) {
  throw "installer not found: $installerPath"
}

$productName = 'Orchester'
# The bundle installs the crate's binary rather than the product name: Tauri
# packages `apps/desktop/src-tauri`'s `[[bin]] name = "orchester-desktop"`, and
# the first dispatch of this workflow failed here looking for `Orchester.exe`.
$binaryName = 'orchester-desktop'
$installDirectory = Join-Path $env:LOCALAPPDATA $productName
$desktopShortcut = Join-Path ([Environment]::GetFolderPath('Desktop')) "$productName.lnk"
$startMenuShortcut = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\$productName.lnk"
$uninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$productName"

function Remove-InstallationTraces {
  if (Test-Path -LiteralPath $installDirectory) {
    $uninstaller = Join-Path $installDirectory 'uninstall.exe'
    if (Test-Path -LiteralPath $uninstaller) {
      Start-Process -FilePath $uninstaller -ArgumentList '/S' -Wait
    }
  }
  foreach ($path in @($desktopShortcut, $startMenuShortcut)) {
    if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force }
  }
  if (Test-Path -LiteralPath $installDirectory) {
    Remove-Item -LiteralPath $installDirectory -Recurse -Force
  }
  if (Test-Path -LiteralPath $uninstallKey) {
    Remove-Item -LiteralPath $uninstallKey -Recurse -Force
  }
}

Remove-InstallationTraces

$install = Start-Process -FilePath $installerPath -ArgumentList '/S' -Wait -PassThru
if ($install.ExitCode -ne 0) { throw "installer exited with $($install.ExitCode)" }

$executable = Join-Path $installDirectory "$binaryName.exe"
if (-not (Test-Path -LiteralPath $executable)) { throw "installation did not place $executable" }

foreach ($shortcut in @($desktopShortcut, $startMenuShortcut)) {
  if (-not (Test-Path -LiteralPath $shortcut)) { throw "shortcut was not created: $shortcut" }
  $target = (New-Object -ComObject WScript.Shell).CreateShortcut($shortcut).TargetPath
  if ($target -ne $executable) { throw "shortcut $shortcut targets $target, expected $executable" }
}

if (-not (Test-Path -LiteralPath $uninstallKey)) { throw 'uninstall registry entry is missing' }

$running = Start-Process -FilePath $executable -PassThru
try {
  $deadline = (Get-Date).AddSeconds(60)
  $window = $null
  while ((Get-Date) -lt $deadline) {
    if ($running.HasExited) { break }
    $window = Get-Process -Id $running.Id -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 }
    if ($window) { break }
    Start-Sleep -Milliseconds 500
  }
  if ($running.HasExited) { throw "installed application exited with $($running.ExitCode)" }
  if (-not $window) { throw 'installed application never opened a window' }
}
finally {
  if (-not $running.HasExited) {
    $null = $running.CloseMainWindow()
    if (-not $running.WaitForExit(30000)) { Stop-Process -Id $running.Id -Force }
  }
}

$uninstaller = Join-Path $installDirectory 'uninstall.exe'
if (-not (Test-Path -LiteralPath $uninstaller)) { throw 'uninstaller is missing' }
$uninstall = Start-Process -FilePath $uninstaller -ArgumentList '/S' -Wait -PassThru
if ($uninstall.ExitCode -ne 0) { throw "uninstaller exited with $($uninstall.ExitCode)" }

foreach ($path in @($desktopShortcut, $startMenuShortcut)) {
  if (Test-Path -LiteralPath $path) { throw "shortcut survived uninstall: $path" }
}
if (Test-Path -LiteralPath $uninstallKey) { throw 'uninstall registry entry survived uninstall' }

Write-Host "verify-install: installed, launched, and removed $productName $Version ($Arch)"
