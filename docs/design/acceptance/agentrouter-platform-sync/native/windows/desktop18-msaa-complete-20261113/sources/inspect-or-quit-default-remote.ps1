[CmdletBinding()]
param(
 [ValidateSet('Inspect','InspectConsole','Quit')][string]$Action='Inspect',
 [string]$ExpectedParentCreatedAt='',
 [string]$ExpectedParentImageSha256=''
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version 2.0
$targetPid=6908
$parentPid=5216
$targetCreated='2026-11-10T02:29:29.8577484Z'
$targetImage='C:\Users\han\Downloads\OrchesterRemote\orchester.exe'
$targetSha='4569036d09a2279153f9e5fd3a362403212cd39b0ec68481b09fca7afe303cee'
$parentImage=Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
$helper=Join-Path $PSScriptRoot 'OwnedConsoleQuit.cs'
if ((Get-FileHash -LiteralPath $helper -Algorithm SHA256).Hash.ToLowerInvariant() -cne '531a104d8e2075b4cf7f045b2078e7d4af1357704b37cde68bef331128790572') { throw 'Reviewed console helper differs.' }
$target=Get-Process -Id $targetPid -ErrorAction Stop
$parent=Get-Process -Id $parentPid -ErrorAction Stop
$targetRow=Get-CimInstance Win32_Process -Filter ('ProcessId='+$targetPid)
if (-not $targetRow -or $targetRow.ParentProcessId -ne $parentPid -or
    -not [string]::Equals($target.Path,$targetImage,[StringComparison]::OrdinalIgnoreCase) -or
    $target.StartTime.ToUniversalTime().ToString('o') -cne $targetCreated -or
    (Get-FileHash -LiteralPath $targetImage -Algorithm SHA256).Hash.ToLowerInvariant() -cne $targetSha -or
    -not [string]::Equals($parent.Path,$parentImage,[StringComparison]::OrdinalIgnoreCase) -or
    $parent.StartTime.ToUniversalTime() -gt $target.StartTime.ToUniversalTime()) { throw 'Existing default listener identity differs.' }
$parentCreated=$parent.StartTime.ToUniversalTime().ToString('o')
$parentSha=(Get-FileHash -LiteralPath $parentImage -Algorithm SHA256).Hash.ToLowerInvariant()
if ($Action -ceq 'Inspect') {
 [ordered]@{action='inspect';targetPid=$targetPid;targetCreatedAt=$targetCreated;targetImage=$targetImage;targetImageSha256=$targetSha;parentPid=$parentPid;parentCreatedAt=$parentCreated;parentImage=$parentImage;parentImageSha256=$parentSha;identityMatched=$true;commandWritten=$false;processesStopped=0} | ConvertTo-Json -Depth 4
 return
}
if ([string]::IsNullOrWhiteSpace($ExpectedParentCreatedAt) -or $parentCreated -cne $ExpectedParentCreatedAt -or
    $parentSha -cne $ExpectedParentImageSha256) { throw 'Reviewed parent identity is required.' }
Add-Type -Path $helper
$result=[Orchester.FinalRuntime.OwnedConsoleQuit]::Run($targetPid,$targetCreated,$targetImage,$targetSha,$parentPid,$parentCreated,$parentImage,$parentSha,($Action -ceq 'Quit'))
$result | ConvertTo-Json -Depth 4
if ($Action -ceq 'Quit' -and -not $result.OriginalHandleExited) { throw 'Ordinary quit was sent; original listener handle has not exited. No process was terminated.' }
