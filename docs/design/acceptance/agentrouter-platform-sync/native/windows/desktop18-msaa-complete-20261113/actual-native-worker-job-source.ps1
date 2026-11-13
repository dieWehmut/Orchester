# Prepared only: invokes the reviewed Desktop18 script at its actual directory.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0
$stage = 'fresh-runtime-preflight'
try {
    $entryPoint = 'C:\Users\han\AppData\Local\Orchester-build\continue-20261111-monitors-final21-71def64\p18m-1113-bf492b45\launch-smoke.ps1'
    $manifestFile = 'C:\Users\han\AppData\Local\Orchester-build\continue-20261111-monitors-final21-71def64\p18m-1113-bf492b45\prepared-scripts.json'
    if ((Get-FileHash -LiteralPath $entryPoint -Algorithm SHA256).Hash.ToLowerInvariant() -cne '2650988b451a2f4135cc4f8f3bc2247a906967efdd8904a6a70324217994d5d2') { throw 'fresh-entrypoint-pin-differs' }
    if ((Get-FileHash -LiteralPath $manifestFile -Algorithm SHA256).Hash.ToLowerInvariant() -cne '6725e731344348beae0d0106596bdd6d6809752e8f9f6c1acecf898d8d678705') { throw 'fresh-manifest-pin-differs' }
    $commonFile = 'C:\Users\han\AppData\Local\Orchester-build\continue-20261111-monitors-final21-71def64\monitor-final-common.ps1'
    if ((Get-FileHash -LiteralPath $commonFile -Algorithm SHA256).Hash.ToLowerInvariant() -cne '2d4e07f3d703043ea9af8fbf6377814f08135253837b4e36b8f075ea3aeb2f4f') { throw 'fresh-parent-common-pin-differs' }
    $parameters = @{
        PlanFile = 'C:\Users\han\AppData\Local\Orchester-build\monitor-final-20261110\monitors-final21-71def64-20261111\plan.json'
        ExpectedPlanSha256 = '8645dca6d20f2d5ff0d845d7c74522f52ca72c6aec49ec9529dcb86a997911cd'
        ExpectedFinalResultSha256 = '7ddcd67633e127a4b664dc69d91cf20bd8751a9ab6a009d8ca73d9b0951fe023'
        ExpectedPreparedScriptsSha256 = '6725e731344348beae0d0106596bdd6d6809752e8f9f6c1acecf898d8d678705'
        RuntimeInputsFile = 'C:\Users\han\AppData\Local\Orchester-build\monitor-final-20261110\monitors-final21-71def64-20261111\fresh-runtime-inputs.json'
        ExpectedRuntimeInputsSha256 = 'b364598c896cbcd8f4c7a70129a3ecbb6088f8930a0ea591a3860aaba6c7f86f'
        ResultDirectory = 'C:\Users\han\AppData\Local\Orchester-build\monitor-final-20261110\monitors-final21-71def64-20261111\d18m-1113-bf492b45'
        AckTimeoutSeconds = 900
        RuntimeTimeoutSeconds = 2400
    }
    if (Test-Path -LiteralPath $parameters.ResultDirectory) { throw 'fresh-result-directory-already-exists' }
    $stage = 'full-desktop18-runtime'
    $global:LASTEXITCODE = $null
    & $entryPoint @parameters
    if ($LASTEXITCODE -isnot [int]) { throw 'desktop18-script-exit-code-unavailable' }
    exit $LASTEXITCODE
} catch {
    [Console]::Error.WriteLine((ConvertTo-Json -InputObject ([ordered]@{
        state = 'failed'; stage = $stage; exceptionType = $_.Exception.GetType().FullName
    }) -Compress))
    exit 1
}
