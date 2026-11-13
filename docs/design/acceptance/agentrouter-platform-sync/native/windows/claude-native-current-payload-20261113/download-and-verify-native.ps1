# Windows PowerShell 5.1. Download and verify only; never execute the payload.
Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$outRoot = 'C:\Users\han\AppData\Local\Orchester-build\continuation-native-access-20261113\owned-native-worker-attempt3-3fa131682683515d5d18a4e5\jobs\23fd582bdce1abd4fa64435560a8d95d'
$sourceUrl = 'https://downloads.claude.ai/releases/win32/x64/3.6556.2/Claude-20c019a1e61fa5b687616af57d31f97bb3181723.exe'
$expectedSha256 = '945082290478E5C668E8AAA25A392E1CD129E6219173E907B7FF1F7F99DED45E'
$manifestUrl = 'https://raw.githubusercontent.com/microsoft/winget-pkgs/dde9f425ebb1721103491cd450e25ea7059ef98a/manifests/a/Anthropic/Claude/3.6556.2/Anthropic.Claude.installer.yaml'
$downloadSeconds = 120
$signatureSeconds = 45
$maximumBytes = [long]1073741824
$resultCode = 99
$resultPath = $null
$workRoot = $null
$partialPath = $null
$downloadedPath = $null
$finalPath = $null
$report = [ordered]@{
    schema_version = 1
    task = 'claude-official-payload-native-safe-download-verification'
    started_utc = [DateTime]::UtcNow.ToString('o')
    finished_utc = $null
    phase = 'initializing'
    exit_code = 99
    source = [ordered]@{ package = 'Anthropic.Claude'; version = '3.6556.2'; architecture = 'x64'; installer_type = 'exe'; url = $sourceUrl; manifest_url = $manifestUrl; expected_sha256 = $expectedSha256 }
    limits = [ordered]@{ download_seconds = $downloadSeconds; download_process_guard_seconds = 126; signature_process_seconds = $signatureSeconds; termination_grace_seconds = 5; output_drain_timeout_seconds = 5; maximum_accepted_payload_bytes = $maximumBytes }
    download = $null
    hash = $null
    signature = $null
    signature_process = $null
    artifact = $null
    error = $null
    installed = $false
    payload_executed = $false
    gui_started = $false
    tls_validation_disabled = $false
    global_proxy_or_dns_changed = $false
    verification_complete = $false
}

function Write-JsonFile([string]$Path, $Value) {
    $json = $Value | ConvertTo-Json -Depth 15
    [IO.File]::WriteAllText($Path, $json, (New-Object Text.UTF8Encoding($false)))
}

function Protect-Diagnostic([string]$Text) {
    if ($null -eq $Text) { return '' }
    $clean = [regex]::Replace($Text, '(?im)^\s*(Authorization|Proxy-Authorization|Cookie|Set-Cookie)\s*:.*$', '[sensitive header omitted]')
    $clean = [regex]::Replace($clean, '(?i)https?://[^\s<>]+', [Text.RegularExpressions.MatchEvaluator]{
        param($match)
        try {
            $uri = New-Object Uri($match.Value)
            $safe = $uri.Scheme + '://' + $uri.IdnHost
            if (-not $uri.IsDefaultPort) { $safe += ':' + $uri.Port }
            $safe += $uri.AbsolutePath
            if ($uri.Query.Length -gt 0 -or $uri.Fragment.Length -gt 0) { $safe += '[query/fragment omitted]' }
            return $safe
        } catch { return '[URL omitted]' }
    })
    if ($clean.Length -gt 16384) { $clean = $clean.Substring(0,16384) + '[truncated]' }
    return $clean
}

function Quote-NativeArgument([string]$Argument) {
    # All supplied paths end in a filename; reject unsupported quoting inputs.
    if ($Argument.Contains([string][char]34) -or $Argument.Contains([string][char]0) -or $Argument.Contains([string][char]10) -or $Argument.Contains([string][char]13) -or $Argument.EndsWith('\')) {
        throw 'Unsupported native argument quoting input.'
    }
    return ([string][char]34 + $Argument + [string][char]34)
}

function Invoke-BoundedProcess([string]$Executable, [string[]]$Arguments, [int]$TimeoutMilliseconds) {
    $info = New-Object Diagnostics.ProcessStartInfo
    $info.FileName = $Executable
    $info.Arguments = (($Arguments | ForEach-Object { Quote-NativeArgument $_ }) -join ' ')
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $process = New-Object Diagnostics.Process
    $process.StartInfo = $info
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $stdout = ''
    $stderr = ''
    $nativeCode = $null
    $timedOut = $false
    $nativeExited = $false
    try {
        if (-not $process.Start()) { throw 'Native process did not start.' }
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit($TimeoutMilliseconds)) {
            $timedOut = $true
            try { $process.Kill() } catch { }
            $null = $process.WaitForExit(5000)
        }
        $nativeExited = $process.HasExited
        if ($nativeExited) {
            $nativeCode = $process.ExitCode
            $null = [Threading.Tasks.Task]::WaitAll([Threading.Tasks.Task[]]@($stdoutTask,$stderrTask), 5000)
        }
        if ($stdoutTask.IsCompleted -and -not $stdoutTask.IsFaulted -and -not $stdoutTask.IsCanceled) { $stdout = $stdoutTask.Result }
        if ($stderrTask.IsCompleted -and -not $stderrTask.IsFaulted -and -not $stderrTask.IsCanceled) { $stderr = $stderrTask.Result }
        return [ordered]@{ executable = $Executable; timed_out = $timedOut; native_exited = $nativeExited; actual_exit_code = $nativeCode; elapsed_seconds = [Math]::Round($watch.Elapsed.TotalSeconds,3); stdout = (Protect-Diagnostic $stdout); stderr = (Protect-Diagnostic $stderr) }
    } finally { $watch.Stop(); $process.Dispose() }
}

function Get-SystemExecutable([string]$RelativeName) {
    $windowsRoot = [Environment]::GetFolderPath([Environment+SpecialFolder]::Windows)
    if ([Environment]::Is64BitOperatingSystem -and -not [Environment]::Is64BitProcess) { $systemRoot = Join-Path $windowsRoot 'Sysnative' }
    else { $systemRoot = Join-Path $windowsRoot 'System32' }
    return Join-Path $systemRoot $RelativeName
}

function As-PSLiteral([string]$Value) { return "'" + $Value.Replace("'", "''") + "'" }
function Encode-PSCommand([string]$Code) { return [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($Code)) }

try {
    if (-not [IO.Path]::IsPathRooted($outRoot)) { $resultCode = 10; throw 'Worker output placeholder must be replaced with an absolute Windows directory.' }
    $null = [IO.Directory]::CreateDirectory($outRoot)
    $resultPath = Join-Path $outRoot 'claude-official-payload-native-safe.json'
    $workRoot = Join-Path $outRoot ('claude-official-3.6556.2-download-' + [Guid]::NewGuid().ToString('N'))
    $null = [IO.Directory]::CreateDirectory($workRoot)
    $partialPath = Join-Path $workRoot 'Claude-3.6556.2-x64.exe.partial'
    $downloadedPath = Join-Path $workRoot 'Claude-3.6556.2-x64.downloaded.exe'
    $finalPath = Join-Path $workRoot 'Claude-3.6556.2-x64.verified.exe'
    $powershellPath = Get-SystemExecutable 'WindowsPowerShell\v1.0\powershell.exe'
    if (-not [IO.File]::Exists($powershellPath)) { $resultCode = 10; throw 'System Windows PowerShell is required for bounded signature verification.' }
    $report.phase = 'downloading'
    Write-JsonFile $resultPath $report
    $curlPath = Get-SystemExecutable 'curl.exe'
    if ([IO.File]::Exists($curlPath)) {
        $curlArgs = @('--disable','--fail','--show-error','--silent','--location','--max-redirs','5','--proto','=https','--proto-redir','=https','--tlsv1.2','--connect-timeout','15','--max-time',([string]$downloadSeconds),'--max-filesize',([string]$maximumBytes),'--output',$partialPath,'--write-out','http_code=%{http_code}\nsize_download=%{size_download}\nssl_verify_result=%{ssl_verify_result}\nnum_redirects=%{num_redirects}\n',$sourceUrl)
        $report.download = Invoke-BoundedProcess $curlPath $curlArgs 126000
        $report.download['method'] = 'system-curl-default-chain-and-hostname-validation'
        if ($report.download.timed_out) { $resultCode = 12; throw 'Download exceeded the outer process deadline; any bytes remain marked partial.' }
        if ($report.download.actual_exit_code -ne 0) { $resultCode = 13; throw 'System curl returned a nonzero exit code; download is not complete.' }
        if ($report.download.stdout -notmatch '(?m)^http_code=200\s*$') { $resultCode = 14; throw 'Complete download requires HTTP 200.' }
        if ($report.download.stdout -notmatch '(?m)^ssl_verify_result=0\s*$') { $resultCode = 14; throw 'Complete download requires successful TLS certificate verification.' }
    } else {
        # Clean, separate PS5.1 process. Default .NET validation is never replaced.
        $downloadStatusPath = Join-Path $workRoot 'dotnet-download-status.json'
        $downloadCode = @'
Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$downloadUrl = __DOWNLOAD_URL_LITERAL__
$partial = __DOWNLOAD_PARTIAL_LITERAL__
$statusFile = __DOWNLOAD_STATUS_LITERAL__
$limitSeconds = 120
$limitBytes = [long]1073741824
$code = 13
$response = $null; $source = $null; $destination = $null
$state = [ordered]@{ method = 'native-dotnet-default-chain-and-hostname-validation'; complete = $false; http_code = $null; bytes_written = [long]0; content_length = $null; redirects = 0; tls12_enabled_process_only = $true; revocation_check_enabled_process_only = $true; web_exception_status = $null; error = $null; exit_code = 13 }
$watch = [Diagnostics.Stopwatch]::StartNew()
try {
    if ($null -ne [Net.ServicePointManager]::ServerCertificateValidationCallback) { throw 'Unexpected global certificate callback; refusing fallback.' }
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    [Net.ServicePointManager]::CheckCertificateRevocationList = $true
    $uri = New-Object Uri($downloadUrl)
    while ($true) {
        if ($uri.Scheme -ne 'https') { throw 'A redirect attempted a non-HTTPS scheme.' }
        if ($watch.Elapsed.TotalSeconds -ge $limitSeconds) { $code = 12; throw 'Download deadline reached.' }
        $request = [Net.HttpWebRequest]::Create($uri)
        $request.Method = 'GET'
        $request.AllowAutoRedirect = $false
        $request.UseDefaultCredentials = $false
        $request.PreAuthenticate = $false
        $request.Credentials = $null
        $request.Timeout = [Math]::Max(1,[int](($limitSeconds - $watch.Elapsed.TotalSeconds)*1000))
        $request.ReadWriteTimeout = 15000
        $response = $request.GetResponse()
        $state.http_code = [int]$response.StatusCode
        if ($state.http_code -in @(301,302,303,307,308)) {
            if ($state.redirects -ge 5) { throw 'HTTPS redirect limit exceeded.' }
            $location = $response.Headers['Location']
            if ([string]::IsNullOrEmpty($location)) { throw 'Redirect has no location.' }
            $nextUri = New-Object Uri($uri,$location)
            $response.Close(); $response = $null
            $uri = $nextUri; $state.redirects++; continue
        }
        if ($state.http_code -ne 200) { throw 'Complete download requires HTTP 200.' }
        break
    }
    $state.content_length = $response.ContentLength
    if ($response.ContentLength -gt $limitBytes) { throw 'Declared payload exceeds the byte limit.' }
    $source = $response.GetResponseStream()
    $destination = New-Object IO.FileStream($partial,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
    $buffer = New-Object byte[] 65536
    while ($true) {
        if ($watch.Elapsed.TotalSeconds -ge $limitSeconds) { $code = 12; $request.Abort(); throw 'Download deadline reached.' }
        $count = $source.Read($buffer,0,$buffer.Length)
        if ($count -eq 0) { break }
        if (($state.bytes_written + $count) -gt $limitBytes) { throw 'Downloaded payload exceeds the byte limit.' }
        $destination.Write($buffer,0,$count); $state.bytes_written += $count
    }
    $destination.Flush($true)
    if ($state.bytes_written -le 0) { throw 'Payload is empty.' }
    if ($response.ContentLength -ge 0 -and $state.bytes_written -ne $response.ContentLength) { throw 'Content length does not match complete downloaded bytes.' }
    $state.complete = $true; $code = 0
} catch {
    # Do not archive response headers, proxy credentials, cookies, or signed redirect URLs.
    $state.error = ($_.Exception.GetType().FullName + ': download or TLS validation failed; raw URL-bearing exception omitted')
    $webException = $_.Exception -as [Net.WebException]
    if ($null -ne $webException) { $state.web_exception_status = [string]$webException.Status }
    if ($null -ne $webException -and $webException.Response -is [Net.HttpWebResponse]) { $state.http_code = [int]$webException.Response.StatusCode }
} finally {
    if ($null -ne $destination) { $destination.Dispose() }
    if ($null -ne $source) { $source.Dispose() }
    if ($null -ne $response) { $response.Close() }
    $state.exit_code = $code
    [IO.File]::WriteAllText($statusFile, ($state | ConvertTo-Json -Depth 8), (New-Object Text.UTF8Encoding($false)))
}
exit $code
'@
        $downloadCode = $downloadCode.Replace('__DOWNLOAD_URL_LITERAL__',(As-PSLiteral $sourceUrl)).Replace('__DOWNLOAD_PARTIAL_LITERAL__',(As-PSLiteral $partialPath)).Replace('__DOWNLOAD_STATUS_LITERAL__',(As-PSLiteral $downloadStatusPath))
        $report.download = Invoke-BoundedProcess $powershellPath @('-NoLogo','-NoProfile','-NonInteractive','-EncodedCommand',(Encode-PSCommand $downloadCode)) 126000
        $report.download['method'] = 'native-dotnet-fallback-in-isolated-process'
        if ([IO.File]::Exists($downloadStatusPath)) { $report.download['native_status'] = ([IO.File]::ReadAllText($downloadStatusPath) | ConvertFrom-Json) }
        if ($report.download.timed_out) { $resultCode = 12; throw 'Native .NET download exceeded the outer deadline; any bytes remain marked partial.' }
        if ($report.download.actual_exit_code -ne 0) { $resultCode = 13; throw 'Native .NET download failed; download is not complete.' }
        if (-not $report.download.Contains('native_status') -or -not $report.download.native_status.complete) { $resultCode = 14; throw 'Native .NET did not report complete verified-TLS HTTP 200 bytes.' }
    }
    if (-not [IO.File]::Exists($partialPath) -or (Get-Item -LiteralPath $partialPath).Length -le 0) { $resultCode = 14; throw 'No nonempty complete payload was downloaded.' }
    if ((Get-Item -LiteralPath $partialPath).Length -gt $maximumBytes) { $resultCode = 14; throw 'Complete payload exceeds the accepted byte limit; file remains marked partial.' }
    $report.phase = 'checking-full-sha256'
    $actualHash = (Get-FileHash -LiteralPath $partialPath -Algorithm SHA256).Hash.ToUpperInvariant()
    $report.hash = [ordered]@{ algorithm = 'SHA256'; expected = $expectedSha256; actual = $actualHash; matches = ($actualHash -ceq $expectedSha256); full_file_bytes = (Get-Item -LiteralPath $partialPath).Length }
    if (-not $report.hash.matches) { $resultCode = 20; throw 'Full payload SHA256 does not match the official manifest; file remains marked partial.' }
    Move-Item -LiteralPath $partialPath -Destination $downloadedPath
    $report.phase = 'checking-authenticode'
    $signatureStatusPath = Join-Path $workRoot 'authenticode-status.json'
    $signatureCode = @'
Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$payload = __SIGNATURE_PAYLOAD_LITERAL__
$output = __SIGNATURE_OUTPUT_LITERAL__
$code = 33
$state = [ordered]@{ status = $null; status_code = $null; signer = $null; timestamp = $null; vendor_signer_match = $false; accepted_signer_simple_names = @('Anthropic','Anthropic PBC','Anthropic, PBC'); error = $null; exit_code = 33 }
function Certificate-Record($certificate) {
    if ($null -eq $certificate) { return $null }
    return [ordered]@{ subject = $certificate.Subject; issuer = $certificate.Issuer; thumbprint = $certificate.Thumbprint; simple_name = $certificate.GetNameInfo([Security.Cryptography.X509Certificates.X509NameType]::SimpleName,$false); not_before_utc = $certificate.NotBefore.ToUniversalTime().ToString('o'); not_after_utc = $certificate.NotAfter.ToUniversalTime().ToString('o') }
}
try {
    $signature = Get-AuthenticodeSignature -FilePath $payload
    $state.status = [string]$signature.Status
    $state.status_code = [int]$signature.Status
    $state.signer = Certificate-Record $signature.SignerCertificate
    $state.timestamp = Certificate-Record $signature.TimeStamperCertificate
    if ($null -ne $state.signer) { $state.vendor_signer_match = ($state.signer.simple_name -match '^(?i:Anthropic(?:,\s*PBC|\s+PBC)?)$') }
    if ($signature.Status -ne [System.Management.Automation.SignatureStatus]::Valid) { $code = 31 }
    elseif (-not $state.vendor_signer_match) { $code = 32 }
    else { $code = 0 }
} catch { $state.error = $_.Exception.GetType().FullName }
finally {
    $state.exit_code = $code
    [IO.File]::WriteAllText($output, ($state | ConvertTo-Json -Depth 10), (New-Object Text.UTF8Encoding($false)))
}
exit $code
'@
    $signatureCode = $signatureCode.Replace('__SIGNATURE_PAYLOAD_LITERAL__',(As-PSLiteral $downloadedPath)).Replace('__SIGNATURE_OUTPUT_LITERAL__',(As-PSLiteral $signatureStatusPath))
    $report.signature_process = Invoke-BoundedProcess $powershellPath @('-NoLogo','-NoProfile','-NonInteractive','-EncodedCommand',(Encode-PSCommand $signatureCode)) ($signatureSeconds*1000)
    if ($report.signature_process.timed_out) { $resultCode = 30; throw 'Authenticode verification exceeded its process deadline.' }
    if ([IO.File]::Exists($signatureStatusPath)) { $report.signature = ([IO.File]::ReadAllText($signatureStatusPath) | ConvertFrom-Json) }
    if ($null -eq $report.signature) { $resultCode = 33; throw 'Signature verification returned no JSON status.' }
    if ($report.signature.status -ne 'Valid') { $resultCode = 31; throw 'Authenticode signature is not Valid.' }
    if (-not $report.signature.vendor_signer_match) { $resultCode = 32; throw 'Valid signature publisher does not match an accepted Anthropic identity.' }
    if ($report.signature_process.actual_exit_code -ne 0) { $resultCode = 33; throw 'Signature child returned a nonzero exit code.' }
    Move-Item -LiteralPath $downloadedPath -Destination $finalPath
    $report.verification_complete = $true
    $report.phase = 'verified-no-install'
    $resultCode = 0
} catch {
    $report.error = [ordered]@{ type = $_.Exception.GetType().FullName; message = (Protect-Diagnostic $_.Exception.Message) }
    $report.phase = 'failed-no-install'
} finally {
    $partialExists = $false; $downloadedExists = $false; $verifiedExists = $false
    if ($null -ne $partialPath) { $partialExists = [IO.File]::Exists($partialPath) }
    if ($null -ne $downloadedPath) { $downloadedExists = [IO.File]::Exists($downloadedPath) }
    if ($null -ne $finalPath) { $verifiedExists = [IO.File]::Exists($finalPath) }
    $retainedPath = $null
    if ($verifiedExists) { $retainedPath = $finalPath }
    elseif ($downloadedExists) { $retainedPath = $downloadedPath }
    elseif ($partialExists) { $retainedPath = $partialPath }
    $retainedBytes = [long]0
    if ($null -ne $retainedPath) {
        try { $retainedBytes = (Get-Item -LiteralPath $retainedPath).Length }
        catch { $retainedBytes = $null }
    }
    $report.artifact = [ordered]@{ directory = $workRoot; retained_path = $retainedPath; retained_bytes = $retainedBytes; partial_path = $partialPath; partial_exists = $partialExists; downloaded_sha_matched_path = $downloadedPath; downloaded_sha_matched_exists = $downloadedExists; verified_path = $finalPath; verified_exists = $verifiedExists; installed = $false; executed = $false }
    $report.exit_code = $resultCode
    $report.finished_utc = [DateTime]::UtcNow.ToString('o')
    if ($null -ne $resultPath) { Write-JsonFile $resultPath $report }
}
exit $resultCode
