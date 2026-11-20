$ErrorActionPreference='Stop';Set-StrictMode -Version 2.0
$out='C:\Users\han\AppData\Local\Orchester-build\default-finish-0e6df6887414';$helper='C:\Users\han\AppData\Local\Orchester-build\continue-20261111-monitors-final21-71def64\p18m-1113-bf492b45\OwnedRuntimeProcesses.cs'
if((Get-FileHash -LiteralPath $helper -Algorithm SHA256).Hash.ToLowerInvariant() -cne 'd335743a632acae0c90968e482b87eda44aaf1dae241e938e62b5c7833598d45'){throw 'source-pin'}
Add-Type -Path $helper
$paths=@('C:\Users\han\Downloads\OrchesterRemote\orchester.exe','C:\WINDOWS\System32\conhost.exe','C:\WINDOWS\System32\wsl.exe','C:\Program Files\WSL\wsl.exe','C:\Program Files\WSL\wslhost.exe')
$server=$null;$own=$null
try {
 $server=[Diagnostics.Process]::GetProcessById(9844);$h=$server.Handle
 if($server.StartTime.ToUniversalTime().ToFileTimeUtc().ToString() -cne '134395440680673328'){throw 'birth-pin'}
 $own=[Orchester.FinalRuntime.OwnedProcesses]::new($paths);$own.CaptureRoot($server,$paths[0],'24c8f9806256cc54b239b03f6b4adc976c3fee50c30bc0f2ecf601971c22ce2a');$own.RegisterPassiveWslHostObservation();$own.CaptureAllowedDescendants()
 $observed=@();foreach($p in $own.Identities){$sig=Get-AuthenticodeSignature -LiteralPath $p.Image;$observed+=@([ordered]@{pid=$p.Pid;parentPid=$p.ParentPid;depth=$p.Depth;createdFileTimeExact=$p.Created.ToString();image=$p.Image;imageSha256=$p.Sha256;signatureStatus=$sig.Status.ToString();signerIsMicrosoft=($sig.SignerCertificate -and $sig.SignerCertificate.Subject -ceq 'CN=Microsoft Corporation, O=Microsoft Corporation, L=Redmond, S=Washington, C=US');sameHeldHandleLive=$own.IsAlive($p)})}
 $static=@();foreach($path in $paths[1..($paths.Length-1)]){$sig=Get-AuthenticodeSignature -LiteralPath $path;$static+=@([ordered]@{image=$path;sha256=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant();signatureStatus=$sig.Status.ToString();signerIsMicrosoft=($sig.SignerCertificate -and $sig.SignerCertificate.Subject -ceq 'CN=Microsoft Corporation, O=Microsoft Corporation, L=Redmond, S=Washington, C=US')})}
 $r=[ordered]@{schemaVersion=1;status='current-successor-native-tree-images-and-signatures-observed';observedAtUtc=[DateTime]::UtcNow.ToString('o');originalCreatorHandleRecovered=$false;freshQueryHandlesHeldDuringSnapshot=$true;ownedObservedTree=$observed;unknownObservationEvents=$own.UnexpectedDescendants;safeUnexpectedImages=@($own.SafeUnexpectedImages);systemImageVerification=$static;processSignals=0;commandLinesRead=$false;privateDataValuesIncluded=$false}
 $json=$r|ConvertTo-Json -Depth 10;[IO.File]::WriteAllText((Join-Path $out 'successor-current-tree-signature-safe.json'),$json,[Text.UTF8Encoding]::new($false));$json
}finally{if($own){$own.Dispose()};if($server){$server.Dispose()}}
