param(
 [Parameter(Mandatory=$true)][System.Diagnostics.Process]$Process,
 [Parameter(Mandatory=$true)][uint32]$ExpectedProcessId,
 [Parameter(Mandatory=$true)][long]$ExpectedCreated,
 [Parameter(Mandatory=$true)][string]$ExpectedImage,
 [Parameter(Mandatory=$true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedSha256
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version 2.0
if (-not ('Orchester.FinalDesktop.NativeWindows' -as [type])) { throw 'The pinned native window helper must be loaded by the owning worker.' }
@([Orchester.FinalDesktop.NativeWindows]::Snapshot($Process,$ExpectedProcessId,$ExpectedCreated,$ExpectedImage,$ExpectedSha256))
