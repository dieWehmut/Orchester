$ErrorActionPreference='Stop'
$path='C:\Users\han\AppData\Local\Orchester-build\continue-20261111-monitors-final21-71def64\prm-1113-8798425b\02a-current-promotion-and-cim-readonly-diagnostic.ps1'
$hold=[IO.File]::Open($path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
try {
 if((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()-cne '4485b26d34fabf46ef16b0c2eb03a97d938e2eb32ed55326c1ae96bdc3221f55'){throw 'diagnostic-source-pin-differs'}
 $t=$null;$e=$null;[void][Management.Automation.Language.Parser]::ParseFile($path,[ref]$t,[ref]$e)
 if(@($e).Count-ne 0){throw 'native-ast-invalid'}
 & $path
 $code=$LASTEXITCODE
 if($null-eq $code){throw 'native-exit-unobserved'}
}catch{Write-Output ('wrapper-error-type='+$_.Exception.GetType().FullName);$code=1}finally{$hold.Dispose()}
exit $code
