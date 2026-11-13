[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$OutputFile,
  [Parameter(Mandatory=$true)][int]$ExpectedWorkerPid,
  [Parameter(Mandatory=$true)][ValidatePattern('\A[0-9]+\z')][string]$ExpectedWorkerCreationFiletime,
  [Parameter(Mandatory=$true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedWorkerImageSha256
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version 2.0
$stage='native-worker-handle'
$handles=[Collections.Generic.List[IntPtr]]::new()
$result=$null
$exitCode=1
if(Test-Path -LiteralPath $OutputFile){throw 'New output path required.'}
if(-not ('Orchester.AndroidFiveReadonly.Native' -as [type])){
  Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
namespace Orchester.AndroidFiveReadonly {
  public static class Native {
    [DllImport("kernel32.dll", SetLastError=true)] public static extern IntPtr OpenProcess(uint access, bool inherit, uint pid);
    [DllImport("kernel32.dll", SetLastError=true)] public static extern bool GetProcessTimes(IntPtr handle, out long created, out long exited, out long kernel, out long user);
    [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)] public static extern bool QueryFullProcessImageNameW(IntPtr handle, uint flags, StringBuilder image, ref uint size);
    [DllImport("kernel32.dll", SetLastError=true)] public static extern uint WaitForSingleObject(IntPtr handle, uint milliseconds);
    [DllImport("kernel32.dll", SetLastError=true)] public static extern bool CloseHandle(IntPtr handle);
  }
}
'@
}
function Require([bool]$Condition,[string]$Code){if(-not $Condition){throw $Code}}
function File-Sha([string]$Path){
  Require (Test-Path -LiteralPath $Path -PathType Leaf) 'pinned-file-missing'
  return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}
function Read-Handle([int]$ProcessId){
  $handle=[Orchester.AndroidFiveReadonly.Native]::OpenProcess(0x00101000,$false,[uint32]$ProcessId)
  if($handle -eq [IntPtr]::Zero){
    $errorCode=[Runtime.InteropServices.Marshal]::GetLastWin32Error()
    Require ($errorCode -eq 87) 'native-process-identity-unreadable'
    return $null
  }
  $handles.Add($handle)
  $created=0L;$exited=0L;$kernel=0L;$user=0L
  Require ([Orchester.AndroidFiveReadonly.Native]::GetProcessTimes($handle,[ref]$created,[ref]$exited,[ref]$kernel,[ref]$user)) 'native-process-times-unreadable'
  $wait=[Orchester.AndroidFiveReadonly.Native]::WaitForSingleObject($handle,0)
  Require ($wait -eq 0 -or $wait -eq 258) 'native-process-wait-state-unreadable'
  $buffer=[Text.StringBuilder]::new(32768);$size=[uint32]32768
  Require ([Orchester.AndroidFiveReadonly.Native]::QueryFullProcessImageNameW($handle,0,$buffer,[ref]$size)) 'native-process-image-unreadable'
  return [pscustomobject]@{Handle=$handle;Pid=$ProcessId;CreatedExact=$created.ToString([Globalization.CultureInfo]::InvariantCulture);Live=($wait -eq 258);Image=$buffer.ToString()}
}
try{
  $worker=Read-Handle $ExpectedWorkerPid
  Require ($null -ne $worker -and $worker.Live -and $worker.CreatedExact -ceq $ExpectedWorkerCreationFiletime) 'current-native-worker-identity-differs'
  Require ((File-Sha $worker.Image) -ceq $ExpectedWorkerImageSha256) 'current-native-worker-image-pin-differs'
  $stage='sdk-and-existing-avd-files'
  $pins=@(
    [pscustomobject]@{label='adb';path='C:\Users\han\AppData\Local\Android\Sdk\platform-tools\adb.exe';sha256='b4a6b455702684652cccf7b46258b29e653538904359a58fd4931cf3ef286b3f'},
    [pscustomobject]@{label='emulator';path='C:\Users\han\AppData\Local\Android\Sdk\emulator\emulator.exe';sha256='06af3c5da3c2cef7266822fe653f54371fc746d78db6f4282e1b2338a70970c8'},
    [pscustomobject]@{label='headless-qemu';path='C:\Users\han\AppData\Local\Android\Sdk\emulator\qemu\windows-x86_64\qemu-system-x86_64-headless.exe';sha256='161d3bcef3d5a93698a77f292c899d131d3e92e335c7b5b2360bbc5ce364cf4c'},
    [pscustomobject]@{label='native-python';path='C:\Users\han\AppData\Roaming\uv\python\cpython-3.14.9-windows-x86_64-none\python.exe';sha256='204d51f19707da17766bccecce2ceececadab350663a8190420d1377bac1150d'},
    [pscustomobject]@{label='existing-avd-ini';path='C:\Users\han\.android\avd\Medium_Phone_API_37.0.ini';sha256='47fe0b97b105655cb8c2c56af3432038820090ea307e0f564b68601ffc5d0846'},
    [pscustomobject]@{label='existing-avd-config';path='C:\Users\han\.android\avd\Medium_Phone.avd\config.ini';sha256='2c09c45e0c587d206264ed83ea246215fdd728ce30c035b0804f3b6f1899f4aa'}
  )
  $fileProof=@()
  foreach($pin in $pins){$actual=File-Sha $pin.path;Require ($actual -ceq $pin.sha256) 'sdk-or-existing-avd-file-pin-differs';$fileProof+=@([ordered]@{label=$pin.label;sha256=$actual;exactPinMatch=$true})}
  $ini=[IO.File]::ReadAllText($pins[4].path)
  $config=[IO.File]::ReadAllText($pins[5].path)
  Require ($ini -match '(?m)^path\s*=\s*C:\\Users\\han\\\.android\\avd\\Medium_Phone\.avd\s*$') 'existing-avd-path-differs'
  Require ($config -match '(?m)^AvdId\s*=\s*Medium_Phone_API_37\.0\s*$') 'existing-avd-id-differs'
  Require ($config -match '(?m)^image\.sysdir\.1\s*=\s*system-images\\android-37\.0\\google_apis_playstore\\x86_64\\\s*$') 'existing-avd-system-image-differs'
  $stage='old-exact-process-identities'
  $old=@(
    [pscustomobject]@{role='manager';pid=30436;created='134389406416098295'},
    [pscustomobject]@{role='adb';pid=14648;created='134389406419545296'},
    [pscustomobject]@{role='emulator';pid=29496;created='134389406419932540'},
    [pscustomobject]@{role='qemu';pid=11140;created='134389406420584962'}
  )
  $oldProof=@()
  foreach($pin in $old){
    $current=Read-Handle $pin.pid
    $sameBirth=$null -ne $current -and $current.CreatedExact -ceq $pin.created
    Require (-not ($sameBirth -and $current.Live)) 'an-original-channel-process-is-still-live'
    $oldProof+=@([ordered]@{role=$pin.role;pid=$pin.pid;originalCreationFiletimeExact=$pin.created;pidCurrentlyAbsent=($null -eq $current);originalIdentityCurrentlyLive=$false;pidReusedWithDifferentBirth=($null -ne $current -and -not $sameBirth);matchingOriginalHandleTerminal=($sameBirth -and -not $current.Live)})
  }
  $stage='current-private-tcp-table'
  $ports=@(5039,5572,5573,19329)
  $connections=@(Get-NetTCPConnection -ErrorAction Stop)
  $portProof=@()
  foreach($port in $ports){
    $entries=@($connections|Where-Object {$_.LocalPort -eq $port})
    $portProof+=@([ordered]@{port=$port;currentNativeTcpLocalEndpointCount=$entries.Count;absentFromCurrentNativeTcpTable=($entries.Count -eq 0)})
    Require ($entries.Count -eq 0) 'a-private-channel-port-has-a-current-local-endpoint'
  }
  Require ([Orchester.AndroidFiveReadonly.Native]::WaitForSingleObject($worker.Handle,0) -eq 258) 'native-worker-exited-during-preflight'
  $result=[ordered]@{
    schemaVersion=1;status='readonly-windows-sdk-existing-avd-and-old-channel-preflight-passed';observedAtUtc=[DateTime]::UtcNow.ToString('o');sourceRevision='71def643cb08b208cc79687f79255c504eb14dc6';actualHost='Windows';scriptSha256=(File-Sha $PSCommandPath)
    currentWorker=[ordered]@{pid=$worker.Pid;creationFiletimeExact=$worker.CreatedExact;imageSha256=$ExpectedWorkerImageSha256;queryAndSynchronizeHandleHeldThroughObservation=$true}
    existingAvdName='Medium_Phone_API_37.0';existingAvdIniAndConfigExact=$true;existingAvdPathBoundInMemory=$true;sdkAndAvdFiles=$fileProof;oldExactProcesses=$oldProof;privatePorts=$portProof
    currentProcessHandlesClosedOnlyAfterObservation=$true;portReservationOrFutureAvailabilityClaimed=$false;currentEmulatorOrAdbTargetIdentityVerified=$false;originalDataOrKeystoreOrTokensRead=$false;credentialGetInvoked=$false;adbOrEmulatorCommandsInvoked=0;cdpOrHttpRequests=0;productsStarted=0;processOrInputSignals=0;avdOrAppOrDefaultAdbMutationCount=0;fullWindowsFiveCurrentWireVerified=$false
  }
  $exitCode=0
}catch{
  $result=[ordered]@{schemaVersion=1;status='readonly-windows-android5-preflight-failed';stage=$stage;observedAtUtc=[DateTime]::UtcNow.ToString('o');errorType=$_.Exception.GetType().Name;rawErrorOrPrivateValuesIncluded=$false;productsStarted=0;adbOrEmulatorCommandsInvoked=0;cdpOrHttpRequests=0;processOrInputSignals=0;fullWindowsFiveCurrentWireVerified=$false}
}finally{
  foreach($handle in $handles){$null=[Orchester.AndroidFiveReadonly.Native]::CloseHandle($handle)}
}
$json=($result|ConvertTo-Json -Depth 12)+"`n"
$bytes=[Text.UTF8Encoding]::new($false).GetBytes($json)
$file=[IO.File]::Open($OutputFile,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
try{$file.Write($bytes,0,$bytes.Length)}finally{$file.Dispose()}
[Console]::WriteLine($json)
exit $exitCode
