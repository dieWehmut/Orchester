[CmdletBinding()]
param(
 [Parameter(Mandatory=$true)][string]$PlanFile,
 [Parameter(Mandatory=$true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedPlanSha256,
 [Parameter(Mandatory=$true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedFinalResultSha256,
 [Parameter(Mandatory=$true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedPreparedScriptsSha256,
 [Parameter(Mandatory=$true)][string]$RuntimeInputsFile,
 [Parameter(Mandatory=$true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedRuntimeInputsSha256,
 [Parameter(Mandatory=$true)][string]$ResultDirectory,
 [string]$WindowsNode='C:\Users\han\AppData\Local\dsh-shortcut\node\node-v24.21.0-win-x64\node.exe',
 [ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedWindowsNodeSha256='ba4e6d110e8c1592a1ecd390f6b05f3da124b13871a5be62b341a07a853c6c32',
 [string]$WebViewExecutable='C:\Program Files (x86)\Microsoft\EdgeWebView\Application\157.0.4341.42\msedgewebview2.exe',
 [ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedWebViewSha256='51b4cd503d669d8f7f6ff6e9e22effeebd28cce11262ddec815972a424474533',
 [ValidateRange(60,1800)][int]$AckTimeoutSeconds=900,
 [ValidateRange(180,7200)][int]$RuntimeTimeoutSeconds=2400
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version 2.0
$script:stage='preflight';$resultCreated=$false;$failed=$false;$cleanupOk=$false
$gui=$null;$node=$null;$guiOwned=$null;$nodeOwned=$null;$guiStarted=$false;$nodeStarted=$false;$embeddedPort=0
$guiOut=$null;$guiErr=$null;$nodeOut=$null;$nodeErr=$null;$http=$null;$trayQuitNatural=$false;$trayQuitSha256=$null
function Assert-True([bool]$Condition,[string]$Label) { if(-not $Condition){throw [InvalidOperationException]::new($Label)} }
function Safe-Exception([Exception]$Exception) {
 $base=$Exception.GetBaseException()
 $allowedCodes=@('tray-process-unavailable','tray-process-identity-mismatch','tray-window-owner-mismatch','tray-window-enumeration-failed','tray-window-not-unique','tray-menu-open-failed','tray-popup-not-unique','tray-popup-uia-owner-mismatch','tray-native-menu-items-differ','tray-native-quit-not-invokable','owned-identity-unreadable','owned-identity-mismatch','owned-wait-failed','owned-root-already-set','owned-root-handle-failed','owned-root-image-mismatch','owned-snapshot-failed','owned-snapshot-empty','owned-snapshot-incomplete','owned-descendant-handle-denied','window-process-unavailable','window-process-identity-mismatch','window-owner-mismatch','window-client-rect-failed','window-client-screen-failed','window-hit-test-timeout','window-enumeration-failed','window-rect-failed','window-resize-failed')
 # Only an exact known helper code may expose Message. No other message or
 # stack, URL, Cookie value or token is serialized, even for wrapped failures.
 [ordered]@{type=$Exception.GetType().FullName;baseExceptionType=$base.GetType().FullName;staticCode=if($base.Message -cin $allowedCodes){$base.Message}else{$null}}
}
function Listening-Ports { @([System.Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties().GetActiveTcpListeners() | ForEach-Object { $_.Port }) }
function Capture-Gui { if($guiOwned -and $guiOwned.Root -and $guiOwned.IsAlive($guiOwned.Root)){$guiOwned.CaptureAllowedDescendants()} }
function Gui-SafeUnexpectedImages {
 if(-not $guiOwned){return @()}
 @($guiOwned.SafeUnexpectedImages | ForEach-Object {[pscustomobject]@{basename=$_.Basename;systemRootRelativePath=$_.SystemRootRelativePath;sha256=$_.Sha256}})
}
function WebView-Pids {
 $pids=@();$pending=New-Object 'System.Collections.Generic.Queue[Orchester.FinalRuntime.OwnedProcess]'
 $pending.Enqueue($guiOwned.Root)
 while($pending.Count -gt 0){foreach($child in @($guiOwned.DirectChildren($pending.Dequeue(),$WebViewExecutable))){$pids+= $child.Pid;$pending.Enqueue($child)}}
 $pids
}
function Write-WindowResponses {
 foreach($requestFile in @(Get-ChildItem -LiteralPath $ResultDirectory -Filter 'window-request-*.json' -File | Sort-Object Name)){
  if($requestFile.Name -notmatch '\Awindow-request-([0-9]{4})\.json\z'){throw [InvalidOperationException]::new('window-request-name-invalid')}
  $id=$Matches[1];$responseFile=Join-Path $ResultDirectory ('window-response-'+$id+'.json')
  if(Test-Path -LiteralPath $responseFile){continue}
  $request=Get-Content -Raw -LiteralPath $requestFile.FullName | ConvertFrom-Json
  Assert-True ($request.id -ceq $id -and @($request.PSObject.Properties.Name).Count -eq 2 -and $request.action -cin @('snapshot','resize')) 'window-request-invalid'
  $identity=@{Process=$gui;ExpectedProcessId=[uint32]$guiOwned.Root.Pid;ExpectedCreated=$guiOwned.Root.Created;ExpectedImage=$guiOwned.Root.Image;ExpectedSha256=$guiOwned.Root.Sha256}
  if($request.action -ceq 'snapshot'){$value=@(& (Join-Path $PSScriptRoot 'window-snapshot.ps1') @identity)}else{$value=& (Join-Path $PSScriptRoot 'resize-isolated-window.ps1') @identity}
  $temporary=$responseFile+'.tmp';Write-JsonFile ([ordered]@{id=$id;status='ok';value=$value}) $temporary
  [IO.File]::Move($temporary,$responseFile)
 }
}
try{
 . (Join-Path (Split-Path -Parent $PSScriptRoot) 'monitor-final-common.ps1')
 $plan=Read-Plan $PlanFile $ExpectedPlanSha256
 $buildFile=Join-Path $plan.runDirectory 'final-result.json'
 Assert-True ((File-Hash $buildFile) -ceq $ExpectedFinalResultSha256) 'final-result-pin-differs'
 $build=Get-Content -Raw -LiteralPath $buildFile | ConvertFrom-Json
 $expectedGates = @($plan.commands)
 Assert-True ($build.status -ceq 'build-and-file-gates-passed' -and $build.sourceRevision -ceq $plan.sourceRevision -and $build.expectedPlanSha256 -ceq $ExpectedPlanSha256 -and (@($build.gates.phase) -join ',') -ceq ($expectedGates -join ',') -and @($build.gates | Where-Object {$_.exitCode -ne 0 -or $_.timedOut -or -not $_.processExited}).Count -eq 0) 'all-sixteen-native-gates-required'
 . (Join-Path $PSScriptRoot 'runtime-input-guards.ps1')
 $runtimeInputs=Read-FreshRuntimeInputs $plan $build $ExpectedPlanSha256 $ExpectedFinalResultSha256 $RuntimeInputsFile $ExpectedRuntimeInputsSha256
 $package=Full-Path $plan.desktopCandidate;$exe=Join-Path $package 'orchester-desktop.exe'
 Same-Inventory $build.desktopInventory (Directory-Inventory $package) 'reviewed desktop candidate'
 Assert-True ((File-Hash $exe) -ceq $build.desktopExeSha256) 'desktop-executable-pin-differs'
 Same-Inventory (Directory-Inventory (Join-Path $plan.sourceRoot 'apps/web/dist')) (Directory-Inventory (Join-Path $package 'web')) 'reviewed desktop Web'
 $manifestFile=Join-Path $PSScriptRoot 'prepared-scripts.json'
 Assert-True ((File-Hash $manifestFile) -ceq $ExpectedPreparedScriptsSha256) 'prepared-script-manifest-pin-differs'
 $manifest=Get-Content -Raw -LiteralPath $manifestFile | ConvertFrom-Json
 $expectedFiles=@('NativeDesktopWindows.cs','OwnedRuntimeProcesses.cs','launch-smoke.ps1','resize-isolated-window.ps1','verify-final-native.mjs','window-snapshot.ps1','NativeTrayQuit.cs','invoke-owned-tray-quit.ps1','runtime-input-guards.ps1','retain-owned-observation.ps1')
 Assert-True ($manifest.format -ceq 'orchester-desktop-native-script-pins-v1' -and (@($manifest.files.path) -join ',') -ceq ($expectedFiles -join ',')) 'prepared-script-inventory-differs'
 foreach($entry in @($manifest.files)){Assert-True ((File-Hash (Join-Path $PSScriptRoot $entry.path)) -ceq $entry.sha256) 'prepared-script-pin-differs'}
 Assert-True ((File-Hash (Join-Path $PSScriptRoot 'OwnedRuntimeProcesses.cs')) -ceq 'd335743a632acae0c90968e482b87eda44aaf1dae241e938e62b5c7833598d45') 'owned-handle-helper-pin-differs'
 $WindowsNode=Full-Path $WindowsNode;$WebViewExecutable=Full-Path $WebViewExecutable
 Assert-True ((File-Hash $WindowsNode) -ceq $ExpectedWindowsNodeSha256 -and (File-Hash $WebViewExecutable) -ceq $ExpectedWebViewSha256) 'native-tool-pin-differs'
 Assert-True ([IO.Path]::GetFileName($WindowsNode) -ieq 'node.exe' -and [IO.Path]::GetFileName($WebViewExecutable) -ieq 'msedgewebview2.exe') 'native-tool-image-differs'
 # Private-state ACL setup uses these absolute system tools despite empty PATH.
 # Only these exact installed images, hashes and original GUI descendants qualify.
 $system32=Join-Path (Full-Path $env:SystemRoot) 'System32'
 $systemHelperPins=@(
  [pscustomobject]@{path=Join-Path $system32 'icacls.exe';sha256='cb9e55d4c02f4e55100724d2da9a1267f49e196bd39fcdf36cec6ea6314fad2a'},
  [pscustomobject]@{path=Join-Path $system32 'WindowsPowerShell\v1.0\powershell.exe';sha256='60c9b29843624dd8af6b6a7b26147753be6aeef65098b9a503cc50850afe410f'},
  [pscustomobject]@{path=Join-Path $system32 'cmd.exe';sha256='15cd209c995b7d3009c91ca40892d5b8f1fb162f3e5f10716642ea278e8e0d7b'},
  [pscustomobject]@{path=Join-Path $system32 'conhost.exe';sha256='c86146188b78e484f32cc29e2331689659943b47689a85570342044f499938ac'}
 )
 $systemWsl=Join-Path $system32 'wsl.exe'
 if(Test-Path -LiteralPath $systemWsl -PathType Leaf){$systemHelperPins+=[pscustomobject]@{path=$systemWsl;sha256='fcd08205b608b5c4831e0dcb1801dd274436cd2c8420815f18ae58a085891a18'}}
 # Capture-only evidence identified the System32 WSL forwarder's MSI child.
 $systemHelperPins+=[pscustomobject]@{path='C:\Program Files\WSL\wsl.exe';sha256='051b7a9b7bda66817c052aced998e026610e5d4d18d63f45629d35d5cb5471c8'}
 $systemHelperPins+=[pscustomobject]@{path='C:\Program Files\WSL\wslhost.exe';sha256='1f214fbb69efcd138260c4aa4e35182bfe5371afe4055b59a3133d8f88f9c3b7'}
 $wslHostSignature=Get-AuthenticodeSignature -LiteralPath 'C:\Program Files\WSL\wslhost.exe'
 Assert-True ($wslHostSignature.Status.ToString() -ceq 'Valid' -and $wslHostSignature.SignerCertificate -and $wslHostSignature.SignerCertificate.Subject -ceq 'CN=Microsoft Corporation, O=Microsoft Corporation, L=Redmond, S=Washington, C=US') 'pinned-wslhost-signature-differs'
 foreach($helperPin in $systemHelperPins){Assert-True ((File-Hash $helperPin.path) -ceq $helperPin.sha256) 'system-helper-image-pin-differs'}
 $guiAllowedImages=@($exe,$WebViewExecutable)+@($systemHelperPins.path)
 Assert-True ($RuntimeTimeoutSeconds -gt (2*$AckTimeoutSeconds+120)) 'runtime-timeout-too-short-for-two-acks'
 Assert-Below $ResultDirectory $plan.runDirectory
 $ResultDirectory=Full-Path $ResultDirectory
 Assert-True (-not (Test-Path -LiteralPath $ResultDirectory)) 'result-directory-already-exists'
 Assert-True (19322 -notin @(Listening-Ports)) 'old-debug-listener-exists'
 $fixture='C:\Users\han\AppData\Local\Orchester-build\native-review-20261110\smoke\state-home\sessions.jsonl'
 $utf8=New-Object System.Text.UTF8Encoding($false)
 $fixtureHash=File-Hash $fixture
 $lines=@([IO.File]::ReadAllLines($fixture,$utf8));Assert-True ($lines.Count -eq 1) 'fixture-must-have-exactly-one-line'
 $row=$lines[0] | ConvertFrom-Json
 Assert-True ((@($row.PSObject.Properties.Name | Sort-Object) -join ',') -ceq 'agent,cwd,final_text,outcome,prompt,recorded_at_unix,usage') 'fixture-fields-differ'
 Assert-True ($row.recorded_at_unix -eq 1794250000 -and $row.agent -ceq 'codex' -and $row.prompt -ceq 'Native smoke saved question' -and $row.final_text -ceq 'Native smoke saved answer' -and $row.outcome -ceq 'success' -and $row.cwd -ceq 'C:\Users\han\AppData\Local\Orchester-build\native-review-20261110\smoke\state-home\workspaces\default') 'fixture-content-differs'
 Assert-True ((@($row.usage.PSObject.Properties.Name | Sort-Object) -join ',') -ceq 'cached_input_tokens,input_tokens,output_tokens,reasoning_output_tokens') 'fixture-usage-fields-differ'
 Assert-True ($row.usage.input_tokens -eq 0 -and $row.usage.output_tokens -eq 0 -and $row.usage.cached_input_tokens -eq 0 -and $row.usage.reasoning_output_tokens -eq 0) 'fixture-usage-differs'
 Assert-True ((File-Hash $fixture) -ceq $fixtureHash) 'fixture-changed-during-review'
 New-Item -ItemType Directory -Path $ResultDirectory | Out-Null;$resultCreated=$true
 $stateHome=Join-Path $ResultDirectory 'state-home';$profile=Join-Path $ResultDirectory 'webview-profile';$emptyTools=Join-Path $ResultDirectory 'empty-tools'
 New-Item -ItemType Directory -Path $stateHome,$profile,$emptyTools | Out-Null
 Copy-Item -LiteralPath $fixture -Destination (Join-Path $stateHome 'sessions.jsonl')
 Assert-True ((File-Hash (Join-Path $stateHome 'sessions.jsonl')) -ceq $fixtureHash) 'fixture-copy-differs'
 Write-JsonFile ([ordered]@{sourceRevision=$plan.sourceRevision;expectedPlanSha256=$ExpectedPlanSha256;expectedFinalResultSha256=$ExpectedFinalResultSha256;expectedRuntimeInputsSha256=$ExpectedRuntimeInputsSha256;expectedPreparedScriptsSha256=$ExpectedPreparedScriptsSha256;candidateInventory=$build.desktopInventory;binarySha256=$build.desktopExeSha256;fixtureSha256=$fixtureHash;nodeSha256=$ExpectedWindowsNodeSha256;webViewSha256=$ExpectedWebViewSha256;systemHelperPins=$systemHelperPins;modelCalls=0;defaultInstallationChanged=$false}) (Join-Path $ResultDirectory 'runtime-inputs.json')
 Add-Type -Path (Join-Path $PSScriptRoot 'OwnedRuntimeProcesses.cs')
 Assert-True ((File-Hash (Join-Path $PSScriptRoot 'retain-owned-observation.ps1')) -ceq '6f758142622e49e7b6e7892c7b254a5dda53b990425d0ff7b9e54d84a7523344') 'retainer-module-pin-differs'
 . (Join-Path $PSScriptRoot 'retain-owned-observation.ps1')
 Add-Type -Path (Join-Path $PSScriptRoot 'NativeDesktopWindows.cs')
 $guiOwned=[Orchester.FinalRuntime.OwnedProcesses]::new($guiAllowedImages);$guiOwned.RegisterPassiveWslHostObservation();$nodeOwned=[Orchester.FinalRuntime.OwnedProcesses]::new(@($WindowsNode))
 $script:stage='desktop-start'
 $start=[Diagnostics.ProcessStartInfo]::new();$start.FileName=$exe;$start.WorkingDirectory=$package;$start.UseShellExecute=$false;$start.CreateNoWindow=$true
 $start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true;$start.StandardOutputEncoding=$utf8;$start.StandardErrorEncoding=$utf8
 $start.EnvironmentVariables['ORCHESTER_HOME']=$stateHome;$start.EnvironmentVariables['WEBVIEW2_USER_DATA_FOLDER']=$profile
 $start.EnvironmentVariables['WEBVIEW2_BROWSER_EXECUTABLE_FOLDER']=Split-Path -Parent $WebViewExecutable
 $start.EnvironmentVariables['WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS']='--remote-debugging-port=19322 --remote-debugging-address=127.0.0.1'
 $start.EnvironmentVariables['PATH']=$emptyTools
 $gui=[Diagnostics.Process]::new();$gui.StartInfo=$start;$guiStarted=$gui.Start();Assert-True $guiStarted 'desktop-start-failed'
 $guiOwned.CaptureRoot($gui,$exe,$build.desktopExeSha256)
 $guiOut=$gui.StandardOutput.ReadToEndAsync();$guiErr=$gui.StandardError.ReadToEndAsync()
 Write-JsonFile ([ordered]@{ProcessId=$guiOwned.Root.Pid;Created=$guiOwned.Root.Created;Path=$guiOwned.Root.Image;binarySha256=$guiOwned.Root.Sha256;packageDirectory=$package;uiFontAssetPins=@($build.desktopInventory | Where-Object {$_.path -cmatch '\Aweb/.*LXGWWenKai.*\.woff2\z'});stateHome=$stateHome;webViewProfile=$profile;cdpPort=19322;sourceRevision=$plan.sourceRevision;expectedPlanSha256=$ExpectedPlanSha256;expectedFinalResultSha256=$ExpectedFinalResultSha256;expectedRuntimeInputsSha256=$ExpectedRuntimeInputsSha256;ackTimeoutSeconds=$AckTimeoutSeconds;modelCalls=0}) (Join-Path $ResultDirectory 'launch-result.json')
 $script:stage='owned-debug-listener'
 $deadline=[DateTime]::UtcNow.AddSeconds(40);$ready=$false
 Add-Type -AssemblyName System.Net.Http
 $handler=[System.Net.Http.HttpClientHandler]::new();$handler.UseProxy=$false;$http=[System.Net.Http.HttpClient]::new($handler);$http.Timeout=[TimeSpan]::FromSeconds(2)
 do{
  Assert-True ($guiOwned.IsAlive($guiOwned.Root)) 'desktop-exited-before-debug-ready';Capture-Gui
  $listeners=@(Get-NetTCPConnection -State Listen -LocalPort 19322 -ErrorAction SilentlyContinue)
  if($listeners.Count -gt 0){
   $pids=@(WebView-Pids)
   Assert-True (@($listeners | Where-Object { $_.LocalAddress -ne '127.0.0.1' }).Count -eq 0) 'debug-listener-not-loopback'
   # A child born in this clock tick is intentionally not registered yet.
   # Wait for the next capture before contacting its debug listener.
   if(@($listeners | Where-Object {$_.OwningProcess -notin $pids}).Count -eq 0){
    try{$response=$http.GetAsync('http://127.0.0.1:19322/json/list').GetAwaiter().GetResult();try{$targets=$response.Content.ReadAsStringAsync().GetAwaiter().GetResult() | ConvertFrom-Json;$ready=$response.IsSuccessStatusCode -and @($targets).Count -gt 0}finally{$response.Dispose()}}catch{$ready=$false}
   }
  }
  if(-not $ready){Start-Sleep -Milliseconds 200}
 }while(-not $ready -and [DateTime]::UtcNow -lt $deadline)
 Assert-True $ready 'owned-debug-endpoint-not-ready'
 Assert-True ($guiOwned.UnexpectedDescendants -eq 0) 'unexpected-desktop-descendant'
 $script:stage='node-start'
 $nodeStart=[Diagnostics.ProcessStartInfo]::new();$nodeStart.FileName=$WindowsNode;$nodeStart.WorkingDirectory=$ResultDirectory;$nodeStart.UseShellExecute=$false;$nodeStart.CreateNoWindow=$true
 $nodeStart.Arguments=(Quote-Native (Join-Path $PSScriptRoot 'verify-final-native.mjs'))+' '+(Quote-Native $ResultDirectory)
 $nodeStart.RedirectStandardOutput=$true;$nodeStart.RedirectStandardError=$true;$nodeStart.StandardOutputEncoding=$utf8;$nodeStart.StandardErrorEncoding=$utf8
 $nodeStart.EnvironmentVariables.Remove('NODE_OPTIONS');$nodeStart.EnvironmentVariables['PATH']=$emptyTools
 $node=[Diagnostics.Process]::new();$node.StartInfo=$nodeStart;$nodeStarted=$node.Start();Assert-True $nodeStarted 'node-start-failed'
 $nodeOwned.CaptureRoot($node,$WindowsNode,$ExpectedWindowsNodeSha256)
 $nodeOut=$node.StandardOutput.ReadToEndAsync();$nodeErr=$node.StandardError.ReadToEndAsync()
 Write-JsonFile ([ordered]@{ProcessId=$nodeOwned.Root.Pid;Created=$nodeOwned.Root.Created;Path=$nodeOwned.Root.Image;sha256=$nodeOwned.Root.Sha256;modelCalls=0}) (Join-Path $ResultDirectory 'node-launch-result.json')
 $script:stage='desktop-eighteen-checks';$runtimeDeadline=[DateTime]::UtcNow.AddSeconds($RuntimeTimeoutSeconds)
 while($nodeOwned.IsAlive($nodeOwned.Root)){
  Assert-True ($guiOwned.IsAlive($guiOwned.Root)) 'desktop-exited-during-ui-checks';Capture-Gui
  Assert-True ($guiOwned.UnexpectedDescendants -eq 0) 'unexpected-desktop-descendant'
  Write-WindowResponses
  Assert-True ([DateTime]::UtcNow -lt $runtimeDeadline) 'desktop-runtime-timeout'
  Start-Sleep -Milliseconds 50
 }
 $null=$node.WaitForExit(2000)
 Assert-True ($node.ExitCode -eq 0) 'node-ui-verification-failed'
 Assert-True ($nodeErr.Wait(2000) -and [string]::IsNullOrWhiteSpace($nodeErr.Result)) 'node-emitted-stderr'
 $native=Get-Content -Raw -LiteralPath (Join-Path $ResultDirectory 'native-windows-result.json') | ConvertFrom-Json
 Assert-True ($native.passed -and $native.sourceRevision -ceq $plan.sourceRevision -and $native.expectedPlanSha256 -ceq $ExpectedPlanSha256 -and $native.expectedFinalResultSha256 -ceq $ExpectedFinalResultSha256 -and $native.binarySha256 -ceq $build.desktopExeSha256 -and @($native.checks).Count -eq 18 -and @($native.writes).Count -eq 7 -and @($native.writes | Where-Object {$_.status -ne 200}).Count -eq 0 -and @($native.screenshotAcknowledgements).Count -eq 2 -and @($native.pageErrors).Count -eq 0) 'desktop-native-result-incomplete'
 foreach($review in @($native.screenshotAcknowledgements)){
  Assert-True ($review.name -cin @('desktop-secondary-history.png','desktop-main-after-secondary-close.png') -and $review.accepted -and (File-Hash (Join-Path $ResultDirectory $review.name)) -ceq $review.sha256) 'reviewed-screenshot-differs'
 }
 $listener=Get-Content -Raw -LiteralPath (Join-Path $ResultDirectory 'runtime-listener.json') | ConvertFrom-Json;$embeddedPort=[int]$listener.embeddedPort
 Assert-True ($embeddedPort -gt 0 -and $embeddedPort -lt 65536 -and $listener.cdpPort -eq 19322) 'runtime-listener-record-invalid'
 Same-Inventory $build.desktopInventory (Directory-Inventory $package) 'desktop candidate after runtime'
 Write-JsonFile ([ordered]@{status='desktop-eighteen-ui-checks-passed-cleanup-pending';sourceRevision=$plan.sourceRevision;checks=18;writes=7;screenshotsReviewed=2;modelCalls=0;defaultInstallationChanged=$false}) (Join-Path $ResultDirectory 'desktop-ui-result.json')
 $script:stage='desktop-native-tray-quit';Capture-Gui
 & (Join-Path $PSScriptRoot 'invoke-owned-tray-quit.ps1') -Process $gui -ExpectedProcessId ([uint32]$guiOwned.Root.Pid) -ExpectedCreated $guiOwned.Root.Created -ExpectedImage $guiOwned.Root.Image -ExpectedSha256 $guiOwned.Root.Sha256 -ResultDirectory $ResultDirectory
 $trayMenu=Get-Content -Raw -LiteralPath (Join-Path $ResultDirectory 'tray-menu-observation.json') | ConvertFrom-Json
 $trayMenuBirth=if($trayMenu.activation -ceq 'MSAA.IAccessible.accDoDefaultAction'){$trayMenu.guiCreatedFileTimeExact}else{$trayMenu.guiCreated}
 Assert-True ($trayMenu.event -ceq 'owned_native_tray_menu_observed' -and $trayMenu.guiPid -eq $guiOwned.Root.Pid -and [string]$trayMenuBirth -ceq $guiOwned.Root.Created.ToString() -and $trayMenu.binarySha256 -ceq $build.desktopExeSha256 -and @($trayMenu.labels).Count -eq 3 -and $trayMenu.labels[0] -ceq 'Open Orchester' -and $trayMenu.labels[1] -ceq 'Settings...' -and $trayMenu.labels[2] -ceq 'Quit Orchester' -and $trayMenu.activation -cin @('UIAutomation.InvokePattern','MSAA.IAccessible.accDoDefaultAction') -and $trayMenu.originalProcessHandleRetained -eq $true -and $trayMenu.noForcedProcessTermination -eq $true) 'desktop-tray-menu-activation-receipt-differs'
 $actualTrayActivation=$trayMenu.activation
 $exitLimit=[DateTime]::UtcNow.AddSeconds(10)
 while($guiOwned.IsAlive($guiOwned.Root) -and [DateTime]::UtcNow -lt $exitLimit){Capture-Gui;Start-Sleep -Milliseconds 50}
 Assert-True ($gui.WaitForExit(1000) -and $gui.ExitCode -eq 0) 'desktop-tray-quit-did-not-exit-zero'
 $passiveLimit=[DateTime]::UtcNow.AddSeconds(5)
 do{$naturalExited=$guiOwned.AllExited();if(-not $naturalExited){Start-Sleep -Milliseconds 50}}while(-not $naturalExited -and [DateTime]::UtcNow -lt $passiveLimit)
 $remaining=@($guiOwned.Identities|Where-Object{$guiOwned.IsAlive($_)})
 Write-JsonFile ([ordered]@{guiExitCode=$gui.ExitCode;passiveWaitSeconds=5;remainingIdentities=$remaining;unexpectedDescendants=$guiOwned.UnexpectedDescendants;forcedTerminations=0;modelCalls=0}) (Join-Path $ResultDirectory 'tray-natural-exit-observation.json')
 Assert-True ($naturalExited -and $guiOwned.UnexpectedDescendants -eq 0 -and $nodeOwned.AllExited()) 'desktop-tray-quit-owned-descendants-remained'
 $portLimit=[DateTime]::UtcNow.AddSeconds(5)
 do{$ports=@(Listening-Ports);$naturalPortsReleased=(19322 -notin $ports -and $embeddedPort -notin $ports);if(-not $naturalPortsReleased){Start-Sleep -Milliseconds 50}}while(-not $naturalPortsReleased -and [DateTime]::UtcNow -lt $portLimit)
 Assert-True $naturalPortsReleased 'desktop-tray-quit-ports-remained'
 $trayQuitFile=Join-Path $ResultDirectory 'tray-quit-result.json'
 Write-JsonFile ([ordered]@{event='owned_native_tray_quit_verified';sourceRevision=$plan.sourceRevision;expectedPlanSha256=$ExpectedPlanSha256;expectedFinalResultSha256=$ExpectedFinalResultSha256;expectedRuntimeInputsSha256=$ExpectedRuntimeInputsSha256;binarySha256=$build.desktopExeSha256;nativeMenuActivation=$actualTrayActivation;guiExitCode=0;allExactGuiHandlesExited=$true;allExactNodeHandlesExited=$true;debugAndEmbeddedPortsReleased=$true;forcedTerminations=0;unexpectedDescendants=0;passiveWaitSeconds=5;modelCalls=0;defaultInstallationChanged=$false}) $trayQuitFile
 $trayQuitNatural=$true;$trayQuitSha256=File-Hash $trayQuitFile

}catch{
 $failed=$true
 $failure=Safe-Exception $_.Exception
 if($resultCreated){try{Write-JsonFile ([ordered]@{status='failed';stage=$script:stage;type=$failure.type;baseExceptionType=$failure.baseExceptionType;staticCode=$failure.staticCode;safeUnexpectedImages=@(Gui-SafeUnexpectedImages);modelCalls=0;defaultInstallationChanged=$false}) (Join-Path $ResultDirectory 'desktop-worker-failure.json')}catch{}}
 [Console]::Error.WriteLine('Desktop native verification failed at '+$script:stage+' ('+$failure.baseExceptionType+', code='+$failure.staticCode+')')
}finally{
 try{
 if($guiOwned){Write-JsonFile @($guiOwned.Identities) (Join-Path $ResultDirectory 'gui-owned-identities.json')}
 if($nodeOwned){Write-JsonFile @($nodeOwned.Identities) (Join-Path $ResultDirectory 'node-owned-identities.json')}
 $guiOutcomes=@();$nodeOutcomes=@();$cleanupType=$null;$cleanupDiagnostic=$null;$allGuiExited=$false;$allNodeExited=$false;$portsReleased=$false
 try{
  # Both Process objects returned by Start remain alive until this block ends.
  # No user/default process is looked up or signalled by PID or image name.
  try{if($guiStarted -and $guiOwned -and -not $guiOwned.Root -and -not $gui.HasExited){$guiOwned.CaptureRoot($gui,$exe,$build.desktopExeSha256)}}catch{$cleanupDiagnostic=Safe-Exception $_.Exception;$cleanupType=$cleanupDiagnostic.type}
  try{if($nodeStarted -and $nodeOwned -and -not $nodeOwned.Root -and -not $node.HasExited){$nodeOwned.CaptureRoot($node,$WindowsNode,$ExpectedWindowsNodeSha256)}}catch{$cleanupDiagnostic=Safe-Exception $_.Exception;$cleanupType=$cleanupDiagnostic.type}
  try{Capture-Gui}catch{$cleanupDiagnostic=Safe-Exception $_.Exception;$cleanupType=$cleanupDiagnostic.type}
  # A discovery/cleanup error for one registry must not skip the other registry.
  try{if($nodeOwned -and $nodeOwned.Root){$allNodeExited=$nodeOwned.AllExited()}elseif(-not $nodeStarted -or $node.HasExited){$allNodeExited=$true}}catch{$cleanupDiagnostic=Safe-Exception $_.Exception;$cleanupType=$cleanupDiagnostic.type}
  try{if($guiOwned -and $guiOwned.Root){$allGuiExited=$guiOwned.AllExited()}elseif(-not $guiStarted -or $gui.HasExited){$allGuiExited=$true}}catch{$cleanupDiagnostic=Safe-Exception $_.Exception;$cleanupType=$cleanupDiagnostic.type}
  if($guiOwned){while($guiOwned.HasLivePassiveWslHost()){
   $liveHosts=@($guiOwned.Identities|Where-Object {$_.Image -ieq 'C:\Program Files\WSL\wslhost.exe' -and $guiOwned.IsAlive($_)})
   $liveProof=@($liveHosts|ForEach-Object {$hostIdentity=$_;$parent=@($guiOwned.Identities|Where-Object {$_.Pid -eq $hostIdentity.ParentPid -and $_.Created -le $hostIdentity.Created}|Sort-Object Created -Descending|Select-Object -First 1);[pscustomobject]@{pid=$hostIdentity.Pid;created=$hostIdentity.Created;parentPid=$hostIdentity.ParentPid;parentCreated=if($parent.Count -eq 1){$parent[0].Created}else{$null};image=$hostIdentity.Image;sha256=$hostIdentity.Sha256;originalHandleLive=$true}})
   Write-JsonFile ([ordered]@{status='passive-pinned-wslhost-observation-pending';remainingCount=[int]$liveProof.Count;remaining=$liveProof;actualRemaining=($liveProof.Count -gt 0);originalHandlesKept=$true;systemBrokerSignalled=$false;sharedLifetimeClaimed=$false;modelCalls=0}) (Join-Path $ResultDirectory 'passive-wslhost-current-safe.json');Start-Sleep -Milliseconds 500
  };$allGuiExited=$guiOwned.AllExited()}
  if($embeddedPort -eq 0 -and $resultCreated -and (Test-Path -LiteralPath (Join-Path $ResultDirectory 'runtime-listener.json'))){try{$embeddedPort=[int](Get-Content -Raw -LiteralPath (Join-Path $ResultDirectory 'runtime-listener.json') | ConvertFrom-Json).embeddedPort}catch{}}
  $portDeadline=[DateTime]::UtcNow.AddSeconds(10)
  do{$ports=@(Listening-Ports);$portsReleased=(19322 -notin $ports -and ($embeddedPort -eq 0 -or $embeddedPort -notin $ports));if(-not $portsReleased){Start-Sleep -Milliseconds 100}}while(-not $portsReleased -and [DateTime]::UtcNow -lt $portDeadline)
  $unexpected=if($guiOwned){$guiOwned.UnexpectedDescendants}else{0}
  $cleanupOk=$allGuiExited -and $allNodeExited -and $portsReleased -and $unexpected -eq 0 -and -not $cleanupType -and $trayQuitNatural -and $guiOutcomes.Count -eq 0 -and $nodeOutcomes.Count -eq 0
 }catch{$cleanupDiagnostic=Safe-Exception $_.Exception;$cleanupType=$cleanupDiagnostic.type;$cleanupOk=$false}
 if($resultCreated){try{
  Write-JsonFile ([ordered]@{status=if($cleanupOk){'exact-owned-cleanup-passed'}else{'failed'};nativeTrayQuitNatural=$trayQuitNatural;allExactGuiHandlesExited=$allGuiExited;allExactNodeHandlesExited=$allNodeExited;debugPortReleased=$portsReleased;embeddedPort=$embeddedPort;unexpectedDescendants=if($guiOwned){$guiOwned.UnexpectedDescendants}else{0};safeUnexpectedImages=@(Gui-SafeUnexpectedImages);guiOutcomes=$guiOutcomes;nodeOutcomes=$nodeOutcomes;type=$cleanupType;diagnostic=$cleanupDiagnostic;identityDiagnostic=[Orchester.FinalRuntime.OwnedProcesses]::LastIdentityDiagnostic;originalHandlesKeptUntilCleanup=$true;modelCalls=0;defaultServiceStopped=$false}) (Join-Path $ResultDirectory 'cleanup-result.json')
  if(-not $failed -and $cleanupOk){Write-JsonFile ([ordered]@{status='desktop-native-eighteen-checks-passed';nativeTrayQuitNatural=$trayQuitNatural;trayQuitResultSha256=$trayQuitSha256;nativeWindowsResultSha256=(File-Hash (Join-Path $ResultDirectory 'native-windows-result.json'));sourceRevision=$plan.sourceRevision;expectedPlanSha256=$ExpectedPlanSha256;expectedFinalResultSha256=$ExpectedFinalResultSha256;expectedRuntimeInputsSha256=$ExpectedRuntimeInputsSha256;expectedPreparedScriptsSha256=$ExpectedPreparedScriptsSha256;binarySha256=$build.desktopExeSha256;checks=18;writes=7;screenshotsReviewed=2;exactOwnedHandlesExited=$true;portsReleased=$true;modelCalls=0;defaultInstallationChanged=$false}) (Join-Path $ResultDirectory 'desktop-runtime-result.json')}
 }catch{$cleanupOk=$false}}
 }finally{
  if($resultCreated -and (Get-Command Retain-OriginalOwnedObservation -ErrorAction SilentlyContinue)){Retain-OriginalOwnedObservation $guiOwned $gui $ResultDirectory;Retain-OriginalOwnedObservation $nodeOwned $node $ResultDirectory}
 }
 if($http){$http.Dispose()}
 if($gui){$gui.Dispose()};if($node){$node.Dispose()}
 if($guiOwned){$guiOwned.Dispose()};if($nodeOwned){$nodeOwned.Dispose()}
}
if($failed -or -not $cleanupOk){exit 1}
Write-Output 'Desktop native verification completed: 18 checks, 7 writes, 2 reviewed screenshots, exact owned cleanup.'
exit 0
