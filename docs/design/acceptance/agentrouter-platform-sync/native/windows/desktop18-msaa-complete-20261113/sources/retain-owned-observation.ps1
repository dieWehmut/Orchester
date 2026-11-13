$ErrorActionPreference='Stop'
if(-not ('OrchesterRuntimeRetainerJson' -as [type])){
 Add-Type -TypeDefinition 'using System.Runtime.InteropServices;public static class OrchesterRuntimeRetainerJson{[DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]static extern bool MoveFileEx(string from,string to,uint flags);public static void Publish(string path,string json){string temp=path+"."+System.Guid.NewGuid().ToString("N")+".tmp";using(var file=new System.IO.FileStream(temp,System.IO.FileMode.CreateNew,System.IO.FileAccess.Write,System.IO.FileShare.None)){byte[] bytes=new System.Text.UTF8Encoding(false).GetBytes(json);file.Write(bytes,0,bytes.Length);file.Flush(true);}if(!MoveFileEx(temp,path,9))throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());}}'
}
function Retain-OriginalOwnedObservation {
 param($Registry,$NativeProcess,[string]$ResultDirectory)
 if(-not $Registry -and -not $NativeProcess){return}
 while($true){
  $records=@();$live=@();$observationFailed=$false;$errorType=$null;$nativeLive=$false
  try{
   if($Registry){$records=@($Registry.Identities);foreach($record in $records){if($Registry.IsAlive($record)){$parent=@($records|Where-Object {$_.Pid -eq $record.ParentPid -and $_.Created -le $record.Created}|Sort-Object Created -Descending|Select-Object -First 1);$live += [pscustomobject]@{pid=$record.Pid;created=$record.Created;parentPid=$record.ParentPid;parentCreated=if($parent.Count -eq 1){$parent[0].Created}else{$null};image=$record.Image;sha256=$record.Sha256;originalHandleLive=$true}}}}
   if($NativeProcess){$nativeLive=-not $NativeProcess.HasExited}
  }catch{$observationFailed=$true;$errorType=$_.Exception.GetBaseException().GetType().FullName}
  if(-not $observationFailed -and $live.Count -eq 0 -and -not $nativeLive){return}
  $nativeMetadata=$null
  if($NativeProcess){try{$nativeMetadata=[ordered]@{pid=$NativeProcess.Id;created=if($NativeProcess.PSObject.Properties.Name -contains 'Created'){$NativeProcess.Created}else{$null};originalHandleLive=if($observationFailed){$null}else{$nativeLive}}}catch{}}
  $proof=[ordered]@{status=if($observationFailed){'original-handle-observation-error-retained'}else{'original-owned-handles-still-live-retained'};observedAtUtc=[DateTime]::UtcNow.ToString('o');remainingCount=[int]$live.Count;remaining=$live;nativeProcess=$nativeMetadata;allOriginalHandlesEndedProved=$false;observationFailed=$observationFailed;observationErrorType=$errorType;originalRegistryAndCreatorProcessRetained=$true;systemBrokerSignalled=$false;sharedLifetimeClaimed=$false;signals=0}
  # Persistence failure must not discard the same native creator/registry objects.
  try{[OrchesterRuntimeRetainerJson]::Publish((Join-Path $ResultDirectory 'retained-owned-current-safe.json'),($proof|ConvertTo-Json -Depth 8)+"`n")}catch{}
  Start-Sleep -Milliseconds 500
 }
}
