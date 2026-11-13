[CmdletBinding()]
param(
 [Parameter(Mandatory=$true)][string]$PlanFile,
 [Parameter(Mandatory=$true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedPlanSha256,
 [Parameter(Mandatory=$true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedFinalResultSha256,
 [Parameter(Mandatory=$true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedApkSha256,
 [Parameter(Mandatory=$true)][string]$RuntimeInputsFile,
 [Parameter(Mandatory=$true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedRuntimeInputsSha256,
 [Parameter(Mandatory=$true)][string]$ResultDirectory
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version 2.0
. (Join-Path (Split-Path -Parent $PSScriptRoot) 'monitor-final-common.ps1')
$plan = Read-Plan $PlanFile $ExpectedPlanSha256
$resultFile = Join-Path $plan.runDirectory 'final-result.json'
if ((File-Hash $resultFile) -cne $ExpectedFinalResultSha256) { throw 'Reviewed final-result hash differs.' }
$build = Get-Content -Raw -LiteralPath $resultFile | ConvertFrom-Json
$expectedGates = @($plan.commands)
if ($build.status -cne 'build-and-file-gates-passed' -or $build.sourceRevision -cne $plan.sourceRevision -or $build.expectedPlanSha256 -cne $ExpectedPlanSha256 -or (@($build.gates.phase) -join ',') -cne ($expectedGates -join ',') -or @($build.gates | Where-Object { $_.exitCode -ne 0 -or $_.timedOut -or -not $_.processExited }).Count -ne 0) { throw 'Complete native gate set is required.' }
. (Join-Path $PSScriptRoot 'runtime-input-guards.ps1')
$runtimeInputs=Read-FreshRuntimeInputs $plan $build $ExpectedPlanSha256 $ExpectedFinalResultSha256 $RuntimeInputsFile $ExpectedRuntimeInputsSha256
$PackageDirectory=$runtimeInputs.remotePackageCandidate
if ($runtimeInputs.apkSha256 -cne $ExpectedApkSha256) { throw 'Pinned fresh runtime APK differs.' }
Assert-Below $ResultDirectory $plan.runDirectory
if (Test-Path -LiteralPath $ResultDirectory) { throw 'Runtime result directory already exists; inspect the original invocation.' }
New-Item -ItemType Directory -Path $ResultDirectory | Out-Null
$helper = Join-Path $PSScriptRoot 'OwnedRuntimeProcesses.cs'
if ((File-Hash $helper) -cne 'd335743a632acae0c90968e482b87eda44aaf1dae241e938e62b5c7833598d45') { throw 'Owned runtime helper differs.' }
Add-Type -Path $helper
if((File-Hash (Join-Path $PSScriptRoot 'retain-owned-observation.ps1')) -cne '6f758142622e49e7b6e7892c7b254a5dda53b990425d0ff7b9e54d84a7523344'){throw 'Original observation retainer module differs.'}
. (Join-Path $PSScriptRoot 'retain-owned-observation.ps1')
$interactiveHelper=Join-Path $PSScriptRoot 'InteractiveFileRunner.cs'
if((File-Hash $interactiveHelper) -cne '03783e81bee77f1e13d517c5efd4ad6caca45509960691faf5dc90ad8dc3cec3'){throw 'Interactive runner differs.'}
Add-Type -Path $interactiveHelper
$runtimeExe = Join-Path $PackageDirectory 'orchester.exe'
$powershellExe = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
$system32=Join-Path $env:SystemRoot 'System32'
$systemHelperPins=@(
 [pscustomobject]@{path=Join-Path $system32 'icacls.exe';sha256='cb9e55d4c02f4e55100724d2da9a1267f49e196bd39fcdf36cec6ea6314fad2a'},
 [pscustomobject]@{path=$powershellExe;sha256='60c9b29843624dd8af6b6a7b26147753be6aeef65098b9a503cc50850afe410f'},
 [pscustomobject]@{path=Join-Path $system32 'cmd.exe';sha256='15cd209c995b7d3009c91ca40892d5b8f1fb162f3e5f10716642ea278e8e0d7b'},
 [pscustomobject]@{path=Join-Path $system32 'conhost.exe';sha256='c86146188b78e484f32cc29e2331689659943b47689a85570342044f499938ac'}
)
$wslExe=Join-Path $system32 'wsl.exe'
if(Test-Path -LiteralPath $wslExe -PathType Leaf){$systemHelperPins+=[pscustomobject]@{path=$wslExe;sha256='fcd08205b608b5c4831e0dcb1801dd274436cd2c8420815f18ae58a085891a18'}}
$systemHelperPins+=[pscustomobject]@{path='C:\Program Files\WSL\wsl.exe';sha256='051b7a9b7bda66817c052aced998e026610e5d4d18d63f45629d35d5cb5471c8'}
$systemHelperPins+=[pscustomobject]@{path='C:\Program Files\WSL\wslhost.exe';sha256='1f214fbb69efcd138260c4aa4e35182bfe5371afe4055b59a3133d8f88f9c3b7'}
$wslHostSignature=Get-AuthenticodeSignature -LiteralPath 'C:\Program Files\WSL\wslhost.exe'
if($wslHostSignature.Status.ToString() -cne 'Valid' -or -not $wslHostSignature.SignerCertificate -or $wslHostSignature.SignerCertificate.Subject -cne 'CN=Microsoft Corporation, O=Microsoft Corporation, L=Redmond, S=Washington, C=US'){throw 'Pinned WSL host signature differs.'}
foreach($pin in $systemHelperPins){if((File-Hash $pin.path) -cne $pin.sha256){throw 'Reviewed system helper differs.'}}
$owned = [Orchester.FinalRuntime.OwnedProcesses]::new(@($runtimeExe)+@($systemHelperPins.path))
$owned.RegisterPassiveWslHostObservation()
$script:stage = 'startup'
$script:cleanup = $null
$script:lastAssertion = $null
Add-Type -AssemblyName System.Net.Http
$utf8=New-Object System.Text.UTF8Encoding($false)
$tempRoot=Join-Path ([System.IO.Path]::GetTempPath()) ('orchester-portable-runtime-'+[Guid]::NewGuid().ToString('N'))
$workspace=Join-Path $tempRoot 'default project'
$selected=Join-Path $tempRoot ('selected '+[char]0x4e2d+[char]0x6587+' project')
$state=Join-Path $tempRoot 'isolated-state'
$proc=$null; $started=$false; $socket=$null; $http=$null; $backendPid=0; $token=$null
$script:transcript=New-Object System.Text.StringBuilder
$script:lastSeq=0
function Assert-True([bool]$Condition,[string]$Message) {
 if(-not $Condition){
  $script:lastAssertion=switch -Exact ($Message) {
   'clean shutdown timed out with unconnected ConPTY' {'shutdown-timeout'}
   'packaged launcher returned a failing shutdown status' {'shutdown-exit-status'}
   'packaged launcher emitted stderr' {'launcher-stderr'}
   'launcher stderr capture did not reach EOF' {'launcher-stderr-eof'}
   'exact owned process handles remained after shutdown' {'owned-handles-remained'}
   'unrecognized runtime descendants were found' {'unknown-descendants'}
   default {'other-assertion'}
  }
  throw ('runtime smoke: '+$Message)
 }
}
function Request-Http([string]$Path,[string]$Method='GET',$Body=$null,[int]$Expected=200,[bool]$Authenticated=$true) {
 $request=New-Object System.Net.Http.HttpRequestMessage ([System.Net.Http.HttpMethod]::new($Method)),([Uri]($script:base+$Path))
 try {
  if($Authenticated -and $script:token){$request.Headers.Authorization=[System.Net.Http.Headers.AuthenticationHeaderValue]::new('Bearer',$script:token)}
  if($null -ne $Body){$request.Content=[System.Net.Http.StringContent]::new(($Body|ConvertTo-Json -Compress -Depth 8),$utf8,'application/json')}
  $response=$http.SendAsync($request).GetAwaiter().GetResult()
  try {
   Assert-True ([int]$response.StatusCode -eq $Expected) ('HTTP '+$Method+' '+$Path+' returned '+[int]$response.StatusCode+', expected '+$Expected)
   $text=$response.Content.ReadAsStringAsync().GetAwaiter().GetResult()
   if($text -and $response.Content.Headers.ContentType.MediaType -eq 'application/json'){return $text|ConvertFrom-Json}
   return $text
  }finally{$response.Dispose()}
 }finally{$request.Dispose()}
}
function Send-Ws($Body) {
 [byte[]]$bytes=$utf8.GetBytes(($Body|ConvertTo-Json -Compress))
 $socket.SendAsync([ArraySegment[byte]]::new($bytes),[System.Net.WebSockets.WebSocketMessageType]::Text,$true,[System.Threading.CancellationToken]::None).GetAwaiter().GetResult()
}
function Receive-Ws {
 $deadline=[System.Threading.CancellationTokenSource]::new(12000)
 $message=[System.IO.MemoryStream]::new()
 [byte[]]$buffer=New-Object byte[] 16384
 try{
  do{
   $received=$socket.ReceiveAsync([ArraySegment[byte]]::new($buffer),$deadline.Token).GetAwaiter().GetResult()
   if($received.MessageType -eq [System.Net.WebSockets.WebSocketMessageType]::Close){return @{type='closed'}}
   $message.Write($buffer,0,$received.Count)
  }while(-not $received.EndOfMessage)
  $data=$utf8.GetString($message.ToArray())|ConvertFrom-Json
  if($data.type -eq 'output'){
   Assert-True ($data.seq -gt $script:lastSeq) 'output replay was not strictly increasing'
   $script:lastSeq=$data.seq
   $script:transcript.Append($data.data)|Out-Null
  }
  if($data.type -eq 'error'){throw ('terminal protocol error: '+$data.code)}
  return $data
 }finally{$message.Dispose();$deadline.Dispose()}
}
function Read-Until([string]$Expected) {
 $limit=[DateTime]::UtcNow.AddSeconds(20)
 while([DateTime]::UtcNow -lt $limit){
  $plain=$script:transcript.ToString() -replace '\x1b\[[0-?]*[ -/]*[@-~]',''
  if($plain.Contains($Expected)){return}
  $message=Receive-Ws
  Assert-True ($message.type -ne 'exit' -and $message.type -ne 'closed') 'terminal exited before expected output'
 }
 throw ('runtime smoke: expected output did not arrive: '+$Expected)
}
try{
 New-Item -ItemType Directory -Path $workspace,$selected -Force|Out-Null
 $reserved=[System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback,0)
 $reserved.Start();$port=$reserved.LocalEndpoint.Port;$reserved.Stop()
 $script:base='http://127.0.0.1:'+$port
 $handler=[System.Net.Http.HttpClientHandler]::new();$handler.UseProxy=$false
 $http=[System.Net.Http.HttpClient]::new($handler);$http.Timeout=[TimeSpan]::FromSeconds(12)
 $stdoutFile=Join-Path $tempRoot 'launcher-stdout-private.raw'
 $stderrFile=Join-Path $tempRoot 'launcher-stderr-private.raw'
 $arguments='-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "'+(Join-Path $PackageDirectory 'start.ps1')+'" -Workspace "'+$workspace+'" -AllowRoot "'+$tempRoot+'" -StateRoot "'+$state+'" -Port '+$port+' -Json'
 $proc=[InteractiveFileRunner]::StartInteractive($powershellExe,$arguments,$PackageDirectory,$stdoutFile,$stderrFile)
 $started=$true
 Assert-True ($proc.Created -gt 0) 'interactive creation identity unavailable'
 $owned.CaptureNativeRoot($proc.RawHandle,$proc.Id,$proc.Created,$powershellExe,(File-Hash $powershellExe))
 $proc.Resume()
 $owned.CaptureAllowedDescendants()
 $startupLimit=[DateTime]::UtcNow.AddSeconds(20);$startupLine=$null
 do{
  Assert-True (-not $proc.HasExited) 'packaged launcher exited before startup'
  $owned.CaptureAllowedDescendants()
  if(Test-Path -LiteralPath $stdoutFile){
   $stream=[IO.FileStream]::new($stdoutFile,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete)
   try{$reader=[IO.StreamReader]::new($stream,$utf8);try{$startupText=$reader.ReadToEnd()}finally{$reader.Dispose()}}finally{$stream.Dispose()}
   if($startupText.Contains("`n")){$startupLine=$startupText.Split("`n")[0]}
  }
  if(-not $startupLine){Start-Sleep -Milliseconds 50}
 }while(-not $startupLine -and [DateTime]::UtcNow -lt $startupLimit)
 Assert-True (-not [string]::IsNullOrWhiteSpace($startupLine)) 'packaged launcher did not emit startup JSON'
 $listening=$startupLine|ConvertFrom-Json
 Assert-True ($listening.event -eq 'listening' -and $listening.address -ceq $base) 'startup used the wrong isolated listener'
 $owned.CaptureAllowedDescendants()
 $backend=@($owned.DirectChildren($owned.Root,$runtimeExe))
 Assert-True ($backend.Count -eq 1) 'could not identify the exact packaged backend child'
 $backendPid=[int]$backend[0].Pid
 $script:stage='http-pairing'
 $html=Request-Http '/' -Authenticated $false
 Assert-True ($html -ceq [System.IO.File]::ReadAllText((Join-Path $PackageDirectory 'web/index.html'),$utf8)) 'HTTP homepage differs from delivered UI'
 Request-Http '/remote/v1/projects' -Expected 401 -Authenticated $false|Out-Null
 $paired=Request-Http '/remote/v1/pair' 'POST' @{code=$listening.pairing_code;device_name='Isolated portable runtime smoke'} -Authenticated $false
 $script:token=$paired.token
 Assert-True (-not [string]::IsNullOrWhiteSpace($token)) 'pairing did not return a token'
 Request-Http '/remote/v1/pair' 'POST' @{code=$listening.pairing_code;device_name='Rejected repeat'} -Expected 429 -Authenticated $false|Out-Null
 $info=Request-Http '/remote/v1/info'
 Assert-True ($info.schema_version -eq 2) 'remote info schema is incorrect'
 $script:stage='project-catalog'
 $projects=Request-Http '/remote/v1/projects'
 Assert-True (@($projects.projects).Count -eq 1) 'isolated state did not start with one project'
 $project=Request-Http '/remote/v1/projects' 'POST' @{path=$selected;name='Selected runtime project'}
 $projects=Request-Http '/remote/v1/projects'
 Assert-True (@($projects.projects).Count -eq 2 -and @($projects.projects|Where-Object {$_.id -eq $project.id}).Count -eq 1) 'selected project was not persisted in the catalog'
 $script:stage='interactive-terminal'
 $terminal=Request-Http '/remote/v1/terminals' 'POST' @{project_id=$project.id;cols=240;rows=24}
 $owned.CaptureAllowedDescendants()
 Assert-True ($terminal.state -eq 'running' -and $terminal.shell -imatch 'cmd') 'Windows ConPTY did not start with cmd'
 $ticket=Request-Http ('/remote/v1/terminals/'+$terminal.id+'/ticket') 'POST' @{after=0}
 $socket=[System.Net.WebSockets.ClientWebSocket]::new()
 $socket.Options.Proxy=$null
 $connectTimeout=[System.Threading.CancellationTokenSource]::new(12000)
 try{$socket.ConnectAsync([Uri]('ws://127.0.0.1:'+$port+$ticket.ws_path),$connectTimeout.Token).GetAwaiter().GetResult()}finally{$connectTimeout.Dispose()}
 $message=Receive-Ws
 Assert-True ($message.type -eq 'ready') 'WebSocket did not confirm readiness'
 Send-Ws @{type='input';data="cd`rset /p ORCHESTER_VALUE=ORCHESTER_PROMPT:`r"}
 Read-Until 'ORCHESTER_PROMPT:'
 $marker='alpha'+[Guid]::NewGuid().ToString('N').Substring(0,12)
 Send-Ws @{type='input';data=($marker+"`recho ORCHESTER_RESULT:%ORCHESTER_VALUE%`r")}
 Read-Until ('ORCHESTER_RESULT:'+$marker)
 Read-Until $selected
 $script:stage='terminal-resize'
 Send-Ws @{type='resize';cols=96;rows=31}
 $resizeLimit=[DateTime]::UtcNow.AddSeconds(10)
 do{
  $snapshot=Request-Http ('/remote/v1/terminals/'+$terminal.id)
  if($snapshot.cols -eq 96 -and $snapshot.rows -eq 31){break}
  Start-Sleep -Milliseconds 50
 }while([DateTime]::UtcNow -lt $resizeLimit)
 Assert-True ($snapshot.cols -eq 96 -and $snapshot.rows -eq 31) 'terminal resize was not applied'
 $script:stage='terminal-drained-close'
 Request-Http ('/remote/v1/terminals/'+$terminal.id) 'DELETE'|Out-Null
 $exitLimit=[DateTime]::UtcNow.AddSeconds(15)
 do{$message=Receive-Ws}while($message.type -ne 'exit' -and $message.type -ne 'closed' -and [DateTime]::UtcNow -lt $exitLimit)
 Assert-True ($message.type -eq 'exit' -and $message.terminal.state -eq 'closed') 'terminal close did not produce a drained exit event'
 $socket.Dispose();$socket=$null
 # A second ConPTY is deliberately left unconnected for shutdown cleanup.
 $script:stage='shutdown-unconnected-terminal'
 $active=Request-Http '/remote/v1/terminals' 'POST' @{project_id=$project.id;cols=80;rows=24}
 $owned.CaptureAllowedDescendants()
 Assert-True ($active.state -eq 'running') 'unconnected shutdown terminal failed to start'
 $proc.WriteLine('quit')
 Assert-True ($proc.WaitForExit(12000)) 'clean shutdown timed out with unconnected ConPTY'
 Assert-True ($proc.ExitCode -eq 0) 'packaged launcher returned a failing shutdown status'
 $passiveLimit=[DateTime]::UtcNow.AddSeconds(5)
 do{$passiveComplete=$owned.AllExited();if(-not $passiveComplete){Start-Sleep -Milliseconds 50}}while(-not $passiveComplete -and [DateTime]::UtcNow -lt $passiveLimit)
 $remaining=@($owned.Identities|Where-Object{$owned.IsAlive($_)})
 Write-JsonFile ([ordered]@{normalBackendExited=$true;backendExit=$proc.ExitCode;passiveWaitSeconds=5;remainingIdentities=$remaining;modelCalls=0}) (Join-Path $ResultDirectory 'normal-shutdown-observation.json')
 Assert-True $passiveComplete 'exact owned process handles remained after shutdown'
 Assert-True ($owned.UnexpectedDescendants -eq 0) 'unrecognized runtime descendants were found'
 $stderrBytes=[IO.File]::ReadAllBytes($stderrFile)
 $stderrHash=File-Hash $stderrFile
 Write-JsonFile ([ordered]@{bytes=$stderrBytes.Length;sha256=$stderrHash;capture='restricted-handle-file';allOwnedExited=$true;modelCalls=0}) (Join-Path $ResultDirectory 'launcher-stderr-diagnostic.json')
 if($stderrBytes.Length -gt 0){Copy-Item -LiteralPath $stderrFile -Destination (Join-Path $ResultDirectory 'launcher-stderr-private.raw')}
 Assert-True ($stderrBytes.Length -eq 0) 'packaged launcher emitted stderr'
 $probe=[System.Net.Sockets.TcpClient]::new()
 try{
  $connection=$probe.ConnectAsync('127.0.0.1',$port)
  try{$connection.GetAwaiter().GetResult();$open=$true}catch{$open=$false}
  Assert-True (-not $open) 'isolated test port remained open'
 }finally{$probe.Dispose()}
 $verified=[ordered]@{event='portable_runtime_verified';source_revision=$plan.sourceRevision;expectedPlanSha256=$ExpectedPlanSha256;expectedFinalResultSha256=$ExpectedFinalResultSha256;expectedRuntimeInputsSha256=$ExpectedRuntimeInputsSha256;exactOwnedHandlesExited=$true;portsReleased=$true;package=$PackageDirectory;binary_sha256=$build.remoteExeSha256;apk_sha256=$runtimeInputs.apkSha256;port=$port;launcher_pid=$proc.Id;backend_pid=$backendPid;backend_exit=0;checks=@('packaged_launcher','html_matches_package','unauthenticated_401','one_time_pairing','project_add_list','selected_unicode_space_cwd','windows_conpty_launch','set_p_interactive_input','ordered_websocket_output','resize_96x31','delete_drained_exit','shutdown_unconnected_terminal','backend_exited','port_released');output_chunks=$lastSeq;model_calls=0;default_installation_changed=$false;stderr_capture='restricted-handle-file';stderr_bytes=$stderrBytes.Length;stderr_sha256=$stderrHash}
 Write-JsonFile $verified (Join-Path $ResultDirectory 'runtime-result.json')
 $verified | ConvertTo-Json -Depth 4
}catch{
 $baseException=$_.Exception.GetBaseException()
 $staticCode=if($baseException.Message -cmatch '\Aowned-(identity-unreadable|wait-failed|identity-mismatch|root-already-set|root-handle-failed|root-image-mismatch|snapshot-failed|snapshot-empty|snapshot-incomplete|descendant-handle-denied)\z'){$baseException.Message}else{'unlisted'}
 Write-JsonFile ([ordered]@{status='failed';stage=$script:stage;assertion=$script:lastAssertion;exceptionType=$_.Exception.GetType().FullName;baseExceptionType=$baseException.GetType().FullName;staticCode=$staticCode;modelCalls=0}) (Join-Path $ResultDirectory 'failure.json')
 [Console]::Error.WriteLine('Portable runtime verification failed at '+$script:stage+' ('+$_.Exception.GetType().Name+')')
 exit 1
}finally{
 try{
 if($socket){$socket.Abort();$socket.Dispose()}
 $fallback = @()
 # The original Process object and redirected stdin remain ours even when
 # duplicating its HANDLE for the cleanup registry fails during startup.
 if($started -and $proc -and -not $proc.HasResumed){$suspendedExited=$proc.TerminateSuspended();Write-JsonFile ([ordered]@{originalSuspendedProcessExited=$suspendedExited;neverResumed=$true;modelCalls=0}) (Join-Path $ResultDirectory 'suspended-start-cleanup.json')}
 if($started -and $proc -and $proc.HasResumed -and -not $proc.HasExited){
  try{$proc.WriteLine('quit')}catch{}
  $null=$proc.WaitForExit(10000)
 }
 if($owned.Root){
  if($owned.IsAlive($owned.Root)){
   $owned.CaptureAllowedDescendants()
   try{$proc.WriteLine('quit')}catch{}
   $null=$proc.WaitForExit(10000)
  }
  # Running children are observed on their original handles until natural exit.
  # Normal CLI quit above is the only running-process shutdown request.
  $script:cleanup=[ordered]@{identityDiagnostic=[Orchester.FinalRuntime.OwnedProcesses]::LastIdentityDiagnostic;ownedIdentities=$owned.Identities;allExactOwnedHandlesExited=$owned.AllExited();unexpectedDescendants=$owned.UnexpectedDescendants;safeUnexpectedImages=$owned.SafeUnexpectedImages;fallbackOutcomes=$fallback;modelCalls=0;defaultServiceStopped=$false}
  Write-JsonFile $script:cleanup (Join-Path $ResultDirectory 'cleanup-result.json')
 }
 if($http){$http.Dispose()}
 $script:token=$null
 if($owned.Root -and $owned.AllExited() -and $owned.UnexpectedDescendants -eq 0 -and (Test-Path -LiteralPath $tempRoot)){Remove-Item -LiteralPath $tempRoot -Recurse -Force}
 }finally{
  # Neither cleanup/query nor receipt failures may discard original live handles.
  Retain-OriginalOwnedObservation $owned $proc $ResultDirectory
  if($proc){$proc.Dispose()}
  $owned.Dispose()
 }
}
