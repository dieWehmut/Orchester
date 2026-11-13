$ErrorActionPreference='Stop'
$launchFile='C:\Users\han\AppData\Local\Orchester-build\monitor-final-20261110\monitors-final21-71def64-20261111\desktop-runtime-native-20261112T1545-root\launch-result.json'
$node='C:\Users\han\AppData\Local\dsh-shortcut\node\node-v24.21.0-win-x64\node.exe'
$diagnostic='C:\Users\han\AppData\Local\Orchester-build\continuation-native-access-20261113\recovered-current-20261113\read-current-monitor-cdp-successor.mjs'
$outRoot='C:\Users\han\AppData\Local\Orchester-build\continuation-native-access-20261113\owned-native-worker-attempt3-3fa131682683515d5d18a4e5\jobs\6567ecac4bdf45dc33d773977a4e08c3'
$held=New-Object 'Collections.Generic.List[Diagnostics.Process]'
$result=[ordered]@{schemaVersion=1;status='held-original-gui-readonly-diagnostic-failed';actualTargetHost='Windows';collectorPid=$PID;nativeHandleGatePassed=$false;diagnosticInvoked=$false;targetGuiStarts=0;modelCalls=0;processSignals=0}
$code=1
$stage='gui-handle-and-original-identity'
try {
 $gui=[Diagnostics.Process]::GetProcessById(37324);$held.Add($gui);$h=$gui.Handle
 if($h -eq [IntPtr]::Zero -or $gui.HasExited){throw 'original-gui-not-live'}
 if($gui.StartTime.ToUniversalTime().ToFileTimeUtc().ToString() -cne '134389718070239855'){throw 'original-gui-birth-differs'}
 if((Get-FileHash -LiteralPath $gui.MainModule.FileName -Algorithm SHA256).Hash.ToLowerInvariant() -cne 'e4aa765befa85c7fb053075844be5693685c00677dd62b4cfff4592ec68c1d05'){throw 'original-gui-image-differs'}
 $nativeSession=[Diagnostics.Process]::GetCurrentProcess().SessionId
 if($gui.SessionId -ne $nativeSession){throw 'current-session-differs'}
 $stage='current-cdp-owner-chain'
 $listeners=@(Get-NetTCPConnection -State Listen -ErrorAction Stop | Where-Object {$_.LocalPort -eq 19322})
 $owners=@($listeners | ForEach-Object {$_.OwningProcess} | Sort-Object -Unique)
 if($owners.Count -ne 1){throw 'cdp-owner-not-unique'}
 $current=[int]$owners[0];$priorBirth=[Int64]::MaxValue;$chain=@();$rootFound=$false
 for($index=0;$index -lt 12;$index++){
  if($current -eq 37324){$rootFound=$true;break}
  $child=[Diagnostics.Process]::GetProcessById($current);$held.Add($child);$ch=$child.Handle
  if($ch -eq [IntPtr]::Zero -or $child.HasExited -or $child.SessionId -ne $nativeSession){throw 'cdp-chain-not-current'}
  $birth=$child.StartTime.ToUniversalTime().ToFileTimeUtc()
  if($birth -gt $priorBirth -or $birth -lt $gui.StartTime.ToUniversalTime().ToFileTimeUtc()){throw 'cdp-chain-birth-order-differs'}
  $priorBirth=$birth
  $item=Get-CimInstance Win32_Process -Filter "ProcessId=$current" -ErrorAction Stop
  if($null -eq $item -or [Math]::Abs([Int64]($item.CreationDate.ToUniversalTime().ToFileTimeUtc()-$birth)) -gt 9){throw 'cdp-cim-identity-differs'}
  $chain+=@([ordered]@{processId=$current;createdFileTimeExact=$birth.ToString();parentProcessId=[int]$item.ParentProcessId})
  $current=[int]$item.ParentProcessId
 }
 if(!$rootFound){throw 'cdp-owner-chain-does-not-reach-original-gui'}
 $stage='previous-readonly-attempts'
 $attempts=@(Get-CimInstance Win32_Process -Filter "Name='node.exe' AND (CommandLine LIKE '%root-monitor-readonly.mjs%' OR CommandLine LIKE '%read-current-monitor-cdp.mjs%' OR CommandLine LIKE '%read-current-monitor-cdp-successor.mjs%')" -ErrorAction Stop)
 if($attempts.Count -ne 0){throw 'prior-readonly-node-attempt-still-present'}
 $stage='native-node-and-diagnostic-pins'
 if((Get-FileHash -LiteralPath $node -Algorithm SHA256).Hash.ToLowerInvariant() -cne 'ba4e6d110e8c1592a1ecd390f6b05f3da124b13871a5be62b341a07a853c6c32'){throw 'native-node-image-differs'}
 if((Get-FileHash -LiteralPath $diagnostic -Algorithm SHA256).Hash.ToLowerInvariant() -cne '65bf0d72a652c8b6ee12216dc4fb12e5ecc52973bf8726fdd582295b0ffde543'){throw 'diagnostic-source-differs'}
 $stage='native-gate-write'
 $gate=[ordered]@{schemaVersion=1;status='current-native-original-gui-and-cdp-owner-verified';actualTargetHost='Windows';observedAtUtc=[DateTime]::UtcNow.ToString('o');launchResultSha256=(Get-FileHash -LiteralPath $launchFile -Algorithm SHA256).Hash.ToLowerInvariant();actualNativeQueryExitCode=0;originalNativeHandleIdentityVerified=$true;currentCdpPortOwnerIdentityVerified=$true;originalReadonlyNodeAttemptsAccountedFor=$true;guiPid=37324;guiCreatedFileTimeExact='134389718070239855';currentCdpOwnerPid=[int]$owners[0];currentCdpOwnerChain=$chain;nativeHandleHeldCount=$held.Count;collectorSourceSha256=(Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash.ToLowerInvariant()}
 $gateFile=Join-Path $outRoot 'current-held-original-gui-native-gate-safe.json'
 [IO.File]::WriteAllText($gateFile,(ConvertTo-Json -InputObject $gate -Depth 8),(New-Object Text.UTF8Encoding($false)))
 $gateSha=(Get-FileHash -LiteralPath $gateFile -Algorithm SHA256).Hash.ToLowerInvariant()
 $output=Join-Path $outRoot 'current-monitor-readonly-diagnostic-safe.json'
 $result.nativeHandleGatePassed=$true
 $stage='readonly-diagnostic-native-invocation'
 $worker=Start-Process -FilePath $node -ArgumentList @($diagnostic,$launchFile,$output,$gateFile,$gateSha) -NoNewWindow -PassThru
 $result.diagnosticInvoked=$true
 try {
  $nativeNodeHandle=$worker.Handle
  if($nativeNodeHandle -eq [IntPtr]::Zero){throw 'diagnostic-node-handle-unavailable'}
  $worker.WaitForExit()
  $observedCode=$worker.ExitCode
  if($null -eq $observedCode -or $observedCode -isnot [int]){throw 'diagnostic-node-exit-code-unavailable'}
  $code=$observedCode
  $result.actualDiagnosticNativeExitCode=$code
  $result.guiOriginalHandleStillLiveAfterDiagnostic=(!$gui.HasExited)
  $result.nativeHandleHeldCountThroughoutDiagnostic=$held.Count
  $result.nativeGateSha256=$gateSha
  $result.diagnosticOutputExists=Test-Path -LiteralPath $output
  $result.status='held-original-gui-readonly-diagnostic-terminal'
 } finally {$worker.Dispose()}
} catch {$code=1;$result.stage=$stage;$result.errorType=$_.Exception.GetType().Name;if(!$result.Contains('actualDiagnosticNativeExitCode')){$result.actualDiagnosticNativeExitCode=$null}}
finally {foreach($item in $held){$item.Dispose()}}
$result.actualOuterNativeExitCode=$code
$result.observedAtUtc=[DateTime]::UtcNow.ToString('o')
$result.sourceSha256=(Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText((Join-Path $outRoot 'held-original-gui-diagnostic-terminal-safe.json'),(ConvertTo-Json -InputObject $result -Depth 8),(New-Object Text.UTF8Encoding($false)))
Write-Output (ConvertTo-Json -InputObject $result -Depth 8)
exit $code
