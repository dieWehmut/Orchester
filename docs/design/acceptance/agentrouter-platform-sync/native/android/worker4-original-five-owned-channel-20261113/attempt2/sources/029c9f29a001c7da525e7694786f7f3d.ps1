# Actual worker4 binding; only root submits this parameterless job.
$ErrorActionPreference='Stop'
Set-StrictMode -Version 2.0
$heldInputs=[Collections.Generic.List[IO.FileStream]]::new()
try {
    $inputs=@(
        @{Path='C:\Users\han\AppData\Roaming\uv\python\cpython-3.14.9-windows-x86_64-none\python.exe';Sha='204d51f19707da17766bccecce2ceececadab350663a8190420d1377bac1150d'},
        @{Path='C:\Users\han\AppData\Local\Orchester-build\a5w4-4896ca7d\manager-bindings.json';Sha='0ce84cad70e70f2068cc5278877f0a2449d8cbbc3a9bf6fa4d494d2acfd3c7a2'},
        @{Path='C:\Users\han\AppData\Local\Orchester-build\a5w4-4896ca7d\preflight4-safe.json';Sha='90b444de104bad2a85e341acbe155a675790c1ba6b751b0ab583e9a7ad64bdab'},
        @{Path='C:\Users\han\AppData\Local\Orchester-build\a5w4-4896ca7d\session-proof-safe.json';Sha='7b06f5b0ef535b23f6970bccfc73e12bfb95d96149e905d47f71139aa2bd84c8'},
        @{Path='C:\Users\han\AppData\Local\Orchester-build\a5w4-4896ca7d\src\manage-owned-windows-original-avd.py';Sha='6878d51581ed69c0d5baf70da758d2646d2c19ec1cc91c8f461e95d68a92f4f2'},
        @{Path='C:\Users\han\AppData\Local\Orchester-build\a5w4-4896ca7d\src\read-current-same-avd-processes.ps1';Sha='11795c1aa06f9ba6475158c5fde709d0eff4085fe41315e160e59cd3344bbc4e'}
    )
    foreach($ownedInput in $inputs){
        $stream=[IO.File]::Open($ownedInput.Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
        $heldInputs.Add($stream)
        $sha=[Security.Cryptography.SHA256]::Create()
        try {$actual=([BitConverter]::ToString($sha.ComputeHash($stream))).Replace('-','').ToLowerInvariant()}finally{$sha.Dispose()}
        if($actual -cne $ownedInput.Sha){throw 'owned-manager-input-pin-differs'}
        $stream.Position=0
    }
    $binding=Get-Content -LiteralPath 'C:\Users\han\AppData\Local\Orchester-build\a5w4-4896ca7d\manager-bindings.json' -Raw | ConvertFrom-Json
    if($binding.nativeWorker.pid -ne 432400 -or $binding.nativeWorker.createdExact -cne '134390532801688162' -or $binding.nativeWorker.sessionId -ne 1){throw 'current-worker-binding-differs'}
    $LASTEXITCODE=$null
    & 'C:\Users\han\AppData\Roaming\uv\python\cpython-3.14.9-windows-x86_64-none\python.exe' -I 'C:\Users\han\AppData\Local\Orchester-build\a5w4-4896ca7d\src\manage-owned-windows-original-avd.py' --bindings 'C:\Users\han\AppData\Local\Orchester-build\a5w4-4896ca7d\manager-bindings.json' --bindings-sha256 '0ce84cad70e70f2068cc5278877f0a2449d8cbbc3a9bf6fa4d494d2acfd3c7a2'
    if($LASTEXITCODE -isnot [int]){throw 'owned-manager-native-exit-unavailable'}
    exit $LASTEXITCODE
}catch {
    [Console]::Error.WriteLine('Owned original Windows AVD manager failed; raw errors/private values omitted.')
    exit 1
}finally {
    foreach($stream in $heldInputs){$stream.Dispose()}
}
