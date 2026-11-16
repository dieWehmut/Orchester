[CmdletBinding()]
param(
 [Parameter(Mandatory=$true)][ValidateSet('Desktop','Remote')][string]$Kind,
 [Parameter(Mandatory=$true)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$ExpectedFreshBackupSafeSha256
)
$ErrorActionPreference='Stop';Set-StrictMode -Version 2.0
$prepRoot=$PSScriptRoot;$base=Split-Path -Parent $prepRoot
$run='C:\Users\han\AppData\Local\Orchester-build\monitor-final-20261110\monitors-final21-71def64-20261111'
$locked=New-Object 'Collections.Generic.List[IO.FileStream]';$stage='private-backup-binding';$code=1
$result=[ordered]@{schemaVersion=1;kind=$Kind;status='failed';sourceRevision='71def643cb08b208cc79687f79255c504eb14dc6';installationMethod='same-volume-staged-directory-promotion';nativeInstallerExecuted=$false;processSignals=0;defaultLaunches=0;modelCalls=0;privateStateValuesIncluded=$false}
function Write-NewJson([string]$Path,$Value){$bytes=([Text.UTF8Encoding]::new($false)).GetBytes((ConvertTo-Json -InputObject $Value -Depth 12)+"`n");$s=[IO.File]::Open($Path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None);try{$s.Write($bytes,0,$bytes.Length);$s.Flush($true)}finally{$s.Dispose()}}
function Plain-Files([string]$Path){
 if(([IO.File]::GetAttributes($Path)-band [IO.FileAttributes]::ReparsePoint)-ne 0){throw 'original-root-reparse-point'}
 $files=@();$pending=New-Object 'Collections.Generic.Stack[string]';$pending.Push($Path)
 while($pending.Count -gt 0){foreach($item in Get-ChildItem -LiteralPath $pending.Pop() -Force){if(($item.Attributes -band [IO.FileAttributes]::ReparsePoint)-ne 0){throw 'original-reparse-point'};if($item.PSIsContainer){$pending.Push($item.FullName)}else{$files+=,$item.FullName}}}
 @($files|Sort-Object -CaseSensitive)
}
function Assert-OffLine{
 if(@(Get-NetTCPConnection -State Listen -ErrorAction Stop|Where-Object {$_.LocalPort -eq 7338}).Count-ne 0){throw '7338-listener-present-not-stopped'}
 if(@(Get-CimInstance Win32_Process -Filter "Name='orchester-desktop.exe' OR Name='orchester.exe'" -ErrorAction Stop).Count-ne 0){throw 'orchester-process-present-not-stopped'}
}
try{
 if((Get-FileHash -LiteralPath (Join-Path $base 'monitor-final-common.ps1') -Algorithm SHA256).Hash.ToLowerInvariant()-cne '2d4e07f3d703043ea9af8fbf6377814f08135253837b4e36b8f075ea3aeb2f4f'){throw 'common-source-pin-differs'}
 . (Join-Path $base 'monitor-final-common.ps1')
 $safePath=Join-Path $prepRoot 'fresh-offline-backup-safe.json'
 if((File-Hash $safePath)-cne $ExpectedFreshBackupSafeSha256){throw 'fresh-backup-safe-pin-differs'}
 $safe=Get-Content -LiteralPath $safePath -Raw|ConvertFrom-Json
 foreach($field in @('allExistingRootsVerified','allOwnedReadLocksReleased','privateAclInheritanceDisabled')){Require-TrueBool $safe.$field ('backup.'+$field)}
 Require-ExactInt $safe.actualNativeExitCode 0 'backup.native-exit'
 if($safe.status-cne 'fresh-offline-original-backup-verified' -or $safe.sourceRevision-cne $result.sourceRevision){throw 'fresh-offline-backup-not-verified'}
 $privatePath=Join-Path $prepRoot 'fresh-offline-backup-private.json'
 if((File-Hash $privatePath)-cne $safe.privateManifestSha256){throw 'private-backup-manifest-pin-differs'}
 $manifest=Get-Content -LiteralPath $privatePath -Raw|ConvertFrom-Json
 if($manifest.status-cne $safe.status -or $manifest.sourceRevision-cne $safe.sourceRevision -or @($manifest.roots).Count-ne 6){throw 'private-backup-manifest-invalid'}
 $stage='source-pins'
 $promote=Join-Path $base 'monitor-final-promote-cim60-prm-1113-8798425b.ps1'
 if((File-Hash (Join-Path $base 'monitor-final-promote.ps1'))-cne '01d96b35675d3a05acc800f44aed2dd29fdc12718b9f5db9303a7e0876ff53dc'){throw 'original-promotion-source-pin-differs'}
 if((File-Hash $promote)-cne '7db0b54b8abb8809720ec97228bd95a5abacacae52b07d87dc2ffef23b02ce41'){throw 'reviewed-cim60-successor-source-pin-differs'}
 $stage='original-offline';Assert-OffLine
 foreach($r in @($manifest.roots)){
  if($r.exists){Same-Inventory $r.inventory (Directory-Inventory $r.backup) 'preserved-fresh-offline-copy'}elseif(Test-Path -LiteralPath $r.source){throw 'original-absent-root-appeared'}
 }
 $targetName=if($Kind-ceq 'Desktop'){'desktop-installation'}else{'remote-installation'}
 $target=@($manifest.roots|Where-Object {$_.name-ceq $targetName});if($target.Count-ne 1 -or !$target[0].exists){throw 'original-installation-missing'}
 Same-Inventory $target[0].inventory (Directory-Inventory $target[0].source) 'original-target-installation-before-promotion'
 $stage='external-state-read-locks'
 foreach($r in @($manifest.roots|Where-Object {$_.name-notin @('desktop-installation','remote-installation')})){
  if($r.exists){foreach($file in @(Plain-Files $r.source)){$locked.Add([IO.File]::Open($file,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read))};Same-Inventory $r.inventory (Directory-Inventory $r.source) 'original-external-state-before-promotion'}
 }
 $stage='candidate-binary-digests-and-native-signature-observation'
 $candidate=if($Kind-ceq 'Desktop'){Join-Path $run 'desktop-candidate\orchester-desktop.exe'}else{Join-Path $run 'remote-package-fresh21\orchester.exe'}
 $expectedCandidate=if($Kind-ceq 'Desktop'){'e4aa765befa85c7fb053075844be5693685c00677dd62b4cfff4592ec68c1d05'}else{'24c8f9806256cc54b239b03f6b4adc976c3fee50c30bc0f2ecf601971c22ce2a'}
 if((File-Hash $candidate)-cne $expectedCandidate){throw 'candidate-full-exe-digest-differs'}
 $sig=Get-AuthenticodeSignature -LiteralPath $candidate -ErrorAction Stop
 $result.candidateExeSha256=$expectedCandidate;$result.candidateBytes=(Get-Item -LiteralPath $candidate).Length;$result.observedNativeAuthenticodeStatus=$sig.Status.ToString();$result.signatureIsVendorInstallerProof=$false
 # These are our source-built application artifacts; NotSigned is observed literally.
 if($sig.Status.ToString()-notin @('NotSigned','Valid')){throw 'candidate-authenticode-unexpected-status'}
 $runtime=if($Kind-ceq 'Desktop'){Join-Path $run 'd18m-1113-bf492b45\desktop-runtime-result.json'}else{Join-Path $run 'portable-runtime-native-20261112T1540-root\runtime-result.json'}
 $runtimeSha=if($Kind-ceq 'Desktop'){'1e7d973533d2dafaacac2146c6569380f85ab3fdb52373de47db09f49764cade'}else{'39049d9c38b1591f98da0505eaf845d1cface715edd7796a4e5461f4162bc573'}
 $cleanupSha=if($Kind-ceq 'Desktop'){'9c0d94d1e80aaab986c1d7edd92d3076faeef734514b72a5949bfcdf98b02516'}else{'3dd820b7d68c2a9213b549c35deccee91d4ee3aed0423a44db17b2c0751226fb'}
 $oldExeSha=if($Kind-ceq 'Desktop'){'9688bae1a669a4c38004af88ae78ac1397d59abac1f8e82cddb6ff888ba5e3e5'}else{'ef633cc43b2e767f222fe861fd2dfeca718fc677c0761d03f4c20f08d6a7bd66'}
 $sibling=(Split-Path -Parent $target[0].source)+'\'+(Split-Path -Leaf $target[0].source)+'-before-'+(Split-Path -Leaf $prepRoot)
 if(Test-Path -LiteralPath $sibling){throw 'installation-sibling-backup-already-exists'}
 $stage='authorized-staged-promotion'
 $promotionArguments=@{Kind=$Kind;PlanFile=(Join-Path $run 'plan.json');ExpectedPlanSha256='8645dca6d20f2d5ff0d845d7c74522f52ca72c6aec49ec9529dcb86a997911cd';ExpectedInstalledExeSha256=$oldExeSha;ExpectedFinalResultSha256='7ddcd67633e127a4b664dc69d91cf20bd8751a9ab6a009d8ca73d9b0951fe023';RuntimeResultFile=$runtime;ExpectedRuntimeResultSha256=$runtimeSha;ExpectedCleanupResultSha256=$cleanupSha;ExpectedRuntimeInputsGuardSha256='fa6ce76b0ac9d2c4a37b173931e4627d92cab7e5308e6f912b8467a5eb58ff6d';RuntimeInputsFile=(Join-Path $run 'fresh-runtime-inputs.json');ExpectedRuntimeInputsSha256='b364598c896cbcd8f4c7a70129a3ecbb6088f8930a0ea591a3860aaba6c7f86f';BackupDirectory=$sibling}
  $result.promotionAttempted=$true
 & $promote @promotionArguments
 $result.promotionReturnedSuccessfully=$true;$result.promoted=$true
 $stage='external-state-retention-after-promotion'
 foreach($r in @($manifest.roots|Where-Object {$_.name-notin @('desktop-installation','remote-installation')})){if($r.exists){Same-Inventory $r.inventory (Directory-Inventory $r.source) 'original-external-state-after-promotion'}elseif(Test-Path -LiteralPath $r.source){throw 'original-absent-root-appeared-after-promotion'}}
 Same-Inventory $target[0].inventory (Directory-Inventory $sibling) 'original-installation-sibling-backup-exact'
 Assert-OffLine
 $result.status='authorized-directory-promotion-and-offline-retention-verified';$result.promoted=$true;$result.originalInstallationPreserved=$true;$result.originalExternalStateByteExact=$true;$result.noAutoLaunch=$true;$result.current7338ListenerCount=0;$result.freshBackupSafeSha256=$ExpectedFreshBackupSafeSha256;$result.oldExeSha256=$oldExeSha;$result.promotedExeSha256=$expectedCandidate;$result.runtimePostPromotionAcceptance='pending-root';$code=0
}catch{
 $record=$_;$error=$record.Exception;$result.stage=$stage;$result.exceptionType=$error.GetType().FullName;$result.hResult=[int]$error.HResult
 $c=$error;while($c -and $c.GetType().FullName -cne 'Microsoft.Management.Infrastructure.CimException'){$c=$c.InnerException}
 if($c){$result.cimStatusName=$c.StatusCode.ToString();$result.cimStatusNumeric=[int]$c.StatusCode}
 $privateError=[ordered]@{schemaVersion=1;stage=$stage;exceptionType=$error.GetType().FullName;exceptionText=$error.ToString();scriptStackTrace=$record.ScriptStackTrace;errorRecordText=$record.ToString();hResult=$result.hResult}
 Write-NewJson (Join-Path $prepRoot ('promotion-'+$Kind.ToLowerInvariant()+'-'+[Guid]::NewGuid().ToString('N')+'-exception-private.json')) $privateError
}
finally{foreach($s in $locked){$s.Dispose()}}
$result.allOwnedExternalReadLocksReleased=$true;$result.actualNativeExitCode=$code;$result.observedAtUtc=[DateTime]::UtcNow.ToString('o')
Write-NewJson (Join-Path $prepRoot ('promotion-'+$Kind.ToLowerInvariant()+'-'+[Guid]::NewGuid().ToString('N')+'-safe.json')) $result
Write-Output (ConvertTo-Json -InputObject $result -Depth 10 -Compress)
exit $code
