$ErrorActionPreference='Stop'
Set-StrictMode -Version 2.0
function Require-TrueBool($Value,[string]$Label) {
 if ($Value -isnot [bool] -or $Value -cne $true) { throw ('Required true boolean: '+$Label) }
}
function Require-ZeroInt($Value,[string]$Label) {
 if (($Value -isnot [int] -and $Value -isnot [long]) -or $Value -ne 0) { throw ('Required integer zero: '+$Label) }
}
function Require-PositiveInt($Value,[string]$Label) {
 if (($Value -isnot [int] -and $Value -isnot [long]) -or $Value -lt 1) { throw ('Required positive integer: '+$Label) }
}
function Require-CanonicalAndroidUi($Ui) {
 $expected=@('codex','claude','deepseek','opencode','gemini','antigravity','cursor','workbuddy','workbuddy_global','qwen','openclaw','cline','roo','kilo','hermes','pi','crush','claude_app','trae','zcode','craft')
 if($Ui.status -cne 'passed' -or $Ui.canonicalProviderIds -isnot [array]) { throw 'Original Android canonical UI evidence is incomplete.' }
 Require-ExactInt $Ui.canonicalProviderCount 21 'Android actual canonical provider count'
 Require-ExactInt $Ui.canonicalThemeImageCount 42 'Android actual canonical theme image count'
 Require-TrueBool $Ui.allCanonicalThemeImagesLoaded 'Android actual canonical images loaded'
 $ids=@($Ui.canonicalProviderIds)
 if($ids.Count -ne 21 -or @($ids|Where-Object {$_ -isnot [string]}).Count -ne 0 -or
    (@($ids|Sort-Object -CaseSensitive) -join ',') -cne (@($expected|Sort-Object -CaseSensitive) -join ',')) { throw 'Original Android actual canonical provider identities differ.' }
}
function Require-NativeProviderInstances([string]$Provider,$Roster,$Unit,[string]$ExpectedExternalUnit) {
 if($Roster.id -cne $Provider -or $Unit.id -cne $Provider -or $Unit.unit -cne $ExpectedExternalUnit -or
    $ExpectedExternalUnit -cnotin @('services','processes') -or $Unit.instances -isnot [array]) { throw 'Native provider instance identity or unit differs.' }
 Require-PositiveInt $Roster.instanceCount 'native roster instance count'
 Require-PositiveInt $Unit.instanceCount 'native unit instance count'
 Require-ExactInt $Unit.instanceCount ([long]$Roster.instanceCount) 'native matching instance counts'
 Require-ExactInt @($Unit.instances).Count ([long]$Unit.instanceCount) 'native actual instance length'
 $seen=New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
 foreach($instance in $Unit.instances) {
  if($instance.provider -isnot [string] -or $instance.provider -cne $Provider -or
     $instance.agentId -isnot [string] -or [string]::IsNullOrWhiteSpace($instance.agentId) -or
     -not $seen.Add($instance.agentId) -or $instance.externalSource -isnot [bool]) { throw 'Native actual DTO instance identity differs.' }
  $expectedUnit=if($instance.externalSource){$ExpectedExternalUnit}else{'windows'}
  if($instance.unit -cne $expectedUnit) { throw 'Native actual DTO instance count unit differs.' }
  foreach($field in @('actualDtoAndNativeUiAgree','loaded','darkAndLightSwitchObserved')) { Require-TrueBool $instance.$field ('native instance '+$field) }
  Require-ExactInt $instance.themeImagesLoaded 2 'native actual instance theme image count'
 }
}
function Read-FreshRuntimeInputs($Plan,$Build,[string]$PlanSha,[string]$FinalSha,[string]$InputsFile,[string]$InputsSha) {
 $file=Full-Path $InputsFile;Assert-Below $file $Plan.runDirectory
 if ((File-Hash $file) -cne $InputsSha) { throw 'Runtime inputs receipt SHA differs.' }
 $inputs=Get-Content -Raw -LiteralPath $file | ConvertFrom-Json
 if($inputs.format -cne 'orchester-fresh-android-runtime-inputs-v1' -or $inputs.status -cne 'verified' -or $inputs.sourceRevision -cne $Plan.sourceRevision -or $inputs.expectedPlanSha256 -cne $PlanSha -or $inputs.expectedFinalResultSha256 -cne $FinalSha -or $inputs.desktopExeSha256 -cne $Build.desktopExeSha256 -or $inputs.remoteExeSha256 -cne $Build.remoteExeSha256 -or $inputs.sourceExportAuditSha256 -cne $Plan.sourceExportAuditSha256) { throw 'Runtime inputs source/build binding differs.' }
 if($Plan.apkBinding -cne 'deferred-fresh-native-android-source-receipt' -or $null -ne $Plan.apkSha256 -or $null -ne $Build.apkSha256) { throw 'Producer must retain immutable deferred APK binding.' }
 if($inputs.apkSha256 -cnotmatch '\A[0-9a-f]{64}\z' -or ($inputs.apkBytes -isnot [int] -and $inputs.apkBytes -isnot [long]) -or $inputs.apkBytes -lt 1 -or (File-Hash $inputs.apkFile) -cne $inputs.apkSha256 -or (Get-Item -LiteralPath $inputs.apkFile).Length -ne $inputs.apkBytes) { throw 'Actual fresh APK bytes differ.' }
 if((File-Hash $inputs.androidAcceptanceReceiptFile) -cne $inputs.androidAcceptanceReceiptSha256) { throw 'Normalized Android receipt changed.' }
 $android=Get-Content -Raw -LiteralPath $inputs.androidAcceptanceReceiptFile | ConvertFrom-Json
 if($android.format -cne 'orchester-fresh-native-android-acceptance-v1' -or $android.status -cne 'passed' -or $android.sourceRevision -cne $Plan.sourceRevision -or $android.apkSha256 -cne $inputs.apkSha256) { throw 'Android source/runtime receipt differs.' }
 foreach($field in @('sourceAndDistVerified','originalInstalledCertificateMatched','nativeBridgeAccepted','nativeUiAccepted','nativeTerminalAccepted','cleanupAccepted')) { Require-TrueBool $android.$field ('android.'+$field) }
 Require-ZeroInt $android.nativeGradleExitCode 'Android native Gradle'
 Require-ZeroInt $android.nativeInstallExitCode 'Android native install'
 if((File-Hash $android.originalReceiptFile) -cne $android.originalReceiptSha256) { throw 'Original Android acceptance receipt changed.' }
 $original=Get-Content -Raw -LiteralPath $android.originalReceiptFile|ConvertFrom-Json
 if($original.status -cne 'passed' -or $original.sourceRevision -cne $Plan.sourceRevision -or $original.apkSha256 -cne $inputs.apkSha256 -or $original.certificateSha256 -cne $android.certificateSha256 -or $original.nativeBridge.status -cne 'passed' -or $original.ui.status -cne 'passed' -or $original.terminal.status -cne 'passed' -or $original.cleanup.status -cne 'passed') { throw 'Original Android native acceptance is incomplete.' }
 Require-TrueBool $original.resourcesDistMirrorAndApkByteExact 'Android original resources'
 Require-TrueBool $original.build.nativeWindows 'Android native Windows build'
 Require-ZeroInt $original.build.exitCode 'Android original build'
 Require-ZeroInt $original.install.appExitCode 'Android original installation'
 Require-ZeroInt $original.nativeBridge.commandActualExitCode 'Android original Bridge'
 Require-ExactInt $original.nativeBridge.tests 4 'Android original Bridge tests'
 Require-CanonicalAndroidUi $original.ui
 Same-Inventory $inputs.mobileDistInventory (Directory-Inventory (Join-Path $Plan.sourceRoot 'apps/mobile/dist')) 'frozen native mobile Web'
 Same-Inventory $android.mobileDistInventory $inputs.mobileDistInventory 'normalized Android mobile Web'
 Same-Inventory @($original.resourceInventory | ForEach-Object {[pscustomobject]@{path=$_.path;sha256=$_.sha256}}) $inputs.mobileDistInventory 'original Android resource inventory'
 $candidate=Full-Path $inputs.remotePackageCandidate;Assert-Below $candidate $Plan.runDirectory
 Same-Inventory $inputs.remotePackageInventory (Directory-Inventory $candidate) 'derived fresh portable package'
 if((File-Hash (Join-Path $candidate 'orchester.exe')) -cne $Build.remoteExeSha256 -or (File-Hash (Join-Path $candidate 'orchester-mobile-debug.apk')) -cne $inputs.apkSha256) { throw 'Derived package actual EXE or APK differs.' }
 Same-Inventory (Directory-Inventory (Join-Path $Plan.remoteCandidate 'web')) (Directory-Inventory (Join-Path $candidate 'web')) 'derived producer mobile Web'
 return $inputs
}
