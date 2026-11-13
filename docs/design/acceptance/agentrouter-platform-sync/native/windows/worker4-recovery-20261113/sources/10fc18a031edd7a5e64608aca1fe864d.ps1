# Actual worker4 binding; only root submits this parameterless job.
$ErrorActionPreference='Stop'
Set-StrictMode -Version 2.0
$heldInputs=[Collections.Generic.List[IO.FileStream]]::new()
try {
    $inputs=@(
        @{Path='C:\Users\han\AppData\Roaming\uv\python\cpython-3.14.9-windows-x86_64-none\python.exe';Sha='204d51f19707da17766bccecce2ceececadab350663a8190420d1377bac1150d'},
        @{Path='C:\Users\han\AppData\Local\Orchester-build\a5w4-b03a222c\manager-bindings.json';Sha='9b904eddb4d1a4e20bbf55736743238ee248c7d382dfdc0fef5ba022ad0701cb'},
        @{Path='C:\Users\han\AppData\Local\Orchester-build\a5w4-b03a222c\preflight4-safe.json';Sha='2865224efe20c9e43910301943eae3b8f9c9c4b5222d19bddaaf3c7d53aec8c3'},
        @{Path='C:\Users\han\AppData\Local\Orchester-build\a5w4-b03a222c\session-proof-safe.json';Sha='93032f3abb174abfc13df00d23834df2f0b50e827c323b16d096ff9282033ee2'},
        @{Path='C:\Users\han\AppData\Local\Orchester-build\a5w4-b03a222c\src\manage-owned-windows-original-avd.py';Sha='bfabe11f7352809885167c75f6e0556c1ebe3ce64ff65537e534b413ecd97027'},
        @{Path='C:\Users\han\AppData\Local\Orchester-build\a5w4-b03a222c\src\read-current-same-avd-processes.ps1';Sha='11795c1aa06f9ba6475158c5fde709d0eff4085fe41315e160e59cd3344bbc4e'}
    )
    foreach($ownedInput in $inputs){
        $stream=[IO.File]::Open($ownedInput.Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
        $heldInputs.Add($stream)
        $sha=[Security.Cryptography.SHA256]::Create()
        try {$actual=([BitConverter]::ToString($sha.ComputeHash($stream))).Replace('-','').ToLowerInvariant()}finally{$sha.Dispose()}
        if($actual -cne $ownedInput.Sha){throw 'owned-manager-input-pin-differs'}
        $stream.Position=0
    }
    $binding=Get-Content -LiteralPath 'C:\Users\han\AppData\Local\Orchester-build\a5w4-b03a222c\manager-bindings.json' -Raw | ConvertFrom-Json
    if($binding.nativeWorker.pid -ne 432400 -or $binding.nativeWorker.createdExact -cne '134390532801688162' -or $binding.nativeWorker.sessionId -ne 1){throw 'current-worker-binding-differs'}
    $LASTEXITCODE=$null
    & 'C:\Users\han\AppData\Roaming\uv\python\cpython-3.14.9-windows-x86_64-none\python.exe' -I 'C:\Users\han\AppData\Local\Orchester-build\a5w4-b03a222c\src\manage-owned-windows-original-avd.py' --bindings 'C:\Users\han\AppData\Local\Orchester-build\a5w4-b03a222c\manager-bindings.json' --bindings-sha256 '9b904eddb4d1a4e20bbf55736743238ee248c7d382dfdc0fef5ba022ad0701cb'
    if($LASTEXITCODE -isnot [int]){throw 'owned-manager-native-exit-unavailable'}
    exit $LASTEXITCODE
}catch {
    [Console]::Error.WriteLine('Owned original Windows AVD manager failed; raw errors/private values omitted.')
    exit 1
}finally {
    foreach($stream in $heldInputs){$stream.Dispose()}
}
