# Public review template; native execution is disabled. Original source digest is in provenance-safe.json.
throw 'public-review-template-native-execution-disabled'
# Root materializes only the Linux-owner SHA placeholder, then submits this native job.
$ErrorActionPreference='Stop'
Set-StrictMode -Version 2.0
$taskRoot='<USERPROFILE>\AppData\Local\Orchester-build\android-resume-original5-20261119'
$attempt='7ec170eaf03e41c8a0fd080d67c7853d'
$linuxOwnerSha='306dcc4ffe1a41ebb9454ee6c582dc1864e02e0cc7894e4108b956e42c091009'
$linuxOwnerFile=Join-Path $taskRoot ('linux-owner-'+$attempt+'-safe.json')
$metadataSha='e6e6b72a2617b84821736d55a3e2f39cd7c5f5f9747df1e87571653c4620f46c'
$managerBindingSha='a0994a5e9fe37be0f14dc590ce592c851c6d1203aa095541884848cb9f878787'
$namespace='0b12b11bcb5c477aba71c94aab73367b'
$managerIdentity='<USERPROFILE>\AppData\Local\Orchester-build\android5-current-wire-runtime-20261113\runtime-0b12b11bcb5c477aba71c94aab73367b\identity-safe.json'
$heldFiles=[Collections.Generic.List[IO.FileStream]]::new()
$phase='source-and-root-Linux-proof-pin-admission'
function Hold-ReadPin([string]$path,[string]$expected){
    $stream=[IO.File]::Open($path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
    $script:heldFiles.Add($stream)
    $hash=[Security.Cryptography.SHA256]::Create()
    try {$actual=([BitConverter]::ToString($hash.ComputeHash($stream))).Replace('-','').ToLowerInvariant()}finally{$hash.Dispose()}
    if($actual -cne $expected){throw 'self-refresh-wire-input-pin-differs'}
    $stream.Position=0
    return $actual
}
function Read-LockedOutput([string]$path){
    $stream=[IO.File]::Open($path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
    $script:heldFiles.Add($stream)
    $hash=[Security.Cryptography.SHA256]::Create()
    try {$actual=([BitConverter]::ToString($hash.ComputeHash($stream))).Replace('-','').ToLowerInvariant()}finally{$hash.Dispose()}
    $stream.Position=0
    return $actual
}
function Write-NewHeldBytes([string]$path,[byte[]]$bytes){
    $writer=[IO.File]::Open($path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
    try {$writer.Write($bytes,0,$bytes.Length);$writer.Flush($true)}finally{$writer.Dispose()}
    return (Read-LockedOutput $path)
}
function Write-NewHeldJson([string]$path,$value){
    $bytes=[Text.UTF8Encoding]::new($false).GetBytes(($value|ConvertTo-Json -Depth 14)+[Environment]::NewLine)
    return (Write-NewHeldBytes $path $bytes)
}
try {
    if($attempt -cnotmatch '^[a-f0-9]{32}$' -or $linuxOwnerSha -cnotmatch '^[a-f0-9]{64}$'){throw 'actual-root-inputs-required'}
    $sources=@(
        @{Path='<USERPROFILE>\AppData\Roaming\uv\python\cpython-3.14.9-windows-x86_64-none\python.exe';Sha='204d51f19707da17766bccecce2ceececadab350663a8190420d1377bac1150d'},
        @{Path=(Join-Path $taskRoot 'refresh-current-identity.py');Sha='390ea55fa0e0d6d517424816c3496377f03332c8cae91efd38ec098c4324dc70'},
        @{Path=(Join-Path $taskRoot 'src\native_wire_common.py');Sha='e5c7c058c9d6644e4a9a11296ac4b1f04ccd8a07f534c6059f814dc6ea1e79b7'},
        @{Path=(Join-Path $taskRoot 'src\read_installed_app_binding.py');Sha='985e0dddfc8f9c3777320534316ed33c65f82b137a30cf1f023b118112a075ea'},
        @{Path=(Join-Path $taskRoot 'src\native_five_current_wire.py');Sha='52a2f0aa4431f2f3211cd823206182cb085c1bd5c2adb6be33755a5edf0d099e'},
        @{Path=(Join-Path $taskRoot 'src\cdp_transport_owned.py');Sha='d4d66a18e453cbaf7e6e71bc036ad40372e54f6d633429254b6961817d07cf05'},
        @{Path=(Join-Path $taskRoot 'src\native_five_wire_expression.js');Sha='14bc742b9de3ed8e4b8f5434aeb2a0bda2db600a822a9c53f7662dcc955b9ae3'},
        @{Path=(Join-Path $taskRoot 'src\ExistingWindowsAliasGuard.java');Sha='3071fcc2dd91a1a459327dfc0462471c30585bdbbf0bcef587937dc07d6cdf5e'},
        @{Path=(Join-Path $taskRoot 'src\dex\classes.dex');Sha='df15578ba781311a7a00377af05c387731c663906ec4174d7a8f46ee9a5ebd31'},
        @{Path=(Join-Path $taskRoot 'src\wait_owned_android_boot.py');Sha='5a7422457887bae319e8e5aab63871b34d13da18c89ddbe91cebc76aa2a4a840'},
        @{Path=$linuxOwnerFile;Sha=$linuxOwnerSha}
    )
    foreach($ownedInput in $sources){$null=Hold-ReadPin $ownedInput.Path $ownedInput.Sha}
    $linux=Get-Content -LiteralPath $linuxOwnerFile -Raw|ConvertFrom-Json
    if($linux.status -cne 'current-managed-linux7337-owner-verified' -or $linux.targetHost -cne 'Linux' -or $linux.port -ne 7337 -or $linux.currentListeningSocketOwned -ne $true -or $linux.managedUnitActive -ne $true){throw 'root-Linux-owner-contract-differs'}
    if($linux.listenerPid -isnot [int] -and $linux.listenerPid -isnot [long]){throw 'root-Linux-listener-pid-must-be-JSON-number'}
    if($linux.listenerStartTicks -isnot [int] -and $linux.listenerStartTicks -isnot [long]){throw 'root-Linux-startticks-must-be-JSON-number'}
    $phase='independent-current-manager-and-native-channel-refresh'
    $global:LASTEXITCODE=$null
    & '<USERPROFILE>\AppData\Roaming\uv\python\cpython-3.14.9-windows-x86_64-none\python.exe' -I (Join-Path $taskRoot 'refresh-current-identity.py')
    if($global:LASTEXITCODE -ne 0){throw 'current-independent-native-identity-refresh-failed'}
    $managerIdentity=Join-Path $taskRoot 'identity-current-safe.json'
    $phase='same-job-current-manager-identity-copy'
    $reader=[IO.File]::Open($managerIdentity,[IO.FileMode]::Open,[IO.FileAccess]::Read,([IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete))
    try {
        $memory=[IO.MemoryStream]::new()
        try {$reader.CopyTo($memory);$identityBytes=$memory.ToArray()}finally{$memory.Dispose()}
    }finally{$reader.Dispose()}
    $identity=[Text.UTF8Encoding]::new($false).GetString($identityBytes)|ConvertFrom-Json
    if($identity.status -cne 'current-owned-windows-medium-target-verified' -or $identity.bindingSha256 -cne $managerBindingSha -or $identity.namespaceId -cne $namespace){throw 'current-owned-manager-identity-differs'}
    if($identity.worker.pid -ne 5340 -or $identity.worker.createdExact -cne '134392926334771025' -or $identity.worker.sessionId -ne 1){throw 'current-owned-worker-binding-differs'}
    $identityFile=Join-Path $taskRoot ('identity-self-'+$attempt+'.json')
    $identitySha=Write-NewHeldBytes $identityFile $identityBytes
    $readBinding=[ordered]@{
        schemaVersion=1;purpose='windows-original-five-readonly-app-binding';sourceRevision='71def643cb08b208cc79687f79255c504eb14dc6'
        namespaceId=$namespace;identityFile=$identityFile;identitySha256=$identitySha;managerBindingSha256=$managerBindingSha
        worker=[ordered]@{pid=5340;createdExact='134392926334771025';sha256='60c9b29843624dd8af6b6a7b26147753be6aeef65098b9a503cc50850afe410f';sessionId=1}
        baselineEncryptedXml='<USERPROFILE>\AppData\Local\Temp\Orchester-final20-android-20261111\credentials-before-private-stdout.log'
    }
    $readBindingFile=Join-Path $taskRoot ('read-self-'+$attempt+'.json')
    $readBindingSha=Write-NewHeldJson $readBindingFile $readBinding
    $bootFile=Join-Path $taskRoot ('boot-self-'+$attempt+'-safe.json')
    $appFile=Join-Path $taskRoot ('app-self-'+$attempt+'-safe.json')
    $phase='same-job-native-boot-readiness'
    $global:LASTEXITCODE=$null
    & '<USERPROFILE>\AppData\Roaming\uv\python\cpython-3.14.9-windows-x86_64-none\python.exe' -I (Join-Path $taskRoot 'src\wait_owned_android_boot.py') --bindings $readBindingFile --bindings-sha256 $readBindingSha --output $bootFile
    if($global:LASTEXITCODE -isnot [int] -or $global:LASTEXITCODE -ne 0){throw 'native-boot-readiness-not-passed'}
    $bootSha=Read-LockedOutput $bootFile
    $phase='same-job-native-installed-app-UID-and-original-five-read'
    $global:LASTEXITCODE=$null
    & '<USERPROFILE>\AppData\Roaming\uv\python\cpython-3.14.9-windows-x86_64-none\python.exe' -I (Join-Path $taskRoot 'src\read_installed_app_binding.py') --bindings $readBindingFile --bindings-sha256 $readBindingSha --output $appFile
    if($global:LASTEXITCODE -isnot [int] -or $global:LASTEXITCODE -ne 0){throw 'native-current-app-binding-not-passed'}
    $appSha=Read-LockedOutput $appFile
    $app=Get-Content -LiteralPath $appFile -Raw|ConvertFrom-Json
    if($app.status -cne 'current-owned-Windows-installed-app-binding-readonly-passed' -or $app.identityReceiptSha256 -cne $identitySha -or $app.originalFiveEncryptedKeyValuesExactlyRetained -ne $true){throw 'native-app-binding-contract-differs'}
    if($app.actualRunAsAppUid -isnot [int] -and $app.actualRunAsAppUid -isnot [long]){throw 'native-app-UID-must-be-JSON-number'}
    $wireBinding=[ordered]@{
        schemaVersion=1;purpose='windows-original-five-native-current-wire';sourceRevision='71def643cb08b208cc79687f79255c504eb14dc6'
        namespaceId=$namespace;identityFile=$identityFile;identitySha256=$identitySha;managerBindingSha256=$managerBindingSha
        worker=$readBinding.worker;baselineEncryptedXml=$readBinding.baselineEncryptedXml
        installedAppBindingFile=$appFile;installedAppBindingSha256=$appSha;expectedActualRunAsAppUid=$app.actualRunAsAppUid
        baselineMetadataJson='<USERPROFILE>\AppData\Local\Temp\Orchester-final20-android-20261111\baseline-private.json';baselineMetadataSha256=$metadataSha
        currentManagedLinux7337File=$linuxOwnerFile;currentManagedLinux7337Sha256=$linuxOwnerSha
        aliasDexFile=(Join-Path $taskRoot 'src\dex\classes.dex');aliasDexSha256='df15578ba781311a7a00377af05c387731c663906ec4174d7a8f46ee9a5ebd31'
        expressionSha256='14bc742b9de3ed8e4b8f5434aeb2a0bda2db600a822a9c53f7662dcc955b9ae3'
        outputFile=('<USERPROFILE>\AppData\Local\Orchester-build\android5-current-wire-runtime-20261113\runtime-'+$namespace+'\native-five-wire-'+$attempt+'-safe.json')
    }
    $wireFile=Join-Path $taskRoot ('wire-self-'+$attempt+'.json')
    $wireSha=Write-NewHeldJson $wireFile $wireBinding
    $null=Write-NewHeldJson (Join-Path $taskRoot ('self-binding-'+$attempt+'-safe.json')) ([ordered]@{
        schemaVersion=1;status='same-job-native-boot-app-read-and-wire-binding-materialized';observedAtUtc=[DateTime]::UtcNow.ToString('o')
        namespaceId=$namespace;identitySha256=$identitySha;managerBindingSha256=$managerBindingSha;actualWindowsRunAsAppUid=$app.actualRunAsAppUid
        actualNativeBootExitCode=0;bootProofSha256=$bootSha;actualNativeAppBindingExitCode=0;appProofSha256=$appSha
        readBindingSha256=$readBindingSha;wireBindingFile=$wireFile;wireBindingSha256=$wireSha;wireSafeOutputFile=$wireBinding.outputFile
        sourceInputsAndRuntimeProofsReadLocked=$true;rawPrivateValuesIncluded=$false;linuxOwnerProofIndependentlyReverifiedByWindows=$false
    })
    $phase='same-job-native-original-five-and-one-original7337-info'
    $global:LASTEXITCODE=$null
    & '<USERPROFILE>\AppData\Roaming\uv\python\cpython-3.14.9-windows-x86_64-none\python.exe' -I (Join-Path $taskRoot 'src\native_five_current_wire.py') --bindings $wireFile --bindings-sha256 $wireSha
    if($global:LASTEXITCODE -isnot [int]){throw 'native-original-five-wire-exit-unavailable'}
    exit $global:LASTEXITCODE
}catch {
    [Console]::Error.WriteLine(('Self-refresh original-five wrapper failed at '+$phase+'; raw errors/private values omitted.'))
    exit 1
}finally {
    foreach($stream in $heldFiles){$stream.Dispose()}
}
