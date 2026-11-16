$ErrorActionPreference='Stop'
Set-StrictMode -Version 2.0
$path='C:\Users\han\AppData\Local\Orchester-build\continue-20261111-monitors-final21-71def64\prm-1113-8798425b\02-promote-with-fresh-offline-retention.ps1'
$hold=[IO.File]::Open($path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
try {
 if((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()-cne 'f18fcf1fa73a3036decac23c355f8f3e4a64b2acb589e947576e854bd8727ae2'){throw 'promotion-source-pin-differs'}
 $t=$null;$e=$null;[void][Management.Automation.Language.Parser]::ParseFile($path,[ref]$t,[ref]$e)
 if(@($e).Count -ne 0){throw 'promotion-native-ast-invalid'}
 & $path -Kind 'Desktop' -ExpectedFreshBackupSafeSha256 'c2277a82ccf1df70c61ac18190fdc803801053f54d36dcbdcff06febd89a090d'
 $code=$LASTEXITCODE
 if($null-eq $code){throw 'promotion-exit-unobserved'}
} catch {Write-Output ('promotion-wrapper-error-type='+$_.Exception.GetType().FullName);$code=1} finally {$hold.Dispose()}
exit $code
