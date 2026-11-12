$ErrorActionPreference='Stop'
$file='C:\Users\han\AppData\Local\Orchester-build\formal-android-composite-20261112T1520\original-composite-acceptance-safe.json'
$defaultParsed=$false;$utf8Parsed=$false
try{$default=Get-Content -LiteralPath $file -Raw|ConvertFrom-Json;$defaultParsed=$true}catch{}
try{$utf8=Get-Content -LiteralPath $file -Encoding UTF8 -Raw|ConvertFrom-Json;$utf8Parsed=$true}catch{}
$record=[ordered]@{status='fixed-public-composite-default-and-explicit-utf8-parser-comparison';windowsPowerShellMajorVersion=$PSVersionTable.PSVersion.Major;defaultContentEncodingParsed=$defaultParsed;explicitUtf8ContentEncodingParsed=$utf8Parsed;fileSha256=(Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant();originalInputsChanged=$false;gateChanged=$false;privateStateOrRawMessageIncluded=$false}
$bytes=[Text.UTF8Encoding]::new($false).GetBytes(($record|ConvertTo-Json -Depth 5)+"`n")
$stream=[IO.FileStream]::new((Join-Path $PSScriptRoot 'explicit-utf8-parser-result-safe.json'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
try{$stream.Write($bytes,0,$bytes.Length);$stream.Flush($true)}finally{$stream.Dispose()}
Write-Output 'Fixed public composite default versus explicit UTF8 parsing compared without input changes.'
exit 0
