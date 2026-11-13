# Prepared only. Copy the pinned dependency to this new task directory first.
$ErrorActionPreference='Stop'
$sourceHandle=$null
try {
    $scriptPath='C:\Users\han\AppData\Local\Orchester-build\android5-current-wire-preflight-20261113\read-current-windows-android5-preflight.ps1'
    $expectedScriptSha256='2712818f73e1369a484455f7344f9e47ed2fe711fecb2082ab400798fb4d02e9'
    $sourceHandle=[IO.File]::Open($scriptPath,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
    $sha=[Security.Cryptography.SHA256]::Create()
    try {$actual=([BitConverter]::ToString($sha.ComputeHash($sourceHandle))).Replace('-','').ToLowerInvariant()}finally{$sha.Dispose()}
    if($actual -cne $expectedScriptSha256){throw 'android5-preflight-source-pin-differs'}
    $sourceHandle.Position=0
    $LASTEXITCODE=$null
    & $scriptPath -OutputFile (Join-Path 'C:\Users\han\AppData\Local\Orchester-build\continuation-native-access-20261113\owned-native-worker-attempt3-3fa131682683515d5d18a4e5\jobs\8e41a9c881c2b3d174c44fd9d47904f8' 'windows-android5-readonly-preflight-safe.json') -ExpectedWorkerPid 326920 -ExpectedWorkerCreationFiletime '134390405872180528' -ExpectedWorkerImageSha256 '60c9b29843624dd8af6b6a7b26147753be6aeef65098b9a503cc50850afe410f'
    if($LASTEXITCODE -isnot [int]){throw 'android5-preflight-exit-code-unavailable'}
    exit $LASTEXITCODE
}catch {
    [Console]::Error.WriteLine('Android5 readonly preflight failed before a successful receipt; no raw error values included.')
    exit 1
}finally {
    if($sourceHandle){$sourceHandle.Dispose()}
}
