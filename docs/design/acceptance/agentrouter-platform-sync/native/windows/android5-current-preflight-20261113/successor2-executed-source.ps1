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
    & $scriptPath -OutputFile (Join-Path 'C:\Users\han\AppData\Local\Orchester-build\continuation-native-access-20261113\owned-native-worker-attempt3-3fa131682683515d5d18a4e5\jobs\54fdcb3218245026a461792bad7688f3' 'windows-android5-readonly-preflight-successor2-safe.json') -ExpectedWorkerPid 326920 -ExpectedWorkerCreationFiletime '134390405872180528' -ExpectedWorkerImageSha256 '60c9b29843624dd8af6b6a7b26147753be6aeef65098b9a503cc50850afe410f'
    if($LASTEXITCODE -isnot [int]){throw 'android5-preflight-exit-code-unavailable'}
    exit $LASTEXITCODE
}catch {
    [Console]::Error.WriteLine('Android5 readonly successor2 preflight failed before a successful receipt; no raw error values included.')
    exit 1
}finally {
    if($sourceHandle){$sourceHandle.Dispose()}
}
