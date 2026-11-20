$ErrorActionPreference='Stop';Set-StrictMode -Version 2.0
$out='C:\Users\han\AppData\Local\Orchester-build\default-finish-0e6df6887414';$dir='C:\Users\han\AppData\Local\Orchester-build\reinstall-backups\private-final21-offline-prm-1113-8798425b\runtime-12ec60f8c4144d89a6469b21b02368cf';$managerSource='C:\Users\han\AppData\Local\Orchester-build\continue-20261111-monitors-final21-71def64\prm-1113-8798425b\06-keep-original-state-managed-7338-system-children.ps1'
function Need([bool]$c,[string]$l){if(-not $c){throw [InvalidOperationException]::new($l)}}
$manager=$null;$server=$null
try{
 $manager=Get-Process -Id 4184 -ErrorAction Stop;$server=Get-Process -Id 21176 -ErrorAction Stop
 $mh=$manager.Handle;$sh=$server.Handle
 Need ($manager.StartTime.ToUniversalTime().ToFileTimeUtc().ToString() -ceq '134392960875953491' -and $server.StartTime.ToUniversalTime().ToFileTimeUtc().ToString() -ceq '134392960887170890' -and $manager.SessionId -eq 1 -and $server.SessionId -eq 1) 'original-held-process-birth-or-session-differs'
 Need ((Get-FileHash -LiteralPath $server.Path -Algorithm SHA256).Hash.ToLowerInvariant() -ceq '24c8f9806256cc54b239b03f6b4adc976c3fee50c30bc0f2ecf601971c22ce2a' -and (Get-FileHash -LiteralPath $managerSource -Algorithm SHA256).Hash.ToLowerInvariant() -ceq '3f0a3efb99ded06451159a9570b185cc91cab969aa31edf84315cf43d7a3fb10') 'original-source-or-image-changed'
 $c=Get-CimInstance Win32_Process -Filter 'ProcessId=21176' -OperationTimeoutSec 30;Need ($c.ParentProcessId -eq 4184) 'original-manager-parent-differs'
 $listeners=@(Get-NetTCPConnection -State Listen -LocalPort 7338 -ErrorAction Stop);Need ($listeners.Count -gt 0 -and @($listeners|Where-Object {$_.OwningProcess -ne 21176 -or $_.LocalAddress -cne '0.0.0.0'}).Count -eq 0) 'owned-listener-differs'
 $request=[ordered]@{schemaVersion=1;action='normal-quit';remotePid=21176;remoteCreatedFileTimeExact='134392960887170890';remoteExeSha256='24c8f9806256cc54b239b03f6b4adc976c3fee50c30bc0f2ecf601971c22ce2a';managerSourceSha256='3f0a3efb99ded06451159a9570b185cc91cab969aa31edf84315cf43d7a3fb10'}
 $f=[IO.File]::Open((Join-Path $dir 'request-managed-7338-normal-quit.json'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None);try{$b=([Text.UTF8Encoding]::new($false)).GetBytes(($request|ConvertTo-Json -Compress));$f.Write($b,0,$b.Length);$f.Flush($true)}finally{$f.Dispose()}
 Need ($server.WaitForExit(45000)) 'original-server-normal-exit-not-observed';Need ($manager.WaitForExit(10000)) 'original-manager-exit-not-observed'
 Need ($server.ExitCode -eq 0) 'original-server-exit-not-zero'
 Need (@(Get-NetTCPConnection -State Listen -LocalPort 7338 -ErrorAction SilentlyContinue).Count -eq 0) '7338-listener-still-present'
 [IO.File]::WriteAllText((Join-Path $out 'owned-failed-7338-normal-close-safe.json'),([ordered]@{schemaVersion=1;status='exact-owned-failed-attempt-normal-stdin-close-verified';observedAtUtc=[DateTime]::UtcNow.ToString('o');originalServerPid=21176;originalManagerPid=4184;originalServerExitCode=$server.ExitCode;originalManagerExitCode=$manager.ExitCode;heldProcessHandlesUntilExit=$true;normalStdinQuitThroughOriginalController=$true;originalManagerAcceptanceFailed=$true;'7338Released'=$true;forcedTerminations=0;pairingOrCredentialActions=0;modelCalls=0;privateValuesIncluded=$false}|ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
}finally{if($manager){$manager.Dispose()};if($server){$server.Dispose()}}
exit 0
