$ErrorActionPreference='Stop'
$held=$null
try {
 $held=[Diagnostics.Process]::GetProcessById(432400)
 $nativeHandle=$held.Handle
 if($nativeHandle -eq [IntPtr]::Zero -or $held.HasExited -or $held.StartTime.ToUniversalTime().ToFileTimeUtc().ToString() -cne '134390532801688162'){throw 'current-worker-identity-differs'}
 $image=$held.MainModule.FileName
 if((Get-FileHash -LiteralPath $image -Algorithm SHA256).Hash.ToLowerInvariant() -cne '60c9b29843624dd8af6b6a7b26147753be6aeef65098b9a503cc50850afe410f'){throw 'current-worker-image-differs'}
 $session=$held.SessionId
 if($held.HasExited){throw 'current-worker-not-live-through-observation'}
 $r=[ordered]@{schemaVersion=1;status='fresh-held-current-worker-session-observed';observedAtUtc=[DateTime]::UtcNow.ToString('o');workerPid=432400;creationFiletimeExact='134390532801688162';imageSha256='60c9b29843624dd8af6b6a7b26147753be6aeef65098b9a503cc50850afe410f';sessionId=$session;nativeHandleHeldThroughObservation=$true;rawUserOrSidIncluded=$false}
 $path=Join-Path 'C:\Users\han\AppData\Local\Orchester-build\continuation-native-access-20261113\owned-native-worker-attempt4-1ed007a58623f12c8e1c1223\jobs\286166913689c89d58318e74d48d3a84' 'current-worker-session-safe.json'
 $text=ConvertTo-Json -InputObject $r -Depth 4
 [IO.File]::WriteAllText($path,$text,[Text.UTF8Encoding]::new($false))
}finally{if($held){$held.Dispose()}}
# Prepared only. Copy the pinned dependency to this new task directory first.
$ErrorActionPreference='Stop'
$sourceHandle=$null
try {
    $scriptPath='C:\Users\han\AppData\Local\Orchester-build\android5-current-wire-preflight-20261113\read-current-windows-android5-preflight-successor2.ps1'
    $expectedScriptSha256='dce8f140d1172f5df0961849918917891e65e032466ab80ee486682a2a43409c'
    $sourceHandle=[IO.File]::Open($scriptPath,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
    $sha=[Security.Cryptography.SHA256]::Create()
    try {$actual=([BitConverter]::ToString($sha.ComputeHash($sourceHandle))).Replace('-','').ToLowerInvariant()}finally{$sha.Dispose()}
    if($actual -cne $expectedScriptSha256){throw 'android5-preflight-source-pin-differs'}
    $sourceHandle.Position=0
    $LASTEXITCODE=$null
    & $scriptPath -OutputFile (Join-Path 'C:\Users\han\AppData\Local\Orchester-build\continuation-native-access-20261113\owned-native-worker-attempt4-1ed007a58623f12c8e1c1223\jobs\286166913689c89d58318e74d48d3a84' 'windows-android5-readonly-preflight-successor2-safe.json') -ExpectedWorkerPid 432400 -ExpectedWorkerCreationFiletime '134390532801688162' -ExpectedWorkerImageSha256 '60c9b29843624dd8af6b6a7b26147753be6aeef65098b9a503cc50850afe410f'
    if($LASTEXITCODE -isnot [int]){throw 'android5-preflight-exit-code-unavailable'}
    exit $LASTEXITCODE
}catch {
    [Console]::Error.WriteLine('Android5 readonly successor2 preflight failed before a successful receipt; no raw error values included.')
    exit 1
}finally {
    if($sourceHandle){$sourceHandle.Dispose()}
}
