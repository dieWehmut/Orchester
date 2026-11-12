$ErrorActionPreference='Stop'
Set-StrictMode -Version 2.0
$stage='load-fixed-helpers'
$observation=$null
$nativeExit=1
try {
 $task='C:\Users\han\AppData\Local\Orchester-build\continue-20261111-monitors-final21-71def64'
 . (Join-Path $task 'monitor-final-common.ps1')
 $guard=Join-Path $task 'prepared-runtime\runtime-input-guards.ps1'
 if((File-Hash $guard) -cne 'fa6ce76b0ac9d2c4a37b173931e4627d92cab7e5308e6f912b8467a5eb58ff6d'){throw 'fixed-guard-pin-differs'}
 . $guard
 $run='C:\Users\han\AppData\Local\Orchester-build\monitor-final-20261110\monitors-final21-71def64-20261111'
 $planSha='8645dca6d20f2d5ff0d845d7c74522f52ca72c6aec49ec9529dcb86a997911cd'
 $finalSha='7ddcd67633e127a4b664dc69d91cf20bd8751a9ab6a009d8ca73d9b0951fe023'
 $stage='read-immutable-plan';$plan=Read-Plan (Join-Path $run 'plan.json') $planSha
 $stage='read-immutable-build';$final=Join-Path $run 'final-result.json'
 if((File-Hash $final) -cne $finalSha){throw 'fixed-build-pin-differs'}
 $build=Get-Content -LiteralPath $final -Raw|ConvertFrom-Json
 $candidate=Join-Path $run 'fresh-runtime-inputs-readonly-diagnostic-c3c39a78.json'
 $candidateSha='6f18b0cf72de54bdb0a60d68c0e533b20049f11e17bf4fa220b2ab516cda5b63'
 $stage='read-successor-readonly-diagnostic-candidate-through-unmodified-guard'
 $verified=Read-FreshRuntimeInputs $plan $build $planSha $finalSha $candidate $candidateSha
 if((File-Hash $candidate) -cne $candidateSha){throw 'successor-readonly-diagnostic-candidate-changed-during-readonly-verification'}
 $observation=[ordered]@{status='successor-readonly-diagnostic-candidate-now-accepted-by-unmodified-guard';stage=$stage;readOnly=$true;candidateSha256=$candidateSha;guardSha256=(File-Hash $guard);runtimeInputsFrozen=[bool](Test-Path -LiteralPath (Join-Path $run 'fresh-runtime-inputs.json'));originalFilesModified=$false;freezeInvoked=$false;runtimeInvoked=$false}
 $nativeExit=0
} catch {
 $exception=$_.Exception.GetBaseException()
 $message=[string]$exception.Message
 $knownStaticMessage=$null
 if($message -cin @('Illegal characters in path.','The path is not of a legal form.','Empty path name is not legal.','Runtime inputs source/build binding differs.','Android source/runtime receipt differs.','Original Android native acceptance is incomplete.','Original Android canonical UI evidence is incomplete.','Original Android actual canonical provider identities differ.','fixed-guard-pin-differs','fixed-build-pin-differs','successor-readonly-diagnostic-candidate-changed-during-readonly-verification')){$knownStaticMessage=$message}
 elseif($message -cmatch '\AInventory changed: (frozen native mobile Web|normalized Android mobile Web|original Android resource inventory|derived fresh portable package|derived producer mobile Web)\z'){$knownStaticMessage=$message}
 elseif($message -cmatch '\ARequired (true boolean|integer zero|exact integer): [A-Za-z0-9. ]+\z'){$knownStaticMessage=$message}
 $frames=@([string]$_.ScriptStackTrace -split "`r?`n" | ForEach-Object {if($_ -match 'at ([A-Za-z0-9_-]+), (.+): line (\d+)'){[pscustomobject]@{function=$matches[1];sourceFileName=[IO.Path]::GetFileName($matches[2]);line=[int]$matches[3]}}})
 $observation=[ordered]@{status='successor-readonly-diagnostic-candidate-rejected-by-unmodified-guard';stage=$stage;readOnly=$true;exceptionType=$exception.GetType().FullName;hResult=[int]$exception.HResult;knownStaticErrorLabel=$knownStaticMessage;errorFullyQualifiedId=[string]$_.FullyQualifiedErrorId;scriptFrames=$frames;originalFilesModified=$false;freezeInvoked=$false;runtimeInvoked=$false;privateContentsOrRawMessageIncluded=$false}
 $nativeExit=0
}
$bytes=[Text.UTF8Encoding]::new($false).GetBytes(($observation|ConvertTo-Json -Depth 9)+"`n")
$output=Join-Path $PSScriptRoot 'readonly-guard-result-safe.json'
$stream=[IO.FileStream]::new($output,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
try{$stream.Write($bytes,0,$bytes.Length);$stream.Flush($true)}finally{$stream.Dispose()}
Write-Output 'New standalone diagnostic candidate read through the fixed guard; only known static diagnostics published.'
exit $nativeExit
