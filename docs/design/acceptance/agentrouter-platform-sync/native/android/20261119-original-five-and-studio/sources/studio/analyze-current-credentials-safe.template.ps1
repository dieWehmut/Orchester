# Public review template; native execution is disabled. Original source digest is in provenance-safe.json.
throw 'public-review-template-native-execution-disabled'
$ErrorActionPreference='Stop'
Set-StrictMode -Version 2.0
$taskRoot='<USERPROFILE>\AppData\Local\Orchester-build\android-studio-independent-20261119'
$out=Join-Path $taskRoot 'studio-credentials-current-analysis-corrected-safe.json'
$java='<USERPROFILE>\software\Androidstudio\AndroidS\jbr\bin\java.exe'
$jar='<USERPROFILE>\.android\cli\bundles\43dc5c48789c6d01ddf15ccb4e7958b231c1e2e3\main.jar'
$source='<USERPROFILE>\AppData\Local\Orchester-build\android-studio-21-monitors-continuation-20261111\apps\mobile\android\app\src\main\java\dev\orchester\mobile\CredentialsPlugin.java'
$studioImage='<USERPROFILE>\software\Androidstudio\AndroidS\bin\studio64.exe'
$studio=$null
$streams=[Collections.Generic.List[IO.FileStream]]::new()
$children=[Collections.Generic.List[Diagnostics.Process]]::new()
$phase='native-studio-and-source-admission'
$result=[ordered]@{schemaVersion=1;status='started';observedAtUtc=[DateTime]::UtcNow.ToString('o');studioPid=30552;studioCreatedExact='134392890488717106';processSignals=0;adbOperations=0;avdOperations=0;productSourceMutations=0;rawStdoutArchived=$false;rawStderrArchived=$false;privateValuesIncluded=$false;forcedTerminations=0}
function Sha-Text([string]$value){
  $hash=[Security.Cryptography.SHA256]::Create()
  try{return ([BitConverter]::ToString($hash.ComputeHash([Text.UTF8Encoding]::new($false).GetBytes($value)))).Replace('-','').ToLowerInvariant()}
  finally{$hash.Dispose()}
}
function Hold-Pin([string]$path,[string]$expected){
  $stream=[IO.File]::Open($path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
  $streams.Add($stream)
  $hash=[Security.Cryptography.SHA256]::Create()
  try{$actual=([BitConverter]::ToString($hash.ComputeHash($stream))).Replace('-','').ToLowerInvariant()}
  finally{$hash.Dispose()}
  $stream.Position=0
  if($actual -cne $expected){throw 'analysis-source-pin-differs'}
  return $actual
}
function Guard-Studio{
  if($studio.HasExited){throw 'original-studio-terminal-before-analysis-completion'}
  if($studio.StartTime.ToUniversalTime().ToFileTimeUtc() -ne 134392890488717106L -or $studio.MainModule.FileName -cne $studioImage){throw 'original-studio-identity-differs'}
}
function Invoke-Native([string]$label,[string]$arguments){
  Guard-Studio
  $si=[Diagnostics.ProcessStartInfo]::new()
  $si.FileName=$java
  $si.Arguments='-Dandroid.cli.dir=<USERPROFILE>\.android\cli -jar "'+$jar+'" '+$arguments
  $si.WorkingDirectory='<USERPROFILE>'
  $si.UseShellExecute=$false
  $si.RedirectStandardOutput=$true
  $si.RedirectStandardError=$true
  $si.CreateNoWindow=$true
  $taskNativeProcess=[Diagnostics.Process]::Start($si)
  $children.Add($taskNativeProcess)
  $held=$taskNativeProcess.Handle
  $birth=$taskNativeProcess.StartTime.ToUniversalTime().ToFileTimeUtc().ToString()
  $so=$taskNativeProcess.StandardOutput.ReadToEndAsync()
  $se=$taskNativeProcess.StandardError.ReadToEndAsync()
  while(-not $taskNativeProcess.WaitForExit(1000)){
    $progress=[ordered]@{schemaVersion=1;status='native-analysis-child-still-live';phase=$label;observedAtUtc=[DateTime]::UtcNow.ToString('o');childPid=$taskNativeProcess.Id;childCreatedExact=$birth;originalCreatorHandleHeld=$true;forcedTerminations=0;privateValuesIncluded=$false}
    [IO.File]::WriteAllText((Join-Path $taskRoot 'analysis-progress-safe.json'),($progress|ConvertTo-Json -Depth 5)+[Environment]::NewLine,[Text.UTF8Encoding]::new($false))
  }
  $taskNativeProcess.WaitForExit()
  $stdout=$so.Result
  $stderr=$se.Result
  Guard-Studio
  return [pscustomobject]@{label=$label;pid=$taskNativeProcess.Id;createdExact=$birth;nativeExitCode=$taskNativeProcess.ExitCode;actualOriginalHandleTerminal=$true;stdout=$stdout;stderr=$stderr;stdoutUtf8Sha256=(Sha-Text $stdout);stderrUtf8Sha256=(Sha-Text $stderr)}
}
try{
  $studio=[Diagnostics.Process]::GetProcessById(30552)
  $heldStudio=$studio.Handle
  Guard-Studio
  $result.studioImageSha256=Hold-Pin $studioImage 'a49ee7862667d8b0ddec9879a85a16be55d151e9878a328932087e6592f006d3'
  $result.javaSha256=Hold-Pin $java '397eca543a6761376f2704190aa08f1fde2f784c05e1b2b2bbec64c4dbcc4f41'
  $result.cliJarSha256=Hold-Pin $jar '8e9409225796e1b3619b9cfc3d8461c8ab301df70b852ca596b19708342a4701'
  $result.sourceSha256=Hold-Pin $source '735d1ad2e1e1fd58b9ed190829db3c48972861b6ca94db082a5ce8265a70390c'
  $result.studioCurrentIdentityVerified=$true
  $result.windowPresentBefore=($studio.MainWindowHandle -ne [IntPtr]::Zero)
  $result.respondingBefore=$studio.Responding
  $phase='actual-installed-cli-help'
  $help=Invoke-Native 'studio-analyze-help' 'studio analyze-file --help'
  $result.helpNativeExitCode=$help.nativeExitCode
  $result.helpStdoutUtf8Sha256=$help.stdoutUtf8Sha256
  $result.helpStderrUtf8Sha256=$help.stderrUtf8Sha256
  if($help.nativeExitCode -ne 0){throw 'installed-cli-help-not-passed'}
  $helpPlain=[regex]::Replace($help.stdout,[char]27+'\[[0-?]*[ -/]*[@-~]','')
  $result.cliHelpAdvertisesFormat=($helpPlain -match '--format')
  $phase='actual-studio-credentials-analysis'
  $analysis=Invoke-Native 'studio-credentials-analysis' ('studio analyze-file --pid=30552 --project=android "'+$source+'"')
  $plain=[regex]::Replace($analysis.stdout,[char]27+'\[[0-?]*[ -/]*[@-~]','')
  $errors=[regex]::Matches($plain,'(?m)^\s*ERROR\b').Count
  $warnings=[regex]::Matches($plain,'(?m)^\s*(?:WARNING|WEAK_WARNING)\b').Count
  $infos=[regex]::Matches($plain,'(?m)^\s*INFO\b').Count
  $result.actualNativeExitCode=$analysis.nativeExitCode
  $result.analysis=[ordered]@{file='CredentialsPlugin.java';errors=$errors;warnings=$warnings;infos=$infos;diagnosticCount=($errors+$warnings+$infos);fixedSourcePathReported=$plain.Contains($source);fixedStudioPidExplicitArgumentBound=$true;stdoutReportsStudioPid=($plain -match '\b30552\b');nativeChildPid=$analysis.pid;nativeChildCreatedExact=$analysis.createdExact;originalCreatorHandleActualTerminal=$analysis.actualOriginalHandleTerminal;stdoutUtf8Sha256=$analysis.stdoutUtf8Sha256;stderrUtf8Sha256=$analysis.stderrUtf8Sha256;severityCountsParsedAfterAnsiRemoval=$true;rawMessagesIncluded=$false}
  $result.originalStudioHandleHeldThroughAnalysis=$true
  $result.sourceReadLocksHeldThroughAnalysis=$true
  $result.studioIdentityVerifiedAfterAnalysis=$true
  $result.windowPresentAfter=($studio.MainWindowHandle -ne [IntPtr]::Zero)
  $result.respondingAfter=$studio.Responding
  $result.status=if($analysis.nativeExitCode -eq 0 -and $errors -eq 0 -and $warnings -eq 0 -and $result.analysis.fixedSourcePathReported -and $result.analysis.fixedStudioPidExplicitArgumentBound){'current-windows-studio-credentials-analysis-passed'}else{'current-windows-studio-analysis-result-not-passed'}
}catch{
  $result.status='current-windows-studio-analysis-failed'
  $result.failurePhase=$phase
  $result.errorType=$_.Exception.GetType().Name
  $result.rawErrorIncluded=$false
}finally{
  foreach($child in $children){
    while(-not $child.WaitForExit(1000)){}
    $child.WaitForExit()
  }
  $result.allNativeChildrenOriginalHandlesActuallyTerminal=$true
  $result.nativeCreatorChildCount=$children.Count
  $result.nativeCreatorExitCodes=@($children|ForEach-Object{$_.ExitCode})
  foreach($child in $children){$child.Dispose()}
  foreach($stream in $streams){$stream.Dispose()}
  if($null -ne $studio){$studio.Dispose();$result.studioQueryHandleClosed=$true}
  $result.observedAtUtc=[DateTime]::UtcNow.ToString('o')
  $json=$result|ConvertTo-Json -Depth 9
  [IO.File]::WriteAllText($out,$json+[Environment]::NewLine,[Text.UTF8Encoding]::new($false))
  [Console]::WriteLine($json)
}
if($result.status -cne 'current-windows-studio-credentials-analysis-passed'){exit 1}
