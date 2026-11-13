[CmdletBinding()]
param(
 [Parameter(Mandatory=$true)][string]$PlanFile,
 [Parameter(Mandatory=$true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedPlanSha256,
 [Parameter(Mandatory=$true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedFinalResultSha256,
 [Parameter(Mandatory=$true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedSharedHelperSha256,
 [Parameter(Mandatory=$true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedCaptureScriptSha256,
 [Parameter(Mandatory=$true)][string]$ResultDirectory,
 [string]$SharedHelperPath='C:\Users\han\AppData\Local\Orchester-build\continue-20261110\prepared-new\OwnedRuntimeProcesses.cs',
 [string]$WebViewExecutable='C:\Program Files (x86)\Microsoft\EdgeWebView\Application\157.0.4341.42\msedgewebview2.exe',
 [ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedWebViewSha256='51b4cd503d669d8f7f6ff6e9e22effeebd28cce11262ddec815972a424474533',
 [ValidateRange(3,30)][int]$CaptureSeconds=6
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version 2.0
$stage='preflight';$resultCreated=$false;$failed=$false;$cleanupOk=$false;$gui=$null;$owned=$null;$started=$false;$guiOut=$null;$guiErr=$null
$embeddedPorts=@();$safeUnknown=@();$knownCodes=@('owned-identity-unreadable','owned-identity-mismatch','owned-wait-failed','owned-root-already-set','owned-root-handle-failed','owned-root-image-mismatch','owned-snapshot-failed','owned-snapshot-empty','owned-snapshot-incomplete','owned-descendant-handle-denied')
function Assert-Capture([bool]$Condition,[string]$Label){if(-not $Condition){throw [InvalidOperationException]::new($Label)}}
function Capture-SafeException([Exception]$Exception){$base=$Exception.GetBaseException();[ordered]@{type=$Exception.GetType().FullName;baseExceptionType=$base.GetType().FullName;staticCode=if($base.Message -cin $knownCodes){$base.Message}else{$null}}}
function Capture-ListeningPorts{@([Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties().GetActiveTcpListeners()|ForEach-Object{$_.Port})}
function Capture-OwnedDescendants{if($owned -and $owned.Root -and $owned.IsAlive($owned.Root)){$owned.CaptureAllowedDescendants()}}
function Capture-SafeUnknownImages{
 # Explicitly project only the new helper's public safe image fields. Full
 # image paths, command lines and arbitrary exception messages never leave it.
 Assert-Capture ($owned.PSObject.Properties.Name -contains 'SafeUnexpectedImages') 'safe-unknown-helper-contract-missing'
 @($owned.SafeUnexpectedImages | ForEach-Object {
  $relative=$_.systemRootRelativePath
  if($null -ne $relative){Assert-Capture (-not [IO.Path]::IsPathRooted([string]$relative) -and $relative -notmatch '(\A|[\\/])\.\.([\\/]|\z)') 'unsafe-system-relative-image'}
  Assert-Capture ($_.basename -eq [IO.Path]::GetFileName([string]$_.basename) -and ($null -eq $_.sha256 -or $_.sha256 -cmatch '\A[0-9a-f]{64}\z')) 'safe-image-fields-invalid'
  [pscustomobject]@{basename=$_.basename;systemRootRelativePath=$relative;sha256=$_.sha256}
 })
}
try{
 . (Join-Path (Split-Path -Parent $PSScriptRoot) 'monitor-final-common.ps1')
 Assert-Capture ((File-Hash $PSCommandPath) -ceq $ExpectedCaptureScriptSha256) 'capture-script-pin-differs'
 $plan=Read-Plan $PlanFile $ExpectedPlanSha256
 $buildFile=Join-Path $plan.runDirectory 'final-result.json';Assert-Capture ((File-Hash $buildFile) -ceq $ExpectedFinalResultSha256) 'final-result-pin-differs'
 $build=Get-Content -Raw -LiteralPath $buildFile|ConvertFrom-Json
 $gates=@('toolchain','monitor-process','monitor-activity','monitor-http','remote-cli','root-clippy','root-fmt','cli-build','chrome-test','desktop-test','desktop-clippy','desktop-fmt','desktop-build')
 Assert-Capture ($build.status -ceq 'build-and-file-gates-passed' -and $build.sourceRevision -ceq $plan.sourceRevision -and $build.expectedPlanSha256 -ceq $ExpectedPlanSha256 -and (@($build.gates.phase)-join ',') -ceq ($gates-join ',') -and @($build.gates|Where-Object{$_.exitCode -ne 0 -or $_.timedOut -or -not $_.processExited}).Count -eq 0) 'all-thirteen-native-gates-required'
 $package=Full-Path $plan.desktopCandidate;$exe=Join-Path $package 'orchester-desktop.exe'
 Same-Inventory $build.desktopInventory (Directory-Inventory $package) 'reviewed capture desktop candidate'
 Assert-Capture ((File-Hash $exe) -ceq $build.desktopExeSha256) 'desktop-binary-pin-differs'
 Same-Inventory (Directory-Inventory (Join-Path $plan.sourceRoot 'apps/web/dist')) (Directory-Inventory (Join-Path $package 'web')) 'capture desktop Web'
 $SharedHelperPath=Full-Path $SharedHelperPath
 Assert-Capture ($SharedHelperPath -ieq (Join-Path (Split-Path -Parent $PSScriptRoot) 'prepared-new\OwnedRuntimeProcesses.cs') -and (File-Hash $SharedHelperPath) -ceq $ExpectedSharedHelperSha256) 'shared-helper-pin-differs'
 $WebViewExecutable=Full-Path $WebViewExecutable;Assert-Capture ((File-Hash $WebViewExecutable) -ceq $ExpectedWebViewSha256) 'webview-image-pin-differs'
 $system32=Join-Path (Full-Path $env:SystemRoot) 'System32'
 $helperPins=@(
  [pscustomobject]@{path=Join-Path $system32 'icacls.exe';sha256='cb9e55d4c02f4e55100724d2da9a1267f49e196bd39fcdf36cec6ea6314fad2a'},
  [pscustomobject]@{path=Join-Path $system32 'WindowsPowerShell\v1.0\powershell.exe';sha256='60c9b29843624dd8af6b6a7b26147753be6aeef65098b9a503cc50850afe410f'},
  [pscustomobject]@{path=Join-Path $system32 'cmd.exe';sha256='15cd209c995b7d3009c91ca40892d5b8f1fb162f3e5f10716642ea278e8e0d7b'},
  [pscustomobject]@{path=Join-Path $system32 'conhost.exe';sha256='c86146188b78e484f32cc29e2331689659943b47689a85570342044f499938ac'}
 )
 $wsl=Join-Path $system32 'wsl.exe';if(Test-Path -LiteralPath $wsl -PathType Leaf){$helperPins+=[pscustomobject]@{path=$wsl;sha256='fcd08205b608b5c4831e0dcb1801dd274436cd2c8420815f18ae58a085891a18'}}
 foreach($pin in $helperPins){Assert-Capture ((File-Hash $pin.path) -ceq $pin.sha256) 'system-helper-pin-differs'}
 Assert-Below $ResultDirectory $plan.runDirectory;$ResultDirectory=Full-Path $ResultDirectory
 Assert-Capture (-not (Test-Path -LiteralPath $ResultDirectory)) 'capture-result-directory-exists'
 Assert-Capture (19322 -notin @(Capture-ListeningPorts)) 'existing-debug-listener'
 Add-Type -Path $SharedHelperPath
 $allowedImages=@($exe,$WebViewExecutable)+@($helperPins.path)
 $owned=[Orchester.FinalRuntime.OwnedProcesses]::new($allowedImages)
 Assert-Capture ($owned.PSObject.Properties.Name -contains 'SafeUnexpectedImages') 'safe-unknown-helper-contract-missing'
 New-Item -ItemType Directory -Path $ResultDirectory|Out-Null;$resultCreated=$true
 $stateHome=Join-Path $ResultDirectory 'state-home';$profile=Join-Path $ResultDirectory 'webview-profile';$emptyTools=Join-Path $ResultDirectory 'empty-tools'
 New-Item -ItemType Directory -Path $stateHome,$profile,$emptyTools|Out-Null
 $utf8=New-Object Text.UTF8Encoding($false)
 Write-JsonFile ([ordered]@{purpose='capture-only-native-descendant-diagnostic';sourceRevision=$plan.sourceRevision;expectedPlanSha256=$ExpectedPlanSha256;expectedFinalResultSha256=$ExpectedFinalResultSha256;expectedSharedHelperSha256=$ExpectedSharedHelperSha256;expectedCaptureScriptSha256=$ExpectedCaptureScriptSha256;candidateInventory=$build.desktopInventory;binarySha256=$build.desktopExeSha256;systemHelperPins=$helperPins;webViewSha256=$ExpectedWebViewSha256;captureSeconds=$CaptureSeconds;cdpConnected=$false;uiChecks=0;modelCalls=0;defaultInstallationChanged=$false}) (Join-Path $ResultDirectory 'capture-inputs.json')
 $stage='capture-desktop-start'
 $launch=[Diagnostics.ProcessStartInfo]::new();$launch.FileName=$exe;$launch.WorkingDirectory=$package;$launch.UseShellExecute=$false;$launch.CreateNoWindow=$true
 $launch.RedirectStandardOutput=$true;$launch.RedirectStandardError=$true;$launch.StandardOutputEncoding=$utf8;$launch.StandardErrorEncoding=$utf8
 $launch.EnvironmentVariables['ORCHESTER_HOME']=$stateHome;$launch.EnvironmentVariables['WEBVIEW2_USER_DATA_FOLDER']=$profile
 $launch.EnvironmentVariables['WEBVIEW2_BROWSER_EXECUTABLE_FOLDER']=Split-Path -Parent $WebViewExecutable
 $launch.EnvironmentVariables['WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS']='--remote-debugging-port=19322 --remote-debugging-address=127.0.0.1';$launch.EnvironmentVariables['PATH']=$emptyTools
 $gui=[Diagnostics.Process]::new();$gui.StartInfo=$launch;$started=$gui.Start();Assert-Capture $started 'capture-desktop-start-failed'
 $owned.CaptureRoot($gui,$exe,$build.desktopExeSha256)
 $guiOut=$gui.StandardOutput.ReadToEndAsync();$guiErr=$gui.StandardError.ReadToEndAsync()
 Write-JsonFile ([ordered]@{ProcessId=$owned.Root.Pid;Created=$owned.Root.Created;Path=$owned.Root.Image;sha256=$owned.Root.Sha256;modelCalls=0}) (Join-Path $ResultDirectory 'capture-launch-result.json')
 $stage='capture-owned-descendants';$deadline=[DateTime]::UtcNow.AddSeconds($CaptureSeconds)
 do{
  Assert-Capture ($owned.IsAlive($owned.Root)) 'capture-desktop-exited-early'
  Capture-OwnedDescendants
  # This reads only bindings belonging to our retained GUI, without connecting.
  $embeddedPorts=@($embeddedPorts+@(Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue|Where-Object{$_.OwningProcess -eq $owned.Root.Pid -and $_.LocalAddress -eq '127.0.0.1'}|ForEach-Object{[int]$_.LocalPort})|Select-Object -Unique)
  Start-Sleep -Milliseconds 200
 }while([DateTime]::UtcNow -lt $deadline)
 $safeUnknown=@(Capture-SafeUnknownImages)
 Same-Inventory $build.desktopInventory (Directory-Inventory $package) 'desktop candidate after capture'
 Write-JsonFile ([ordered]@{status='capture-completed-cleanup-pending';sourceRevision=$plan.sourceRevision;capturedOwnedHandles=$owned.Count;uniqueUnknownImageCount=$safeUnknown.Count;uniqueUnknownImages=$safeUnknown;unexpectedDescendants=$owned.UnexpectedDescendants;embeddedPorts=$embeddedPorts;cdpConnected=$false;uiChecks=0;modelCalls=0}) (Join-Path $ResultDirectory 'capture-result.json')
}catch{
 $failed=$true;$diagnostic=Capture-SafeException $_.Exception
 if($resultCreated){try{Write-JsonFile ([ordered]@{status='failed';stage=$stage;diagnostic=$diagnostic;cdpConnected=$false;uiChecks=0;modelCalls=0}) (Join-Path $ResultDirectory 'capture-failure.json')}catch{}}
 [Console]::Error.WriteLine('Desktop descendant capture failed at '+$stage+' ('+$diagnostic.baseExceptionType+', code='+$diagnostic.staticCode+')')
}finally{
 $cleanupDiagnostic=$null;$outcomes=@();$handlesExited=$false;$portsReleased=$false
 try{
  if($started -and $owned -and -not $owned.Root -and -not $gui.HasExited){$owned.CaptureRoot($gui,$exe,$build.desktopExeSha256)}
  try{Capture-OwnedDescendants}catch{$cleanupDiagnostic=Capture-SafeException $_.Exception}
  if($owned -and $owned.Root){
   try{$safeUnknown=@(Capture-SafeUnknownImages)}catch{$cleanupDiagnostic=Capture-SafeException $_.Exception}
   # Unknown images are diagnostic only and are never newly allowed/signalled.
   $outcomes=@($owned.TerminateRemaining());$handlesExited=$owned.AllExited()
  }elseif(-not $started -or $gui.HasExited){$handlesExited=$true}
  $deadline=[DateTime]::UtcNow.AddSeconds(10)
  do{$ports=@(Capture-ListeningPorts);$portsReleased=(19322 -notin $ports -and @($embeddedPorts|Where-Object{$_ -in $ports}).Count -eq 0);if(-not $portsReleased){Start-Sleep -Milliseconds 100}}while(-not $portsReleased -and [DateTime]::UtcNow -lt $deadline)
  $cleanupOk=$handlesExited -and $portsReleased -and -not $cleanupDiagnostic
 }catch{$cleanupDiagnostic=Capture-SafeException $_.Exception;$cleanupOk=$false}
 if($resultCreated){try{
  Write-JsonFile ([ordered]@{status=if($cleanupOk){'exact-owned-cleanup-passed'}else{'failed'};allExactOwnedHandlesExited=$handlesExited;portsReleased=$portsReleased;fallbackOutcomes=$outcomes;diagnostic=$cleanupDiagnostic;uniqueUnknownImageCount=$safeUnknown.Count;uniqueUnknownImages=$safeUnknown;unexpectedDescendants=if($owned){$owned.UnexpectedDescendants}else{0};originalHandlesKeptUntilCleanup=$true;cdpConnected=$false;uiChecks=0;modelCalls=0;defaultServiceStopped=$false}) (Join-Path $ResultDirectory 'capture-cleanup-result.json')
  if(-not $failed -and $cleanupOk){Write-JsonFile ([ordered]@{status='capture-only-completed-cleanup-passed';sourceRevision=$plan.sourceRevision;uniqueUnknownImageCount=$safeUnknown.Count;uniqueUnknownImages=$safeUnknown;unexpectedDescendants=$owned.UnexpectedDescendants;nativeDesktopAcceptance='not-run';cdpConnected=$false;uiChecks=0;modelCalls=0}) (Join-Path $ResultDirectory 'capture-final-result.json')}
 }catch{$cleanupOk=$false}}
 if($gui){$gui.Dispose()};if($owned){$owned.Dispose()}
}
if($failed -or -not $cleanupOk){exit 1}
Write-Output 'Capture-only desktop diagnostic completed; inspect safe unknown images and exact-owned cleanup evidence.'
exit 0
