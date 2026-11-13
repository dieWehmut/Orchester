Set-StrictMode -Version 2.0
$ErrorActionPreference='Stop'
$streams=@()
$stage='same-original-windows-user'
try {
    $oldRoot='C:\Users\han\AppData\Local\Orchester-build\continuation-native-access-20261113\owned-native-worker-attempt3-3fa131682683515d5d18a4e5'
    $originalSid=([IO.DirectoryInfo]::new($oldRoot)).GetAccessControl().GetOwner([Security.Principal.SecurityIdentifier]).Value
    $currentSid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    if($currentSid -cne $originalSid){throw 'original-windows-user-differs'}
    $root=[IO.Path]::GetFullPath($PSScriptRoot)
    $boot=Join-Path $root 'bootstrap.ps1'
    $worker=Join-Path $root 'worker.ps1'
    $stage='pinned-native-files'
    $files=@(
        @{path=$boot;sha='619edb8801695595d2ac4d8bf35463bfcecae3d8dff454c1942f17cd006e97aa'},
        @{path=$worker;sha='16ee5cae10190473a2f05b1c496d1323b1c929763d3ce2b9b37570f976b461b5'},
        @{path=(Join-Path $PSHOME 'powershell.exe');sha='60c9b29843624dd8af6b6a7b26147753be6aeef65098b9a503cc50850afe410f'}
    )
    foreach($file in $files){
        if(([IO.File]::GetAttributes($file.path)-band[IO.FileAttributes]::ReparsePoint)-ne 0){throw 'pinned-file-is-reparse'}
        $stream=[IO.File]::Open($file.path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
        $streams+=,$stream
        $hash=[Security.Cryptography.SHA256]::Create()
        try{$actual=([BitConverter]::ToString($hash.ComputeHash($stream))).Replace('-','').ToLowerInvariant()}finally{$hash.Dispose()}
        if($actual -cne $file.sha){throw 'pinned-file-differs'}
        $stream.Position=0
    }
    $stage='no-existing-owned-bootstrap-or-worker-candidate'
    $candidates=@(Get-CimInstance Win32_Process -Filter "Name = 'powershell.exe'" | Where-Object {
        $null -ne $_.CommandLine -and ($_.CommandLine.IndexOf($boot,[StringComparison]::OrdinalIgnoreCase)-ge 0 -or $_.CommandLine.IndexOf($worker,[StringComparison]::OrdinalIgnoreCase)-ge 0)
    })
    if($candidates.Count -ne 0){throw 'existing-owned-process-candidate-requires-observation'}
    foreach($name in @('bootstrap.lock','worker.lock','bootstrap.started-safe.json','bootstrap.terminal-safe.json','worker.ready-safe.json','worker.terminal-safe.json')){
        if([IO.File]::Exists((Join-Path $root $name))){throw 'existing-bootstrap-or-worker-record-requires-observation'}
    }
    $stage='invoke-byte-pinned-bootstrap-once'
    $LASTEXITCODE=$null
    & $boot
    if($LASTEXITCODE -isnot [int]){throw 'bootstrap-exit-code-unavailable'}
    exit $LASTEXITCODE
}catch{
    [Console]::WriteLine((ConvertTo-Json -InputObject ([ordered]@{state='guard-rejected';stage=$stage;exceptionType=$_.Exception.GetType().FullName;rawMessagesIncluded=$false}) -Compress))
    exit 1
}finally{
    foreach($stream in $streams){$stream.Dispose()}
}
