$ErrorActionPreference='Stop'
Set-StrictMode -Version 2.0
$out=$PSScriptRoot;$proc=$null;$owned=$null;$code=1;$stage='current-native-winget-preflight';$started=$false
function Save([string]$name,$value){[IO.File]::WriteAllText((Join-Path $out $name),(ConvertTo-Json -InputObject $value -Depth 10)+"`n",[Text.UTF8Encoding]::new($false))}
try{
 $pkg=@(Get-AppxPackage -Name Microsoft.DesktopAppInstaller -ErrorAction Stop);if($pkg.Count -ne 1){throw 'current-AppInstaller-not-unique'}
 $exe=Join-Path $pkg[0].InstallLocation 'winget.exe';$hash=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant();$sig=Get-AuthenticodeSignature -LiteralPath $exe
 if($sig.Status.ToString() -cne 'Valid'){throw 'native-Microsoft-WinGet-signature-required'}
 $src=Join-Path $out 'OwnedClaudeInstall.cs';if((Get-FileHash -LiteralPath $src -Algorithm SHA256).Hash.ToLowerInvariant() -cne 'c44e4cc7947d500765a7f6d214bae54c1ac52a59d2a6de6c78632c286a345a5f'){throw 'owned-source-pin-mismatch'}
 Add-Type -Path $src
 $si=[Diagnostics.ProcessStartInfo]::new();$si.FileName=$exe;$si.Arguments='show --id 9P6K58THS811 --source msstore --exact --accept-source-agreements --disable-interactivity';$si.WorkingDirectory=$out;$si.UseShellExecute=$false;$si.CreateNoWindow=$true;$si.RedirectStandardOutput=$true;$si.RedirectStandardError=$true
 $proc=[Diagnostics.Process]::new();$proc.StartInfo=$si;$started=$proc.Start();if(-not $started){throw 'winget-show-start-failed'};$handle=$proc.Handle
 $owned=[Orchester.ClaudeInstall1116.OwnedInstall]::new();$owned.CaptureRoot($proc,$exe,$hash);$stdout=$proc.StandardOutput.ReadToEndAsync();$stderr=$proc.StandardError.ReadToEndAsync();$stage='original-native-winget-show-lifetime'
 Save 'winget-store-show-start-safe.json' ([ordered]@{status='native-winget-current-official-Store-show-actually-started';observedAtUtc=[DateTime]::UtcNow.ToString('o');arguments=$si.Arguments;wingetVersion=$pkg[0].Version.ToString();wingetImageSha256=$hash;wingetAuthenticodeStatus=$sig.Status.ToString();nativePid=$owned.Root.Pid;nativeCreatedFileTimeExact=$owned.Root.Created.ToString();originalCreatorAndQueryHandlesHeld=$true;installOperations=0;processSignals=0})
 $quiet=0;do{$owned.Capture();if($owned.LiveCount() -eq 0){$quiet++}else{$quiet=0};Start-Sleep -Milliseconds 50}while($quiet -lt 3)
 $proc.WaitForExit();$stdout.Wait();$stderr.Wait();$nativeCode=$proc.ExitCode
 [IO.File]::WriteAllText((Join-Path $out 'winget-store-show-stdout-private.log'),$stdout.Result,[Text.UTF8Encoding]::new($false));[IO.File]::WriteAllText((Join-Path $out 'winget-store-show-stderr-private.log'),$stderr.Result,[Text.UTF8Encoding]::new($false))
 $r=[ordered]@{status='native-winget-current-official-Store-show-terminal';observedAtUtc=[DateTime]::UtcNow.ToString('o');actualNativeExitCode=$nativeCode;originalCreatorAndQueryHandlesHeldToExit=$true;identities=@($owned.Records());publicCatalogProductId='9P6K58THS811';showStdoutBytes=([Text.Encoding]::UTF8.GetByteCount($stdout.Result));showStderrBytes=([Text.Encoding]::UTF8.GetByteCount($stderr.Result));publicErrorCodes=@([regex]::Matches(($stdout.Result+$stderr.Result),'0x[0-9A-Fa-f]{8}')|ForEach-Object {$_.Value.ToLowerInvariant()}|Select-Object -Unique);storeInstallerMetadataPresent=($stdout.Result -match '(?i)msstore');nativeShowReturnedSuccess=($nativeCode -eq 0);installOperations=0;explicitLoginActions=0;modelAuthCalls=0;profileValuesRead=$false;sharedWorkerOperations=0;processSignals=0}
 Save 'winget-store-show-terminal-safe.json' $r;$r|ConvertTo-Json -Depth 10;$code=if($nativeCode -eq 0){0}else{1}
}catch{$r=[ordered]@{status='native-current-Store-show-observer-failed';stage=$stage;started=$started;errorType=$_.Exception.GetBaseException().GetType().FullName;originalHeldLiveCount=if($owned){$owned.LiveCount()}else{0};installOperations=0;processSignals=0};Save 'winget-store-show-observer-failure-safe.json' $r;$r|ConvertTo-Json -Depth 10}
finally{if($owned){while($owned.LiveCount() -gt 0){Start-Sleep -Milliseconds 500};$owned.Dispose()};if($proc){if($started){$proc.WaitForExit()};$proc.Dispose()}}
exit $code
