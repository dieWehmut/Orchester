# Public review template; native execution is disabled. Original source digest is in provenance-safe.json.
throw 'public-review-template-native-execution-disabled'
$ErrorActionPreference='Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class OrchesterReleaseAudit {
 [DllImport("kernel32.dll",SetLastError=true)] static extern IntPtr OpenProcess(uint access,bool inherit,uint pid);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetProcessTimes(IntPtr h,out long create,out long exit,out long kernel,out long user);
 [DllImport("kernel32.dll",SetLastError=true)] static extern uint WaitForSingleObject(IntPtr h,uint ms);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool CloseHandle(IntPtr h);
 public static object Observe(uint pid,string birth) {
  IntPtr h=OpenProcess(0x1000|0x100000,false,pid);
  if(h==IntPtr.Zero) { int e=Marshal.GetLastWin32Error();if(e!=87)throw new Exception("native-read-unknown");return new {pid=pid,expectedCreatedExact=birth,currentPidAbsent=true,originalProcessAbsent=true,readError=e,queryHandleClosed=true}; }
  long c,e1,k,u;
  try { if(!GetProcessTimes(h,out c,out e1,out k,out u))throw new Exception("native-birth-read-unknown");bool same=c.ToString()==birth;uint wait=WaitForSingleObject(h,0);if(wait!=0&&wait!=258)throw new Exception("native-wait-unknown");return new {pid=pid,expectedCreatedExact=birth,currentPidAbsent=false,birthMatched=same,currentNativeHandleTerminal=wait==0,originalProcessAbsent=!same||wait==0,queryHandleClosed=true}; }
  finally { if(!CloseHandle(h))throw new Exception("native-query-close-unknown"); }
 }
}
'@
$expected=@(@{role='manager';pid=25436;birth='134392957905433988'},@{role='worker';pid=5340;birth='134392926334771025'},@{role='server';pid=28772;birth='134392957914721635'},@{role='head';pid=4080;birth='134392957921995388'},@{role='qemu';pid=33048;birth='134392957922548739'})
$rows=@($expected|ForEach-Object{[ordered]@{role=$_.role;observation=[OrchesterReleaseAudit]::Observe([uint32]$_.pid,$_.birth)}})
$tcp=@(Get-NetTCPConnection -ErrorAction Stop|Where-Object{$_.LocalPort -in @(5041,5572,5573,19330)}|ForEach-Object{[ordered]@{localPort=$_.LocalPort;state=$_.State.ToString();ownerPid=$_.OwningProcess}})
$passed=@($rows|Where-Object{-not $_.observation.originalProcessAbsent}).Count -eq 0 -and $tcp.Count -eq 0
$result=[ordered]@{schemaVersion=1;status=if($passed){'current-original-native-processes-absent-and-private-ports-released'}else{'current-original-release-check-incomplete'};observedAtUtc=[DateTime]::UtcNow.ToString('o');processes=$rows;allFiveOriginalNativeProcessesAbsent=$passed;privatePorts=@(5041,5572,5573,19330);privateTcpRowCount=$tcp.Count;privateTcpRows=$tcp;queryHandlesClosed=$true;processSignals=0;adbOperations=0;avdOperations=0;newJobsSubmitted=0;rawAddressesOrUserValuesIncluded=$false}
[IO.File]::WriteAllText('<USERPROFILE>\AppData\Local\Orchester-build\android-resume-original5-20261119\post-release-current-safe.json',($result|ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
$result|ConvertTo-Json -Depth 8
if(-not $passed){exit 1}
