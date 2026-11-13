$ErrorActionPreference='Stop'
Set-StrictMode -Version 2.0
$handles=[Collections.Generic.List[IntPtr]]::new()
$matched=0;$departed=0;$candidates=0;$official=0;$nonOfficial=0;$submicrosecond=0;$unknown=$false
$safeStage='load-native-api';$safeError=$null;$safeNativeApi=$null;$safeNativeError=$null;$sourceSha=$null
function Fail-Candidate([string]$code,$nativeApi=$null,$nativeError=$null){
 $script:safeError=$code;$script:safeNativeApi=$nativeApi;$script:safeNativeError=$nativeError
 throw $code
}
try {
 Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class AndroidFiveAvdCandidateNative {
 [DllImport("kernel32.dll",SetLastError=true)] public static extern IntPtr OpenProcess(uint a,bool i,uint p);
 [DllImport("kernel32.dll",SetLastError=true)] public static extern bool GetProcessTimes(IntPtr h,out long c,out long e,out long k,out long u);
 [DllImport("kernel32.dll",SetLastError=true)] public static extern uint WaitForSingleObject(IntPtr h,uint m);
 [DllImport("kernel32.dll",SetLastError=true,CharSet=CharSet.Unicode)] public static extern bool QueryFullProcessImageName(IntPtr h,uint f,StringBuilder b,ref uint n);
 [DllImport("kernel32.dll",SetLastError=true)] public static extern bool CloseHandle(IntPtr h);
}
'@
 $safeStage='enumerate-candidate-metadata'
 # Initial enumeration does not request any command line. Only an official
 # SDK image identified through a held native HANDLE gets an argv read below.
 $rows=@(Get-CimInstance -ClassName Win32_Process -Filter "Name = 'emulator.exe' OR Name LIKE 'qemu-system-%.exe'" -Property Name,ProcessId -ErrorAction Stop)
 foreach($row in $rows){
  $candidates++;$safeStage='candidate-open'
  $h=[AndroidFiveAvdCandidateNative]::OpenProcess(0x00101000,$false,[uint32]$row.ProcessId)
  if($h -eq [IntPtr]::Zero){
   $err=[Runtime.InteropServices.Marshal]::GetLastWin32Error()
   if($err -eq 87){$departed++;continue}
   Fail-Candidate 'candidate-identity-unreadable' 'OpenProcess' $err
  }
  $handles.Add($h);$safeStage='candidate-wait'
  $wait=[AndroidFiveAvdCandidateNative]::WaitForSingleObject($h,0)
  if($wait -eq 0){$departed++;continue}
  if($wait -ne 258){$err=[Runtime.InteropServices.Marshal]::GetLastWin32Error();Fail-Candidate 'candidate-wait-unreadable' 'WaitForSingleObject' $err}
  $safeStage='candidate-image';$size=[uint32]32768;$image=[Text.StringBuilder]::new(32768)
  if(-not [AndroidFiveAvdCandidateNative]::QueryFullProcessImageName($h,0,$image,[ref]$size)){
   $err=[Runtime.InteropServices.Marshal]::GetLastWin32Error();Fail-Candidate 'candidate-image-unreadable' 'QueryFullProcessImageNameW' $err
  }
  # Derive the image name from the current held HANDLE, never a stale CIM name.
  $name=[IO.Path]::GetFileName($image.ToString())
  if($name -ieq 'emulator.exe'){$expectedImage='C:\Users\han\AppData\Local\Android\Sdk\emulator\emulator.exe'}
  elseif($name -match '(?i)^qemu-system-[A-Za-z0-9_.-]+\.exe$'){$expectedImage='C:\Users\han\AppData\Local\Android\Sdk\emulator\qemu\windows-x86_64\'+$name}
  else {$nonOfficial++;continue}
  if(-not [string]::Equals($image.ToString(),$expectedImage,[StringComparison]::OrdinalIgnoreCase)){$nonOfficial++;continue}
  $official++;$safeStage='candidate-birth';$c=0L;$e=0L;$k=0L;$u=0L
  if(-not [AndroidFiveAvdCandidateNative]::GetProcessTimes($h,[ref]$c,[ref]$e,[ref]$k,[ref]$u)){
   $err=[Runtime.InteropServices.Marshal]::GetLastWin32Error();Fail-Candidate 'candidate-birth-unreadable' 'GetProcessTimes' $err
  }
  $safeStage='candidate-read-commandline'
  $argvRows=@(Get-CimInstance -ClassName Win32_Process -Filter ('ProcessId = '+[uint32]$row.ProcessId) -Property ProcessId,CreationDate,CommandLine -ErrorAction Stop)
  $safeStage='candidate-final-native-wait';$wait=[AndroidFiveAvdCandidateNative]::WaitForSingleObject($h,0)
  if($wait -eq 0){$departed++;continue}
  if($wait -ne 258){$err=[Runtime.InteropServices.Marshal]::GetLastWin32Error();Fail-Candidate 'candidate-wait-unreadable' 'WaitForSingleObject' $err}
  if($argvRows.Count -ne 1 -or $null -eq $argvRows[0].CreationDate){Fail-Candidate 'candidate-cim-current-identity-unreadable'}
  $safeStage='candidate-compare-birth'
  # DMTF CreationDate has microsecond precision. Subtracting the remainder
  # preserves integer FILETIME arithmetic; floating division would lose ticks.
  $cimBirth=[int64]$argvRows[0].CreationDate.ToUniversalTime().ToFileTimeUtc()
  $nativeRemainder=[int64]($c % 10L);$cimRemainder=[int64]($cimBirth % 10L)
  if($nativeRemainder -ne 0){$submicrosecond++}
  if(($c-$nativeRemainder) -ne ($cimBirth-$cimRemainder)){Fail-Candidate 'candidate-sample-microsecond-identity-differs'}
  if([string]::IsNullOrWhiteSpace($argvRows[0].CommandLine)){Fail-Candidate 'candidate-commandline-unreadable'}
  # Candidate command lines and paths remain in this process's memory only.
  $safeStage='candidate-match-avd';$command=[string]$argvRows[0].CommandLine
  $pathPattern='(?i)C:[\\/]Users[\\/]han[\\/]\.android[\\/]avd[\\/]Medium_Phone\.avd(?=[\\/"\s]|$)'
  $namePattern='(?i)(?<![A-Za-z0-9_.-])Medium_Phone_API_37\.0(?![A-Za-z0-9_.-])'
  if($command -match $pathPattern -or $command -match $namePattern){$matched++}
 }
 $safeStage='source-hash';$sourceSha=(Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256 -ErrorAction Stop).Hash.ToLowerInvariant()
 $safeStage='complete'
}catch {
 $unknown=$true
 if($null -eq $safeError){$safeError='candidate-cim-metadata-or-script-failed';$safeNativeApi=$null;$safeNativeError=$null}
}finally {
 foreach($h in $handles){
  if(-not [AndroidFiveAvdCandidateNative]::CloseHandle($h)){
   $err=[Runtime.InteropServices.Marshal]::GetLastWin32Error();$unknown=$true
   if($null -eq $safeError){$safeStage='release-query-handles';$safeError='candidate-query-handle-close-failed';$safeNativeApi='CloseHandle';$safeNativeError=$err}
  }
 }
}
$passed=(-not $unknown -and $matched -eq 0)
$result=[ordered]@{schemaVersion=1;status=if($passed){'no-current-process-uses-original-medium-avd'}else{'same-avd-running-or-candidate-unreadable'};observedAtUtc=[DateTime]::UtcNow.ToString('o');currentCandidateCount=$candidates;currentOfficialSdkCandidateCount=$official;nonOfficialImageCandidateCount=$nonOfficial;departedCandidateCount=$departed;nativeCreationSubmicrosecondCandidateCount=$submicrosecond;currentSameAvdProcessCount=$matched;candidateReadUnknown=$unknown;safeStage=$safeStage;safeFailureCode=$safeError;safeNativeApi=$safeNativeApi;safeNativeError=$safeNativeError;cimBirthComparisonPrecision='microsecond';rawCommandLinesPathsUsersOrPrivateValuesIncluded=$false;nonOfficialCommandLinesRead=0;otherAvdProcessesSignalled=0;productsStarted=0;sourceSha256=$sourceSha}
[Console]::WriteLine(($result|ConvertTo-Json -Depth 4 -Compress))
if($passed){exit 0}else{exit 1}
