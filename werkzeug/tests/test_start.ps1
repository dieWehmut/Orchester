[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

$StartScript = Join-Path (Split-Path -Parent $PSScriptRoot) 'remote/start.ps1'
$PortableStartScript = Join-Path (Split-Path -Parent $PSScriptRoot) 'remote/portable-start.ps1'
$TempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('orchester-start-test-' + [guid]::NewGuid().ToString('N'))
$OriginalCurrentLocation = Get-Location
$OriginalOrchesterHome = $env:ORCHESTER_HOME
$OriginalLocalAppData = $env:LOCALAPPDATA

function Fail([string]$Message) {
    throw "start test: $Message"
}

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) {
        Fail $Message
    }
}

try {
    New-Item -ItemType Directory -Force -Path $TempRoot | Out-Null
    $callerDirectory = Join-Path $TempRoot 'caller'
    $localAppData = Join-Path $TempRoot 'local-app-data'
    $existingHome = Join-Path $TempRoot 'existing-home'
    New-Item -ItemType Directory -Force -Path @($callerDirectory, $localAppData, $existingHome) | Out-Null
    Set-Location -LiteralPath $callerDirectory
    $env:LOCALAPPDATA = $localAppData
    $env:ORCHESTER_HOME = $existingHome

    $existingSentinel = Join-Path $existingHome 'keep.txt'
    $localSentinel = Join-Path $localAppData 'keep.txt'
    [System.IO.File]::WriteAllText($existingSentinel, 'existing state')
    [System.IO.File]::WriteAllText($localSentinel, 'default state')

    $tokens = $null
    $parseErrors = $null
    $ast = [System.Management.Automation.Language.Parser]::ParseFile(
        $StartScript,
        [ref]$tokens,
        [ref]$parseErrors
    )
    Assert-True ($parseErrors.Count -eq 0) 'start script did not parse'

    $source = [System.IO.File]::ReadAllText($StartScript)
    Assert-True ($source -match '\$env:ORCHESTER_HOME\s*=\s*Resolve-RemoteStateRoot\s+-StateRoot\s+\$StateRoot\s+-StateRootWasSpecified\s+\$stateRootWasSpecified') `
        'start script does not apply the resolved state root to ORCHESTER_HOME'

    $resolverAsts = @($ast.FindAll({
        param($node)
        return (
            ($node -is [System.Management.Automation.Language.FunctionDefinitionAst]) -and
            ($node.Name -eq 'Resolve-RemoteStateRoot')
        )
    }, $true))
    Assert-True ($resolverAsts.Count -eq 1) 'state root resolver is missing or duplicated'
    Invoke-Expression $resolverAsts[0].Extent.Text

    $explicitRelative = Resolve-RemoteStateRoot -StateRoot 'explicit-state' -StateRootWasSpecified $true
    $expectedExplicit = [System.IO.Path]::GetFullPath((Join-Path $callerDirectory 'explicit-state'))
    Assert-True ($explicitRelative -eq $expectedExplicit) 'explicit relative StateRoot was not resolved from the caller working directory'

    $environmentRoot = Resolve-RemoteStateRoot -StateRoot $null -StateRootWasSpecified $false
    $expectedEnvironment = [System.IO.Path]::GetFullPath($existingHome)
    Assert-True ($environmentRoot -eq $expectedEnvironment) 'existing ORCHESTER_HOME did not take precedence over the LOCALAPPDATA default'

    $emptyStateRootRejected = $false
    try {
        Resolve-RemoteStateRoot -StateRoot '' -StateRootWasSpecified $true | Out-Null
    } catch {
        $emptyStateRootRejected = $true
    }
    Assert-True $emptyStateRootRejected 'an explicitly empty StateRoot silently fell back to another state directory'

    $env:ORCHESTER_HOME = ''
    $defaultRoot = Resolve-RemoteStateRoot -StateRoot $null -StateRootWasSpecified $false
    $expectedDefault = [System.IO.Path]::GetFullPath((Join-Path $localAppData 'OrchesterRemote/state-home'))
    Assert-True ($defaultRoot -eq $expectedDefault) 'LOCALAPPDATA state default was not selected when ORCHESTER_HOME was absent'

    Assert-True ([System.IO.File]::ReadAllText($existingSentinel) -eq 'existing state') 'existing state marker was modified'
    Assert-True ([System.IO.File]::ReadAllText($localSentinel) -eq 'default state') 'LOCALAPPDATA marker was modified'

    $portableTokens = $null
    $portableParseErrors = $null
    $portableAst = [System.Management.Automation.Language.Parser]::ParseFile(
        $PortableStartScript,
        [ref]$portableTokens,
        [ref]$portableParseErrors
    )
    Assert-True ($portableParseErrors.Count -eq 0) 'portable start script did not parse'

    $portableResolverAsts = @($portableAst.FindAll({
        param($node)
        return (
            ($node -is [System.Management.Automation.Language.FunctionDefinitionAst]) -and
            ($node.Name -eq 'Resolve-RemoteStateRoot')
        )
    }, $true))
    Assert-True ($portableResolverAsts.Count -eq 1) 'portable state root resolver is missing or duplicated'
    Invoke-Expression $portableResolverAsts[0].Extent.Text

    $portableEnvironmentRoot = Resolve-RemoteStateRoot -StateRoot $null -StateRootWasSpecified $false
    Assert-True ($portableEnvironmentRoot -eq $expectedEnvironment) 'portable launcher did not preserve existing ORCHESTER_HOME'
    $portableExplicit = Resolve-RemoteStateRoot -StateRoot 'portable-state' -StateRootWasSpecified $true
    $expectedPortableExplicit = [System.IO.Path]::GetFullPath((Join-Path $callerDirectory 'portable-state'))
    Assert-True ($portableExplicit -eq $expectedPortableExplicit) 'portable launcher did not prioritize explicit StateRoot'

    $portableEmptyRejected = $false
    try {
        Resolve-RemoteStateRoot -StateRoot '' -StateRootWasSpecified $true | Out-Null
    } catch {
        $portableEmptyRejected = $true
    }
    Assert-True $portableEmptyRejected 'portable launcher accepted an explicitly empty StateRoot'

    $env:ORCHESTER_HOME = ''
    $portableDefault = Resolve-RemoteStateRoot -StateRoot $null -StateRootWasSpecified $false
    Assert-True ($portableDefault -eq $expectedDefault) 'portable launcher did not use LOCALAPPDATA when ORCHESTER_HOME was absent'
    Assert-True ([System.IO.File]::ReadAllText($existingSentinel) -eq 'existing state') 'portable launcher modified the existing state marker'

    Write-Host 'start state root tests passed'
} finally {
    Set-Location $OriginalCurrentLocation
    $env:ORCHESTER_HOME = $OriginalOrchesterHome
    $env:LOCALAPPDATA = $OriginalLocalAppData
    if (Test-Path -LiteralPath $TempRoot) {
        Remove-Item -LiteralPath $TempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
