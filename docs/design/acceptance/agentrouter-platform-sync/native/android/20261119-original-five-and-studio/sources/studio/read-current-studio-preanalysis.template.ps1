# Public review template; native execution is disabled. Original source digest is in provenance-safe.json.
throw 'public-review-template-native-execution-disabled'
$ErrorActionPreference='Stop'
$s=@(Get-Process studio64 -ErrorAction SilentlyContinue)
$rows=@($s|ForEach-Object{[ordered]@{pid=$_.Id;createdExact=$_.StartTime.ToUniversalTime().ToFileTimeUtc().ToString();path=$_.Path;sha256=(Get-FileHash -LiteralPath $_.Path -Algorithm SHA256).Hash.ToLowerInvariant();responsive=$_.Responding;hasWindow=($_.MainWindowHandle -ne 0)}})
$result=[ordered]@{schemaVersion=1;status='current-android-studio-processes-readonly-observed';observedAtUtc=[DateTime]::UtcNow.ToString('o');studioProcessCount=$rows.Count;rows=$rows;processSignals=0;appOrAdbOperations=0;windowTitlesOrUserValuesIncluded=$false}
[IO.File]::WriteAllText('<USERPROFILE>\AppData\Local\Orchester-build\android-resume-original5-20261119\android-studio-current-safe.json',($result|ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
$result|ConvertTo-Json -Depth 6
