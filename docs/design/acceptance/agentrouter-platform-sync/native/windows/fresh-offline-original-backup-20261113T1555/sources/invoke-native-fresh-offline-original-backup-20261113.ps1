$ErrorActionPreference='Stop'
$h=$null
try {
 $path='C:\Users\han\AppData\Local\Orchester-build\continue-20261111-monitors-final21-71def64\prm-1113-8798425b\01-fresh-offline-backup.ps1'
 $h=[IO.File]::Open($path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
 if((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -cne 'a5ca1344c232c390408ab32b68a86b330e5b37c610c03791a608cfe03ffbd4c1'){throw 'backup-script-pin-differs'}
 $tokens=$null;$errors=$null
 [void][Management.Automation.Language.Parser]::ParseFile($path,[ref]$tokens,[ref]$errors)
 if(@($errors).Count -ne 0){throw 'backup-script-native-ast-errors'}
 $LASTEXITCODE=$null
 & $path
 if($LASTEXITCODE -isnot [int]){throw 'backup-script-native-exit-unavailable'}
 exit $LASTEXITCODE
}catch{[Console]::Error.WriteLine('Owned offline backup wrapper failed; raw/private error values omitted.');exit 1}
finally{if($h){$h.Dispose()}}
