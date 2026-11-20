$ErrorActionPreference='Stop';Set-StrictMode -Version 2.0
$root='C:\Users\han\AppData\Local\Orchester-build\default-finish-0e6df6887414';$prep='C:\Users\han\AppData\Local\Orchester-build\continue-20261111-monitors-final21-71def64\prm-1113-8798425b'
foreach($name in @('05-successor-default-runtime.ps1','06-successor-managed-7338.ps1')){$tokens=$null;$errors=$null;[void][Management.Automation.Language.Parser]::ParseFile((Join-Path $root $name),[ref]$tokens,[ref]$errors);if(@($errors).Count -ne 0){throw 'native-ast-errors'}}
$sessionRaw=& (Join-Path $prep 'read-current-default-session-and-input-desktop-1116.ps1');$sessionCode=$global:LASTEXITCODE;$session=$sessionRaw|ConvertFrom-Json
if($sessionCode -ne 0 -or $session.interactiveDefault -ne $true -or $session.currentSessionId -ne 1){throw 'current-native-desktop-not-ready'}
$node='C:\Users\han\AppData\Local\dsh-shortcut\node\node-v24.21.0-win-x64\node.exe'
foreach($name in @('04-successor-native-ui.mjs','03-successor-original-retention.mjs')){& $node '--check' (Join-Path $root $name);if($LASTEXITCODE -ne 0){throw 'native-node-syntax-error'}}
$r=[ordered]@{schemaVersion=1;status='native-ast-node-syntax-and-current-interactive-default-verified';observedAtUtc=[DateTime]::UtcNow.ToString('o');actualNativeExitCode=0;currentSessionId=$session.currentSessionId;interactiveDefault=$session.interactiveDefault;processSignals=0;privateValuesIncluded=$false}
$text=$r|ConvertTo-Json -Depth 4;[IO.File]::WriteAllText((Join-Path $root 'native-successor-preflight-safe.json'),$text,[Text.UTF8Encoding]::new($false));$text
