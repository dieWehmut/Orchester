$ErrorActionPreference='Stop';Set-StrictMode -Version 2.0
$root='C:\Users\han\AppData\Local\Orchester-build\default-finish-0e6df6887414';$source=Join-Path $root '11-refresh-current-stable-managed.ps1';$exe='C:\WINDOWS\System32\WindowsPowerShell\v1.0\powershell.exe';$utf8=[Text.UTF8Encoding]::new($false);$p=$null
function Hash([string]$path){(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()}
if((Hash $source)-cne 'ab1b77988a2a180c0041bf571e27d75e7e1598962480fd6bae6aafe391e6dded' -or (Hash $exe)-cne '60c9b29843624dd8af6b6a7b26147753be6aeef65098b9a503cc50850afe410f'){throw 'native-observer-source-or-exe-pin-differs'}
$dir=Join-Path $root ('observer-wrapper-'+[Guid]::NewGuid().ToString('N'));[IO.Directory]::CreateDirectory($dir)|Out-Null
$s=[Diagnostics.ProcessStartInfo]::new();$s.FileName=$exe;$s.WorkingDirectory='C:\Users\han';$s.UseShellExecute=$false;$s.CreateNoWindow=$true;$s.RedirectStandardOutput=$true;$s.RedirectStandardError=$true;$s.StandardOutputEncoding=$utf8;$s.StandardErrorEncoding=$utf8;$s.Arguments='-NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "'+$source+'" -ObserveSeconds 30'
try{
 $p=[Diagnostics.Process]::new();$p.StartInfo=$s;if(-not $p.Start()){throw 'native-observer-start-failed'};$h=$p.Handle;$birth=$p.StartTime.ToUniversalTime().ToFileTimeUtc().ToString();$out=$p.StandardOutput.ReadToEndAsync();$err=$p.StandardError.ReadToEndAsync()
 $p.WaitForExit();if(-not $out.Wait(2000) -or -not $err.Wait(2000)){throw 'native-observer-output-not-closed'}
 [IO.File]::WriteAllText((Join-Path $dir 'native-observer-stdout-safe.log'),$out.Result,$utf8);[IO.File]::WriteAllText((Join-Path $dir 'native-observer-stderr-safe.log'),$err.Result,$utf8)
 $r=[ordered]@{schemaVersion=1;status=if($p.ExitCode -eq 0){'native-wrapper-observed-original-observer-exit-zero'}else{'native-wrapper-observed-original-observer-failure'};observedAtUtc=[DateTime]::UtcNow.ToString('o');actualNativeObserverExitCode=$p.ExitCode;observerPid=$p.Id;observerCreatedFileTimeExact=$birth;originalObserverKernelAndProcessHandleRetainedUntilExit=$true;observerCreateNoWindow=$true;observerSourceSha256=(Hash $source);observerExeSha256=(Hash $exe);nativeCommandWorkingDirectory='C:\Users\han';wrapperSourceSha256=(Hash $PSCommandPath);nativeStdoutSha256=(Hash (Join-Path $dir 'native-observer-stdout-safe.log'));nativeStderrSha256=(Hash (Join-Path $dir 'native-observer-stderr-safe.log'));wrapperResultDirectory=$dir;processSignals=0;runtimeRestartCount=0;credentialCommands=0;modelCalls=0;privateValuesIncluded=$false}
 $text=$r|ConvertTo-Json -Depth 8;[IO.File]::WriteAllText((Join-Path $dir 'native-observer-execution-safe.json'),$text,$utf8);$text
 if($p.ExitCode -ne 0){exit 1}
}finally{if($p){$p.Dispose()}}
exit 0
