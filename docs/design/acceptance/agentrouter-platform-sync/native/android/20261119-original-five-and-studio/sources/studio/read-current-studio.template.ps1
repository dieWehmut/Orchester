# Public review template; native execution is disabled. Original source digest is in provenance-safe.json.
throw 'public-review-template-native-execution-disabled'
$ErrorActionPreference='Stop'
Set-StrictMode -Version 2.0
$out='<USERPROFILE>\AppData\Local\Orchester-build\android-studio-independent-20261119\current-studio-identity-safe.json'
$expectedImage='<USERPROFILE>\software\Androidstudio\AndroidS\bin\studio64.exe'
$expectedSha='a49ee7862667d8b0ddec9879a85a16be55d151e9878a328932087e6592f006d3'
$expectedBirth=134392890488717106L
$studio=$null
$result=[ordered]@{schemaVersion=1;status='identity-not-yet-verified';observedAtUtc=[DateTime]::UtcNow.ToString('o');studioPid=30552;processSignals=0;adbOperations=0;avdOperations=0;privateValuesIncluded=$false}
function Sha-File([string]$path){
  $stream=[IO.File]::Open($path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
  $hash=[Security.Cryptography.SHA256]::Create()
  try{return ([BitConverter]::ToString($hash.ComputeHash($stream))).Replace('-','').ToLowerInvariant()}
  finally{$hash.Dispose();$stream.Dispose()}
}
try{
  $studio=[Diagnostics.Process]::GetProcessById(30552)
  $held=$studio.Handle
  if($studio.HasExited){throw 'studio-not-live'}
  $birth=$studio.StartTime.ToUniversalTime().ToFileTimeUtc()
  $image=$studio.MainModule.FileName
  $sha=Sha-File $image
  if($birth -ne $expectedBirth -or $image -cne $expectedImage -or $sha -cne $expectedSha){throw 'studio-identity-differs'}
  $result.studioCreatedExact=$birth.ToString()
  $result.studioImageSha256=$sha
  $result.studioIdentityMatches=$true
  $result.originalQueryHandleHeld=$true
  $result.windowPresent=($studio.MainWindowHandle -ne [IntPtr]::Zero)
  $result.responding=$studio.Responding
  $result.sources=@()
  foreach($path in @(
    '<USERPROFILE>\software\Androidstudio\AndroidS\jbr\bin\java.exe',
    '<USERPROFILE>\.android\cli\bundles\43dc5c48789c6d01ddf15ccb4e7958b231c1e2e3\main.jar',
    '<USERPROFILE>\AppData\Local\Orchester-build\android-studio-21-monitors-continuation-20261111\apps\mobile\android\app\src\main\java\dev\orchester\mobile\CredentialsPlugin.java'
  )){
    $result.sources+=[ordered]@{path=$path;sha256=(Sha-File $path);bytes=(Get-Item -LiteralPath $path).Length}
  }
  $result.status='current-windows-studio-and-analysis-inputs-verified'
}catch{
  $result.status='current-studio-identity-verification-failed'
  $result.errorType=$_.Exception.GetType().Name
  $result.rawErrorIncluded=$false
}finally{
  if($null -ne $studio){$studio.Dispose();$result.queryHandleClosed=$true}
  $json=$result|ConvertTo-Json -Depth 8
  [IO.File]::WriteAllText($out,$json+[Environment]::NewLine,[Text.UTF8Encoding]::new($false))
  [Console]::WriteLine($json)
}
if($result.status -cne 'current-windows-studio-and-analysis-inputs-verified'){exit 1}
