$ErrorActionPreference='Stop'
Set-StrictMode -Version 2.0
$prepRoot=$PSScriptRoot
$base=Split-Path -Parent $prepRoot
$planFile='C:\Users\han\AppData\Local\Orchester-build\monitor-final-20261110\monitors-final21-71def64-20261111\plan.json'
$planSha='8645dca6d20f2d5ff0d845d7c74522f52ca72c6aec49ec9529dcb86a997911cd'
$locked=New-Object 'Collections.Generic.List[IO.FileStream]'
$stage='source-pins';$code=1
$result=[ordered]@{schemaVersion=1;status='failed';sourceRevision='71def643cb08b208cc79687f79255c504eb14dc6';originalFilesModified=$false;processSignals=0;installerExecutions=0;defaultLaunches=0;privateStateValuesIncluded=$false}
function Set-PrivateAcl([string]$Path){
 $sid=[Security.Principal.WindowsIdentity]::GetCurrent().User
 $acl=[Security.AccessControl.DirectorySecurity]::new()
 $acl.SetOwner($sid);$acl.SetAccessRuleProtection($true,$false)
 $flags=[Security.AccessControl.InheritanceFlags]'ContainerInherit,ObjectInherit'
 foreach($value in @($sid.Value,'S-1-5-18','S-1-5-32-544')|Select-Object -Unique){
  $principal=[Security.Principal.SecurityIdentifier]::new($value)
  $rule=[Security.AccessControl.FileSystemAccessRule]::new($principal,[Security.AccessControl.FileSystemRights]::FullControl,$flags,[Security.AccessControl.PropagationFlags]::None,[Security.AccessControl.AccessControlType]::Allow)
  [void]$acl.AddAccessRule($rule)
 }
 ([IO.DirectoryInfo]::new($Path)).SetAccessControl($acl)
 if(!([IO.DirectoryInfo]::new($Path)).GetAccessControl().AreAccessRulesProtected){throw 'private-acl-not-protected'}
}
function Write-NewJson([string]$Path,$Value){
 $bytes=(New-Object Text.UTF8Encoding($false)).GetBytes((ConvertTo-Json -InputObject $Value -Depth 12)+"`n")
 $s=[IO.File]::Open($Path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
 try{$s.Write($bytes,0,$bytes.Length);$s.Flush($true)}finally{$s.Dispose()}
}
function Plain-Files([string]$Path){
 if(([IO.File]::GetAttributes($Path)-band [IO.FileAttributes]::ReparsePoint)-ne 0){throw 'original-root-reparse-point'}
 $files=@();$pending=New-Object 'Collections.Generic.Stack[string]';$pending.Push($Path)
 while($pending.Count -gt 0){
  foreach($item in Get-ChildItem -LiteralPath $pending.Pop() -Force){
   if(($item.Attributes -band [IO.FileAttributes]::ReparsePoint)-ne 0){throw 'original-reparse-point'}
   if($item.PSIsContainer){$pending.Push($item.FullName)}else{$files+=,$item.FullName}
  }
 }
 @($files|Sort-Object -CaseSensitive)
}
function Assert-OriginalOffline{
 $portOwners=@(Get-NetTCPConnection -State Listen -ErrorAction Stop|Where-Object {$_.LocalPort -eq 7338}|Select-Object -ExpandProperty OwningProcess -Unique)
 if($portOwners.Count -ne 0){
  $obs=@();foreach($id in $portOwners){$p=$null;try{$p=[Diagnostics.Process]::GetProcessById([int]$id);$handle=$p.Handle;$obs+=,[ordered]@{pid=[int]$id;exactBirthFileTime=$p.StartTime.ToUniversalTime().ToFileTimeUtc().ToString();handleHeldDuringObservation=($handle-ne [IntPtr]::Zero);hasExited=$p.HasExited}}catch{$obs+=,[ordered]@{pid=[int]$id;identityVerified=$false}}finally{if($p){$p.Dispose()}}}
  $result.current7338Owners=$obs
  throw '7338-listener-present-not-stopped'
 }
 $names=@('orchester-desktop.exe','orchester.exe')
 $running=@(Get-CimInstance Win32_Process -Filter "Name='orchester-desktop.exe' OR Name='orchester.exe'" -ErrorAction Stop)
 if($running.Count -ne 0){$result.currentNamedOrchesterPids=@($running|ForEach-Object {[int]$_.ProcessId});throw 'orchester-process-present-not-stopped'}
}
try{
 if((Get-FileHash -LiteralPath (Join-Path $base 'monitor-final-common.ps1') -Algorithm SHA256).Hash.ToLowerInvariant() -cne '2d4e07f3d703043ea9af8fbf6377814f08135253837b4e36b8f075ea3aeb2f4f'){throw 'common-source-pin-differs'}
 . (Join-Path $base 'monitor-final-common.ps1')
 $plan=Read-Plan $planFile $planSha
 if((File-Hash (Join-Path $plan.runDirectory 'final-result.json')) -cne '7ddcd67633e127a4b664dc69d91cf20bd8751a9ab6a009d8ca73d9b0951fe023'){throw 'final-build-pin-differs'}
 $stage='private-root';Set-PrivateAcl $prepRoot
 if(Test-Path -LiteralPath (Join-Path $prepRoot 'fresh-offline-backup-safe.json')){throw 'backup-receipt-already-exists'}
 $stage='original-offline';Assert-OriginalOffline
 $backupRoot=Join-Path 'C:\Users\han\AppData\Local\Orchester-build\reinstall-backups' ('private-final21-offline-'+(Split-Path -Leaf $prepRoot))
 if(Test-Path -LiteralPath $backupRoot){throw 'backup-directory-already-exists'}
 [void][IO.Directory]::CreateDirectory($backupRoot);Set-PrivateAcl $backupRoot
 $roots=@(
  [ordered]@{name='desktop-installation';source='C:\Users\han\AppData\Local\Orchester'},
  [ordered]@{name='orchester-user-state';source='C:\Users\han\.orchester'},
  [ordered]@{name='desktop-local-webview';source='C:\Users\han\AppData\Local\io.github.diewehmut.orchester'},
  [ordered]@{name='desktop-roaming-profile';source='C:\Users\han\AppData\Roaming\io.github.diewehmut.orchester'},
  [ordered]@{name='remote-installation';source='C:\Users\han\Downloads\OrchesterRemote'},
  [ordered]@{name='remote-state-home';source='C:\Users\han\AppData\Local\OrchesterRemote\state-home'}
 )
 $stage='original-read-locks'
 foreach($r in $roots){
  $r.exists=Test-Path -LiteralPath $r.source -PathType Container
  $r.backup=Join-Path $backupRoot $r.name
  $r.inventory=@()
  if($r.exists){
   foreach($file in @(Plain-Files $r.source)){$locked.Add([IO.File]::Open($file,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read))}
   $r.inventory=@(Directory-Inventory $r.source)
  }
 }
 $result.heldReadLockCount=$locked.Count
 $stage='copy-and-verify'
 foreach($r in $roots){
  if($r.exists){
   [void][IO.Directory]::CreateDirectory($r.backup)
   foreach($item in Get-ChildItem -LiteralPath $r.source -Force){Copy-Item -LiteralPath $item.FullName -Destination $r.backup -Recurse -ErrorAction Stop}
   Same-Inventory $r.inventory (Directory-Inventory $r.backup) 'fresh-offline-backup-copy'
   Same-Inventory $r.inventory (Directory-Inventory $r.source) 'original-source-stable-under-read-locks'
  }elseif(Test-Path -LiteralPath $r.source){throw 'previously-absent-original-root-appeared'}
 }
 Assert-OriginalOffline
 $stage='private-manifest'
 $manifest=[ordered]@{schemaVersion=1;status='fresh-offline-original-backup-verified';sourceRevision=$plan.sourceRevision;expectedPlanSha256=$planSha;observedAtUtc=[DateTime]::UtcNow.ToString('o');backupDirectory=$backupRoot;roots=$roots;locksHeldThroughoutCopy=$true}
 $manifestFile=Join-Path $prepRoot 'fresh-offline-backup-private.json';Write-NewJson $manifestFile $manifest
 $result.status='fresh-offline-original-backup-verified';$result.observedAtUtc=$manifest.observedAtUtc
 $result.backupDirectory=$backupRoot;$result.privateManifestSha256=File-Hash $manifestFile
 $result.roots=@($roots|ForEach-Object {[ordered]@{name=$_.name;exists=$_.exists;files=@($_.inventory).Count;sourceBeforeCopyAndAfterEqual=$true}})
 $result.allExistingRootsVerified=$true;$result.current7338ListenerCount=0;$result.currentNamedOrchesterProcessCount=0;$result.privateAclInheritanceDisabled=$true
 $result.lockReleaseBeforePromotionRequired=$true;$code=0
}catch{$result.stage=$stage;$result.exceptionType=$_.Exception.GetType().FullName}
finally{foreach($s in $locked){$s.Dispose()}}
$result.allOwnedReadLocksReleased=$true;$result.actualNativeExitCode=$code
if($result.status -ceq 'fresh-offline-original-backup-verified'){Write-NewJson (Join-Path $prepRoot 'fresh-offline-backup-safe.json') $result}
else{Write-NewJson (Join-Path $prepRoot ('fresh-offline-backup-failure-'+[Guid]::NewGuid().ToString('N')+'-safe.json')) $result}
Write-Output (ConvertTo-Json -InputObject $result -Depth 8 -Compress)
exit $code
