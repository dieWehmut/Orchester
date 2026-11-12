$ErrorActionPreference='Stop'
Set-StrictMode -Version 2.0
$run='C:\Users\han\AppData\Local\Orchester-build\monitor-final-20261110\monitors-final21-71def64-20261111'
$composite='C:\Users\han\AppData\Local\Orchester-build\formal-android-composite-20261112T1520'
$files=@(
 (Join-Path $run 'fresh-runtime-inputs-unverified-46aed2a5b57c456c9541f2d9481b7d61.json'),
 (Join-Path $composite 'normalized-acceptance-safe.json'),
 (Join-Path $composite 'original-composite-acceptance-safe.json')
)
$observations=@()
foreach($file in $files){
 $parsed=$false;$kind=$null;$label=$null
 try{$value=Get-Content -LiteralPath $file -Raw|ConvertFrom-Json;$parsed=$true}
 catch{
  $exception=$_.Exception.GetBaseException();$kind=$exception.GetType().FullName
  $message=[string]$exception.Message
  # These are public, fixed source receipt files. Publish only a small static
  # parser diagnostic after excluding credential/URL/path-shaped fragments.
  if($message.Length -le 1200 -and $message -notmatch '(?i)(C:\\|/mnt/|/home/|https?://|bearer\s|token\s*[:=]|sk-[A-Za-z0-9]|credential\s*[:=]|eyJ[A-Za-z0-9_-]{10})'){$label=$message}
 }
 $observations+= [pscustomobject]@{fileName=[IO.Path]::GetFileName($file);parsed=$parsed;exceptionType=$kind;publicStaticJsonParserDiagnostic=$label;fileSha256=(Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()}
}
$record=[ordered]@{status='three-fixed-public-receipts-parsed-readonly';observations=$observations;originalInputsModified=$false;credentialsRawLogsOrPrivateStateRead=$false;freezeOrRuntimeInvoked=$false}
$bytes=[Text.UTF8Encoding]::new($false).GetBytes(($record|ConvertTo-Json -Depth 8)+"`n")
$stream=[IO.FileStream]::new((Join-Path $PSScriptRoot 'public-json-parser-result-safe.json'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
try{$stream.Write($bytes,0,$bytes.Length);$stream.Flush($true)}finally{$stream.Dispose()}
Write-Output 'Three selected public receipt JSON parsing observations saved; inputs unchanged.'
exit 0
