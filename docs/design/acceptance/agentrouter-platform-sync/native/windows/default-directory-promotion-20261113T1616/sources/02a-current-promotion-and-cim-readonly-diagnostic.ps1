$ErrorActionPreference='Stop';Set-StrictMode -Version 2.0
$prepRoot=$PSScriptRoot;$base=Split-Path -Parent $prepRoot;$code=1;$stage='pins'
$result=[ordered]@{schemaVersion=1;status='failed';processSignals=0;installationMutations=0;stateMutations=0;privateExceptionValuesIncluded=$false}
$private=[ordered]@{schemaVersion=1;queries=@();inventories=@()}
function New-Json([string]$Path,$Value){$b=([Text.UTF8Encoding]::new($false)).GetBytes((ConvertTo-Json -InputObject $Value -Depth 12)+"`n");$s=[IO.File]::Open($Path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None);try{$s.Write($b,0,$b.Length);$s.Flush($true)}finally{$s.Dispose()}}
try{
 if((Get-FileHash -LiteralPath (Join-Path $base 'monitor-final-common.ps1') -Algorithm SHA256).Hash.ToLowerInvariant()-cne '2d4e07f3d703043ea9af8fbf6377814f08135253837b4e36b8f075ea3aeb2f4f'){throw 'common-source-pin-differs'}
 . (Join-Path $base 'monitor-final-common.ps1')
 $safeFile=Join-Path $prepRoot 'fresh-offline-backup-safe.json'
 if((File-Hash $safeFile)-cne 'c2277a82ccf1df70c61ac18190fdc803801053f54d36dcbdcff06febd89a090d'){throw 'fresh-backup-safe-pin-differs'}
 $safe=Get-Content -Raw -LiteralPath $safeFile|ConvertFrom-Json
 $mf=Join-Path $prepRoot 'fresh-offline-backup-private.json';if((File-Hash $mf)-cne 'a3b9f604aa32da99f8e1cd08d4671c638cd7b263cfaf9b46717accc6a094e33c'){throw 'fresh-private-backup-pin-differs'}
 $m=Get-Content -Raw -LiteralPath $mf|ConvertFrom-Json
 $stage='readonly-current-original-inventories';$rootResults=@()
 foreach($r in @($m.roots)){
  $exists=Test-Path -LiteralPath $r.source -PathType Container
  $equal=$false;$current=@()
  if($r.exists-and $exists){$current=@(Directory-Inventory $r.source);try{Same-Inventory $r.inventory $current 'current-original-state';$equal=$true}catch{}}
  elseif(!$r.exists-and !$exists){$equal=$true}
  $rootResults+=,[ordered]@{name=$r.name;sourceExists=$exists;sourceFiles=$current.Count;currentEqualsFreshOriginal=$equal}
  $private.inventories+=,[ordered]@{name=$r.name;source=$r.source;inventory=$current}
 }
 $result.roots=$rootResults;$result.allCurrentRootsEqualFreshOriginal=(@($rootResults|Where-Object {!$_.currentEqualsFreshOriginal}).Count-eq 0)
 $result.desktopSiblingBackupExists=Test-Path -LiteralPath ('C:\Users\han\AppData\Local\Orchester-before-'+(Split-Path -Leaf $prepRoot))
 $result.remoteSiblingBackupExists=Test-Path -LiteralPath ('C:\Users\han\Downloads\OrchesterRemote-before-'+(Split-Path -Leaf $prepRoot))
 $result.currentDesktopExeSha256=File-Hash 'C:\Users\han\AppData\Local\Orchester\orchester-desktop.exe'
 $result.currentRemoteExeSha256=File-Hash 'C:\Users\han\Downloads\OrchesterRemote\orchester.exe'
 $stage='readonly-cim-exact-query-diagnostic';$queries=@()
 foreach($timeout in @(5,60)){
  $watch=[Diagnostics.Stopwatch]::StartNew();$q=[ordered]@{operationTimeoutSec=$timeout;returned=$false;rowCount=$null;elapsedMilliseconds=$null;exceptionType=$null;cimStatusName=$null;cimStatusNumeric=$null;hResult=$null}
  try{$rows=@(Get-CimInstance Win32_Process -Filter "Name='orchester-desktop.exe'" -OperationTimeoutSec $timeout -ErrorAction Stop|Where-Object {$_.ExecutablePath-ieq 'C:\Users\han\AppData\Local\Orchester\orchester-desktop.exe'});$q.returned=$true;$q.rowCount=$rows.Count}
  catch{
   $e=$_.Exception;$q.exceptionType=$e.GetType().FullName;$q.hResult=[int]$e.HResult
   $c=$e;while($c-and $c-isnot [Microsoft.Management.Infrastructure.CimException]){$c=$c.InnerException}
   if($c){$q.cimStatusName=$c.StatusCode.ToString();$q.cimStatusNumeric=[int]$c.StatusCode}
   $private.queries+=,[ordered]@{operationTimeoutSec=$timeout;exceptionType=$e.GetType().FullName;exceptionText=$e.ToString();scriptStackTrace=$_.ScriptStackTrace;errorRecordText=$_.ToString();statusName=$q.cimStatusName;statusNumeric=$q.cimStatusNumeric;hResult=$q.hResult}
  }
  $q.elapsedMilliseconds=[long]$watch.ElapsedMilliseconds;$queries+=,$q
 }
 $result.readonlyQueries=$queries
 $result.originalFailedExceptionObjectRecovered=$false
 $result.originalFailureCauseEstablished=$false
 $result.status='current-original-installation-and-cim-readonly-observed';$code=0
}catch{$result.stage=$stage;$result.exceptionType=$_.Exception.GetType().FullName}
$result.actualNativeExitCode=$code;$result.observedAtUtc=[DateTime]::UtcNow.ToString('o')
$id=[Guid]::NewGuid().ToString('N');$privatePath=Join-Path $prepRoot ('cim-readonly-'+$id+'-private.json');New-Json $privatePath $private
$result.privateDiagnosticSha256=(Get-FileHash -LiteralPath $privatePath -Algorithm SHA256).Hash.ToLowerInvariant()
New-Json (Join-Path $prepRoot ('cim-readonly-'+$id+'-safe.json')) $result
Write-Output (ConvertTo-Json -InputObject $result -Depth 9 -Compress)
exit $code
