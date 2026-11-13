# Actual worker4 binding; only root submits this parameterless job.
$ErrorActionPreference='Stop'
Set-StrictMode -Version 2.0
$heldInputs=[Collections.Generic.List[IO.FileStream]]::new()
try {
    $inputs=@(
        @{Path='C:\Users\han\AppData\Roaming\uv\python\cpython-3.14.9-windows-x86_64-none\python.exe';Sha='204d51f19707da17766bccecce2ceececadab350663a8190420d1377bac1150d'},
        @{Path='C:\Users\han\AppData\Local\Orchester-build\a5w4-1b2f8e6a\manager-bindings.json';Sha='3e804002d532472e1a8bd078396a6a8e68e7fe401d6a104c39d244c22b891889'},
        @{Path='C:\Users\han\AppData\Local\Orchester-build\a5w4-1b2f8e6a\preflight4-safe.json';Sha='fcf831326d0709f2a517056fd857186085c9aa63b0ef85bd00fd8a9c03da02fe'},
        @{Path='C:\Users\han\AppData\Local\Orchester-build\a5w4-1b2f8e6a\session-proof-safe.json';Sha='d23798cdc63cbcb7fb46cbe51e5c08152e9b4bac0f83081686edc0434dcaa9d1'},
        @{Path='C:\Users\han\AppData\Local\Orchester-build\a5w4-1b2f8e6a\src\manage-owned-windows-original-avd.py';Sha='d45cfa2267dfe7e43e2da2ca63063a5480a13df199bcd2af63bbd5e6827a9947'},
        @{Path='C:\Users\han\AppData\Local\Orchester-build\a5w4-1b2f8e6a\src\read-current-same-avd-processes.ps1';Sha='11795c1aa06f9ba6475158c5fde709d0eff4085fe41315e160e59cd3344bbc4e'}
    )
    foreach($ownedInput in $inputs){
        $stream=[IO.File]::Open($ownedInput.Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
        $heldInputs.Add($stream)
        $sha=[Security.Cryptography.SHA256]::Create()
        try {$actual=([BitConverter]::ToString($sha.ComputeHash($stream))).Replace('-','').ToLowerInvariant()}finally{$sha.Dispose()}
        if($actual -cne $ownedInput.Sha){throw 'owned-manager-input-pin-differs'}
        $stream.Position=0
    }
    $binding=Get-Content -LiteralPath 'C:\Users\han\AppData\Local\Orchester-build\a5w4-1b2f8e6a\manager-bindings.json' -Raw | ConvertFrom-Json
    if($binding.nativeWorker.pid -ne 432400 -or $binding.nativeWorker.createdExact -cne '134390532801688162' -or $binding.nativeWorker.sessionId -ne 1){throw 'current-worker-binding-differs'}
    $LASTEXITCODE=$null
    & 'C:\Users\han\AppData\Roaming\uv\python\cpython-3.14.9-windows-x86_64-none\python.exe' -I 'C:\Users\han\AppData\Local\Orchester-build\a5w4-1b2f8e6a\src\manage-owned-windows-original-avd.py' --bindings 'C:\Users\han\AppData\Local\Orchester-build\a5w4-1b2f8e6a\manager-bindings.json' --bindings-sha256 '3e804002d532472e1a8bd078396a6a8e68e7fe401d6a104c39d244c22b891889'
    if($LASTEXITCODE -isnot [int]){throw 'owned-manager-native-exit-unavailable'}
    exit $LASTEXITCODE
}catch {
    [Console]::Error.WriteLine('Owned original Windows AVD manager failed; raw errors/private values omitted.')
    exit 1
}finally {
    foreach($stream in $heldInputs){$stream.Dispose()}
}
