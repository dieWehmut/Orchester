param(
    [Parameter(Mandatory = $true)][string]$Workspace,
    [string[]]$AllowRoot = @(),
    [string]$Bind = '127.0.0.1',
    [ValidateRange(1, 65535)][int]$Port = 7337,
    [string]$StateRoot = "$env:LOCALAPPDATA\OrchesterRemote\state-home",
    [switch]$OpenCodeFromCodex,
    [string]$CodexConfig = "$env:USERPROFILE\.codex\config.toml",
    [string]$CodexAuth = "$env:USERPROFILE\.codex\auth.json",
    [switch]$NoPairing,
    [switch]$Json
)
$ErrorActionPreference = 'Stop'
# ProviderPath preserves native UNC paths without a PowerShell provider prefix.
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
$binary = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot 'orchester.exe')).ProviderPath
$assetsPath = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot 'web')).ProviderPath
if (-not (Test-Path -LiteralPath (Join-Path $assetsPath 'index.html') -PathType Leaf)) {
    throw 'The portable package is incomplete: web/index.html is missing.'
}
$env:ORCHESTER_HOME = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($StateRoot)
$serveArgs = @('remote', '--workspace', $workspacePath, '--bind', $Bind,
    '--port', "$Port", '--assets', $assetsPath)
foreach ($rootPath in $allowRootPaths) { $serveArgs += @('--allow-root', $rootPath) }
if ($NoPairing) { $serveArgs += '--no-pairing' }
if ($Json) { $serveArgs += '--json' }
if ($OpenCodeFromCodex) {
    # This override applies to OpenCode children; it does not change Codex login.
    $helper = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot 'codex-provider.py')).ProviderPath
    & python $helper --config $CodexConfig --auth $CodexAuth -- $binary @serveArgs
} else {
    & $binary @serveArgs
}
exit $LASTEXITCODE
