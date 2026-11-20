[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$InputFile,[Parameter(Mandatory=$true)][ValidatePattern('\A[a-f0-9]{64}\z')][string]$ExpectedInputSha256)
$ErrorActionPreference='Stop';Set-StrictMode -Version 2.0
$server=$null;$owned=$null;$stdout=$null;$stderr=$null;$code=1;$stage='input-pins';$directory=$null;$normalQuit=$false
function Hash-File([string]$Path){(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()}
function Need([bool]$Value,[string]$Label){if(-not $Value){throw [InvalidOperationException]::new($Label)}}
function New-Json([string]$Path,$Value){$text=(ConvertTo-Json -InputObject $Value -Depth 10)+"`n";$file=[IO.File]::Open($Path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None);try{$bytes=([Text.UTF8Encoding]::new($false)).GetBytes($text);$file.Write($bytes,0,$bytes.Length);$file.Flush($true)}finally{$file.Dispose()}}
function Current-Json([string]$Path,$Value){
 $nonce=[Guid]::NewGuid().ToString('N');$temporary=$Path+'.'+$nonce+'.tmp';$ownedPrevious=$Path+'.'+$nonce+'.owned-previous'
 Need (-not (Test-Path -LiteralPath $temporary) -and -not (Test-Path -LiteralPath $ownedPrevious)) 'owned-receipt-path-already-exists'
 $replaceAttempted=$false
 try{New-Json $temporary $Value;if(Test-Path -LiteralPath $Path){$replaceAttempted=$true;[IO.File]::Replace($temporary,$Path,$ownedPrevious)}else{[IO.File]::Move($temporary,$Path)}}
 finally{if($replaceAttempted -and [IO.File]::Exists($ownedPrevious)){[IO.File]::Delete($ownedPrevious)};if([IO.File]::Exists($temporary)){[IO.File]::Delete($temporary)}}
}
function Native-Arg([string]$Text){Need (-not $Text.Contains('"') -and -not $Text.EndsWith('\') -and $Text -notmatch '[\r\n]') 'native-argument-invalid';'"'+$Text+'"'}
function Own-Listeners{@(Get-NetTCPConnection -State Listen -LocalPort 7338 -ErrorAction SilentlyContinue)}
function Normal-Command{
 $request=Join-Path $directory 'request-managed-7338-normal-quit.json'
 if(-not (Test-Path -LiteralPath $request -PathType Leaf)){return $false}
 $value=Get-Content -LiteralPath $request -Raw|ConvertFrom-Json
 Need ($value.schemaVersion -eq 1 -and $value.action -ceq 'normal-quit' -and $value.remotePid -eq $owned.Root.Pid -and [string]$value.remoteCreatedFileTimeExact -ceq $owned.Root.Created.ToString() -and $value.remoteExeSha256 -ceq $owned.Root.Sha256 -and $value.managerSourceSha256 -ceq (Hash-File $PSCommandPath)) 'normal-command-identity-differs'
 Need ($owned.IsAlive($owned.Root)) 'normal-command-original-handle-ended'
 $server.StandardInput.WriteLine('quit');$server.StandardInput.Flush();return $true
}
try{
 Need ((Hash-File $InputFile)-ceq $ExpectedInputSha256) 'manager-input-pin-differs'
 $managedInput=Get-Content -LiteralPath $InputFile -Raw|ConvertFrom-Json
 Need ($managedInput.schemaVersion -eq 1 -and $managedInput.sourceRevision -ceq '71def643cb08b208cc79687f79255c504eb14dc6') 'manager-input-shape-differs'
 $directory=[IO.Path]::GetFullPath($managedInput.resultDirectory)
 Need ([IO.Path]::GetFullPath($InputFile) -ieq (Join-Path $directory 'managed-7338-input-private.json')) 'manager-input-location-differs'
 Need ((Hash-File $PSCommandPath)-ceq $managedInput.managerSourceSha256) 'manager-source-pin-differs'
 $helper='C:\Users\han\AppData\Local\Orchester-build\continue-20261111-monitors-final21-71def64\p18m-1113-bf492b45\OwnedRuntimeProcesses.cs'
 Need ((Hash-File $helper)-ceq 'd335743a632acae0c90968e482b87eda44aaf1dae241e938e62b5c7833598d45') 'owned-helper-pin-differs'
 Need ($managedInput.remoteExe -ieq 'C:\Users\han\Downloads\OrchesterRemote\orchester.exe' -and $managedInput.remoteExeSha256 -ceq '24c8f9806256cc54b239b03f6b4adc976c3fee50c30bc0f2ecf601971c22ce2a') 'manager-default-binary-differs'
 Need ((Hash-File $managedInput.remoteExe)-ceq $managedInput.remoteExeSha256) 'installed-backend-pin-differs'
 Need ($managedInput.stateRoot -ieq 'C:\Users\han\AppData\Local\OrchesterRemote\state-home' -and $managedInput.workspace -ceq '\\wsl.localhost\Ubuntu-22.04\home\han\project\Orchester' -and $managedInput.allowRoot -ceq '\\wsl.localhost\Ubuntu-22.04\home\han\project') 'original-remote-paths-differ'
 foreach($path in @($managedInput.stateRoot,$managedInput.workspace,$managedInput.allowRoot)){Need (Test-Path -LiteralPath $path -PathType Container) 'original-remote-directory-unavailable'}
 Need (@(Own-Listeners).Count -eq 0) 'foreign-7338-listener-present-untouched'
 Need (@(Get-CimInstance Win32_Process -Filter "Name='orchester.exe'" -OperationTimeoutSec 60 -ErrorAction Stop).Count -eq 0) 'existing-orchester-backend-present-untouched'
 if(-not ('Orchester.FinalRuntime.OwnedProcesses' -as [type])){Add-Type -Path $helper}
 $systemRoot=Join-Path $env:SystemRoot 'System32'
 $runtimePins=@(@{path=(Join-Path $systemRoot 'conhost.exe');sha='c86146188b78e484f32cc29e2331689659943b47689a85570342044f499938ac'},@{path=(Join-Path $systemRoot 'wsl.exe');sha='fcd08205b608b5c4831e0dcb1801dd274436cd2c8420815f18ae58a085891a18'},@{path='C:\Program Files\WSL\wsl.exe';sha='051b7a9b7bda66817c052aced998e026610e5d4d18d63f45629d35d5cb5471c8'})
 foreach($pin in $runtimePins){Need ((Hash-File $pin.path) -ceq $pin.sha) 'observed-system-child-image-pin-differs';$sig=Get-AuthenticodeSignature -LiteralPath $pin.path;Need ($sig.Status.ToString() -ceq 'Valid') 'observed-system-child-signature-not-valid'}
 $owned=[Orchester.FinalRuntime.OwnedProcesses]::new(@([string]$managedInput.remoteExe)+@($runtimePins.path))
 $stage='start-managed-original-state-backend';$utf8=[Text.UTF8Encoding]::new($false)
 $start=[Diagnostics.ProcessStartInfo]::new();$start.FileName=$managedInput.remoteExe;$start.WorkingDirectory=Split-Path -Parent $managedInput.remoteExe;$start.UseShellExecute=$false;$start.CreateNoWindow=$true
 $start.RedirectStandardInput=$true;$start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true;$start.StandardOutputEncoding=$utf8;$start.StandardErrorEncoding=$utf8
 $start.EnvironmentVariables['ORCHESTER_HOME']=$managedInput.stateRoot
 $start.Arguments='remote --workspace '+(Native-Arg $managedInput.workspace)+' --allow-root '+(Native-Arg $managedInput.allowRoot)+' --bind 0.0.0.0 --port 7338 --assets '+(Native-Arg (Join-Path $start.WorkingDirectory 'web'))+' --no-pairing --json'
 $server=[Diagnostics.Process]::new();$server.StartInfo=$start;Need ($server.Start()) 'managed-backend-start-failed'
 $owned.CaptureRoot($server,$managedInput.remoteExe,$managedInput.remoteExeSha256);$stdout=$server.StandardOutput.ReadToEndAsync();$stderr=$server.StandardError.ReadToEndAsync()
 $deadline=[DateTime]::UtcNow.AddSeconds(60);$listening=$false
 do{Need ($owned.IsAlive($owned.Root)) 'managed-backend-exited-before-listen';$owned.CaptureAllowedDescendants();$listeners=@(Own-Listeners);if($listeners.Count -gt 0){Need (@($listeners|Where-Object {$_.OwningProcess -ne $owned.Root.Pid -or $_.LocalAddress -cne '0.0.0.0'}).Count -eq 0) '7338-listener-owner-or-original-bind-differs';$listening=$true}else{Start-Sleep -Milliseconds 200}}while(-not $listening -and [DateTime]::UtcNow -lt $deadline)
 Need ($listening -and $owned.UnexpectedDescendants -eq 0) 'managed-backend-not-ready'
 $self=[Diagnostics.Process]::GetCurrentProcess()
 $ready=[ordered]@{schemaVersion=1;status='managed-original-state-7338-listener-verified-awaiting-normal-command';observedAtUtc=[DateTime]::UtcNow.ToString('o');managerPid=$self.Id;managerCreatedFileTimeExact=$self.StartTime.ToUniversalTime().ToFileTimeUtc().ToString();managerSourceSha256=$managedInput.managerSourceSha256;remotePid=$owned.Root.Pid;remoteCreatedFileTimeExact=$owned.Root.Created.ToString();remoteExeSha256=$owned.Root.Sha256;remoteExe=$owned.Root.Image;port=7338;bind='0.0.0.0';currentListenerOwnedByOriginalStartedProcess=$true;originalServerProcessAndKernelHandleRetained=$true;originalStateHomeUsed=$true;startupPairingDisabled=$true;pairingOrCredentialCommands=0;modelCalls=0;privateValuesIncluded=$false}
 New-Json (Join-Path $directory 'managed-7338-ready-safe.json') $ready
 $stage='retained-managed-7338';$observationError=$null
 while($owned.IsAlive($owned.Root)){
  try{
   $owned.CaptureAllowedDescendants();$listeners=@(Own-Listeners)
   $current=@{};foreach($key in $ready.Keys){$current[$key]=$ready[$key]};$current.observedAtUtc=[DateTime]::UtcNow.ToString('o');$current.originalHandleCurrentlyLive=$true
   $current.currentListenerOwnedByOriginalStartedProcess=($listeners.Count -gt 0 -and @($listeners|Where-Object {$_.OwningProcess -ne $owned.Root.Pid -or $_.LocalAddress -cne '0.0.0.0'}).Count -eq 0)
   $current.unexpectedDescendants=$owned.UnexpectedDescendants;$current.normalQuitRequested=$normalQuit
   Current-Json (Join-Path $directory 'managed-7338-current-safe.json') $current
   if(-not $normalQuit){$normalQuit=Normal-Command}
  }catch{$observationError=$_.Exception.GetBaseException().GetType().FullName;try{Current-Json (Join-Path $directory 'managed-7338-observation-error-safe.json') ([ordered]@{status='manager-error-original-server-handle-retained';exceptionType=$observationError;observedAtUtc=[DateTime]::UtcNow.ToString('o');remotePid=$owned.Root.Pid;signals=0;automaticRestart=$false;privateValuesIncluded=$false})}catch{}}
  Start-Sleep -Milliseconds 500
 }
 Need ($server.WaitForExit(2000)) 'managed-backend-normal-exit-not-proven'
 New-Json (Join-Path $directory 'managed-7338-original-server-exit-safe.json') ([ordered]@{schemaVersion=1;status='original-server-native-exit-observed';observedAtUtc=[DateTime]::UtcNow.ToString('o');remotePid=$owned.Root.Pid;remoteCreatedFileTimeExact=$owned.Root.Created.ToString();actualNativeServerExitCode=$server.ExitCode;originalCreatorProcessAndKernelHandleRetainedUntilExit=$true;normalQuitRequested=$normalQuit;forcedTerminations=0;privateValuesIncluded=$false})
 Need ($server.ExitCode -eq 0 -and $normalQuit -and $owned.AllExited() -and $owned.UnexpectedDescendants -eq 0) 'managed-backend-normal-exit-not-proven'
 Need (@(Own-Listeners).Count -eq 0) 'managed-backend-port-not-released'
 if($stdout -and $stdout.Wait(2000)){[IO.File]::WriteAllText((Join-Path $directory 'managed-7338-stdout-private.log'),$stdout.Result,$utf8)}
 if($stderr -and $stderr.Wait(2000)){[IO.File]::WriteAllText((Join-Path $directory 'managed-7338-stderr-private.log'),$stderr.Result,$utf8)}
 New-Json (Join-Path $directory 'managed-7338-normal-exit-safe.json') ([ordered]@{schemaVersion=1;status='original-managed-7338-normal-quit-and-owned-exit-verified';observedAtUtc=[DateTime]::UtcNow.ToString('o');actualNativeServerExitCode=0;originalHandlesRetainedUntilExit=$true;normalQuitCommandOnly=$true;allOwnedHandlesExited=$true;portReleased=$true;forcedTerminations=0;privateValuesIncluded=$false});$code=0
}catch{
 if($directory){try{
  $staticLabels=@('24c8f9806256cc54b239b03f6b4adc976c3fee50c30bc0f2ecf601971c22ce2a','71def643cb08b208cc79687f79255c504eb14dc6','7338-listener-owner-or-original-bind-differs','d335743a632acae0c90968e482b87eda44aaf1dae241e938e62b5c7833598d45','existing-orchester-backend-present-untouched','failed','foreign-7338-listener-present-untouched','input-pins','installed-backend-pin-differs','managed-7338-failure-','managed-backend-exited-before-listen','managed-backend-normal-exit-not-proven','managed-backend-not-ready','managed-backend-port-not-released','managed-backend-start-failed','managed-original-state-7338-listener-verified-awaiting-normal-command','manager-default-binary-differs','manager-error-original-server-handle-retained','manager-input-location-differs','manager-input-pin-differs','manager-input-shape-differs','manager-source-pin-differs','native-argument-invalid','normal-command-identity-differs','normal-command-original-handle-ended','normal-quit','o','original-managed-7338-normal-quit-and-owned-exit-verified','original-remote-directory-unavailable','original-remote-paths-differ','owned-helper-pin-differs','owned-receipt-path-already-exists','quit','retained-managed-7338','start-managed-original-state-backend','web')
  $errorMessage=$_.Exception.GetBaseException().Message
  $staticCode=if($errorMessage -cin $staticLabels){$errorMessage}else{$null}
  $diag=[ordered]@{schemaVersion=1;status='native-managed-7338-static-failure-observed';stage=$stage;sourceLine=$_.InvocationInfo.ScriptLineNumber;exceptionType=$_.Exception.GetBaseException().GetType().FullName;staticCode=$staticCode;ownedRootCaptured=($owned -and $owned.Root -ne $null);ownedIdentityCount=if($owned){$owned.Count}else{0};unexpectedDescendants=if($owned){$owned.UnexpectedDescendants}else{0};safeUnexpectedImages=if($owned){@($owned.SafeUnexpectedImages)}else{@()};lastHelperStaticIdentityDiagnostic=[Orchester.FinalRuntime.OwnedProcesses]::LastIdentityDiagnostic;originalServerStillLive=if($owned -and $owned.Root){$owned.IsAlive($owned.Root)}else{$null};privateMessagesArgumentsOrStateValuesIncluded=$false;forcedTerminations=0}
  New-Json (Join-Path $directory 'managed-7338-failure-static-safe.json') $diag
 }catch{}}
 if($directory){try{New-Json (Join-Path $directory ('managed-7338-failure-'+[Guid]::NewGuid().ToString('N')+'-safe.json')) ([ordered]@{status='failed';stage=$stage;exceptionType=$_.Exception.GetBaseException().GetType().FullName;observedAtUtc=[DateTime]::UtcNow.ToString('o');privateValuesIncluded=$false})}catch{}}}
finally{
 # A live backend remains held. No timeout, PID signalling, forced termination,
 # credential command or automatic restart is allowed by this management source.
 if($owned -and $owned.Root){while($owned.IsAlive($owned.Root)){try{if(-not $normalQuit){$normalQuit=Normal-Command}}catch{};Start-Sleep -Milliseconds 500}}
 if($owned){$owned.Dispose()};if($server){$server.Dispose()}
}
exit $code
