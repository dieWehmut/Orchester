$ErrorActionPreference='Stop'
$self=[Diagnostics.Process]::GetCurrentProcess()
$handle=$self.Handle
try {
 if($handle -eq [IntPtr]::Zero){throw 'own-native-handle-unavailable'}
 $result=[ordered]@{schemaVersion=1;status='owned-native-worker-hello';actualTargetHost='Windows';observedAtUtc=[DateTime]::UtcNow.ToString('o');pid=$PID;createdFileTimeExact=$self.StartTime.ToUniversalTime().ToFileTimeUtc().ToString();sessionId=$self.SessionId;powerShellVersion=$PSVersionTable.PSVersion.ToString();workingDirectory=(Get-Location).ProviderPath;sourceSha256=(Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash.ToLowerInvariant();privateValuesIncluded=$false}
 [IO.File]::WriteAllText((Join-Path 'C:\Users\han\AppData\Local\Orchester-build\continuation-native-access-20261113\owned-native-worker-attempt4-1ed007a58623f12c8e1c1223\jobs\1b8b715fd7cb21c3c0f378d36193acba' 'hello-native-safe.json'),(ConvertTo-Json -InputObject $result -Depth 5),(New-Object Text.UTF8Encoding($false)))
 Write-Output (ConvertTo-Json -InputObject $result -Depth 5 -Compress)
} finally {$self.Dispose()}
exit 0
