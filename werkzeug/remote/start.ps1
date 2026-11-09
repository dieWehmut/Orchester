param(
    [Parameter(Mandatory = $true)][string]$Workspace,
    [string[]]$AllowRoot = @(),
    [string]$Bind = '127.0.0.1',
    [ValidateRange(1, 65535)][int]$Port = 7337,
    [string]$Assets,
    [string]$StateRoot = "$env:LOCALAPPDATA\OrchesterRemote\state-home",
    [string]$TargetDir = "$env:LOCALAPPDATA\OrchesterRemote\target",
    [switch]$SkipBuild,
    [switch]$NoPairing
)
$ErrorActionPreference = 'Stop'
# Path may be provider-qualified for UNC locations; native tools require the
# filesystem path (ProviderPath), including when this script runs from WSL.
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).ProviderPath
$workspacePath = (Resolve-Path -LiteralPath $Workspace).ProviderPath
if (-not (Test-Path -LiteralPath $workspacePath -PathType Container)) {
    throw 'Workspace must be an existing directory.'
}
$allowRootPaths = @()
foreach ($root in $AllowRoot) {
    $rootPath = (Resolve-Path -LiteralPath $root).ProviderPath
    if (-not (Test-Path -LiteralPath $rootPath -PathType Container)) {
        throw 'AllowRoot must contain existing directories.'
    }
    $allowRootPaths += $rootPath
}
if (-not $Assets) { $Assets = Join-Path $repoRoot 'apps/mobile/dist' }
$assetsPath = (Resolve-Path -LiteralPath $Assets).ProviderPath
if (-not (Test-Path -LiteralPath (Join-Path $assetsPath 'index.html') -PathType Leaf)) {
    throw 'Build the mobile UI first: pnpm --filter @orchester/mobile build'
}
if (-not $SkipBuild) {
    & cargo build --locked --manifest-path (Join-Path $repoRoot 'Cargo.toml') `
        --target x86_64-pc-windows-msvc --target-dir $TargetDir -p orchester-konsole
    if ($LASTEXITCODE -ne 0) { throw "Backend build failed ($LASTEXITCODE)." }
}
$binary = Join-Path $TargetDir 'x86_64-pc-windows-msvc/debug/orchester.exe'
if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) { throw "Backend not found: $binary" }
$env:ORCHESTER_HOME = [System.IO.Path]::GetFullPath($StateRoot)
$serveArgs = @('remote', '--workspace', $workspacePath, '--bind', $Bind,
    '--port', "$Port", '--assets', $assetsPath)
foreach ($rootPath in $allowRootPaths) { $serveArgs += @('--allow-root', $rootPath) }
if ($NoPairing) { $serveArgs += '--no-pairing' }
& $binary @serveArgs
exit $LASTEXITCODE
