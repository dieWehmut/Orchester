# Windows PowerShell 5.1; prepare/run as a separate metadata-only worker job.
# Reads the verified official page and actual official Linux href HEAD metadata.
# Never downloads a complete installer, starts GUI, or changes system settings.
Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$outRoot = 'C:\Users\han\AppData\Local\Orchester-build\continuation-native-access-20261113\owned-native-worker-attempt4-1ed007a58623f12c8e1c1223\jobs\fa5f68dd35d15a5eeb074ab50d626ed6'
$resultPath = $null
$workRoot = $null
$resultCode = 99
$report = [ordered]@{
    schema_version = 1
    task = 'claude-official-download-page-linux-hrefs-native-dotnet'
    started_utc = [DateTime]::UtcNow.ToString('o')
    finished_utc = $null
    exit_code = 99
    source_url = 'https://claude.ai/download'
    source_provenance = 'Official MCP merged PR 3426 links this vendor page; no payload URL is inferred.'
    process = $null
    native_status = $null
    artifact = $null
    error_type = $null
    limits = [ordered]@{ native_operation_seconds = 80; native_process_guard_seconds = 90; termination_grace_seconds = 5; output_drain_seconds = 5; html_maximum_bytes = 5242880; maximum_candidate_hrefs_for_head = 3; maximum_redirects_per_candidate = 5; maximum_total_head_requests = 18 }
    linux_release_nonexistence_concluded = $false
    complete_installer_downloaded = $false
    payload_executed = $false
    gui_started = $false
    tls_validation_disabled = $false
    global_proxy_dns_or_certificate_modified = $false
}

function Write-JsonFile([string]$Path, $Value) {
    [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 16), (New-Object Text.UTF8Encoding($false)))
}
function Protect-Diagnostic([string]$Text) {
    if ($null -eq $Text) { return '' }
    $safe = [regex]::Replace($Text,'(?im)^\s*(Authorization|Proxy-Authorization|Cookie|Set-Cookie)\s*:.*$','[sensitive header omitted]')
    $safe = [regex]::Replace($safe,'(?i)https?://[^\s<>]+','[URL omitted]')
    if ($safe.Length -gt 16384) { $safe = $safe.Substring(0,16384) + '[truncated]' }
    return $safe
}
function Quote-NativeArgument([string]$Argument) {
    if ($Argument.Contains([string][char]34) -or $Argument.Contains([string][char]0) -or $Argument.Contains([string][char]10) -or $Argument.Contains([string][char]13) -or $Argument.EndsWith('\')) { throw 'Unsupported native argument quoting input.' }
    return ([string][char]34 + $Argument + [string][char]34)
}
function Invoke-BoundedProcess([string]$Executable,[string[]]$Arguments,[int]$TimeoutMilliseconds) {
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
    $stdout = ''; $stderr = ''; $nativeCode = $null
    $timedOut = $false; $nativeExited = $false
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
            $null = [Threading.Tasks.Task]::WaitAll([Threading.Tasks.Task[]]@($stdoutTask,$stderrTask),5000)
        }
        if ($stdoutTask.IsCompleted -and -not $stdoutTask.IsFaulted -and -not $stdoutTask.IsCanceled) { $stdout = $stdoutTask.Result }
        if ($stderrTask.IsCompleted -and -not $stderrTask.IsFaulted -and -not $stderrTask.IsCanceled) { $stderr = $stderrTask.Result }
        return [ordered]@{ executable = $Executable; timed_out = $timedOut; native_exited = $nativeExited; actual_exit_code = $nativeCode; elapsed_seconds = [Math]::Round($watch.Elapsed.TotalSeconds,3); stdout = (Protect-Diagnostic $stdout); stderr = (Protect-Diagnostic $stderr) }
    } finally { $watch.Stop(); $process.Dispose() }
}
function Get-SystemPowerShell {
    $windowsRoot = [Environment]::GetFolderPath([Environment+SpecialFolder]::Windows)
    if ([Environment]::Is64BitOperatingSystem -and -not [Environment]::Is64BitProcess) { $systemRoot = Join-Path $windowsRoot 'Sysnative' }
    else { $systemRoot = Join-Path $windowsRoot 'System32' }
    return Join-Path $systemRoot 'WindowsPowerShell\v1.0\powershell.exe'
}
function As-PSLiteral([string]$Value) { return "'" + $Value.Replace("'","''") + "'" }

try {
    if (-not [IO.Path]::IsPathRooted($outRoot)) { $resultCode = 10; throw 'Worker output placeholder must be an absolute Windows directory.' }
    $null = [IO.Directory]::CreateDirectory($outRoot)
    $resultPath = Join-Path $outRoot 'cl-page.json'
    $workRoot = Join-Path $outRoot ('pg-' + [Guid]::NewGuid().ToString('N'))
    $null = [IO.Directory]::CreateDirectory($workRoot)
    $nativeStatusPath = Join-Path $workRoot 'n.json'
    $childScriptPath = Join-Path $workRoot 'r.ps1'
    $powershellPath = Get-SystemPowerShell
    if (-not [IO.File]::Exists($powershellPath)) { $resultCode = 10; throw 'System Windows PowerShell is unavailable.' }
    Write-JsonFile $resultPath $report
    $childCode = @'
Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$workRoot = __WORK_ROOT_LITERAL__
$statusPath = __STATUS_PATH_LITERAL__
$initialUri = [Uri]'https://claude.ai/download'
$allowedHosts = @('claude.ai','claude.com','www.claude.ai','www.claude.com','downloads.claude.ai')
$maximumHtmlBytes = [long]5242880
$operationSeconds = 80
$nativeCode = 13
$systemProxy = $null
$response = $null; $sourceStream = $null; $destination = $null
$partialPath = Join-Path $workRoot 'b.html.part'
$fullPath = Join-Path $workRoot 'b.html'
$state = [ordered]@{
    phase = 'initializing'; exit_code = 13; complete_html_read = $false
    source_url = 'https://claude.ai/download'; final_source_url_without_query = $null
    http_code = $null; content_type = $null; declared_content_length = $null; source_html_encoding_name = $null
    bytes_written = [long]0; body_sha256 = $null; retained_body_path = $null; body_is_partial = $true
    safe_proxy_meta = $null; page_trace = $null; redirects = 0; challenge_page_detected = $false
    linux_candidates = @(); linux_candidates_present = $false; head_results = @(); head_metadata_incomplete = $false
    candidate_limit_reached = $false; credential_bearing_hrefs_omitted = $false; unsupported_href_whitespace_omitted = $false
    base_href_present = $false; relative_hrefs_withheld_due_to_base = $false
    exception_type = $null; web_exception_status = $null; native_hresult = $null
    linux_release_nonexistence_concluded = $false; complete_installer_downloaded = $false
    payload_executed = $false; pac_or_bypass_values_emitted = $false
}
$watch = [Diagnostics.Stopwatch]::StartNew()

function Save-State { [IO.File]::WriteAllText($statusPath, ($state | ConvertTo-Json -Depth 16), (New-Object Text.UTF8Encoding($false))) }
function Remaining-Milliseconds {
    $remaining = [int](($operationSeconds - $watch.Elapsed.TotalSeconds)*1000)
    if ($remaining -le 0) { throw [TimeoutException]::new('Metadata operation deadline reached.') }
    return $remaining
}
function Uri-WithoutQuery([Uri]$Uri) {
    $value = $Uri.Scheme + '://' + $Uri.IdnHost
    if (-not $Uri.IsDefaultPort) { $value += ':' + $Uri.Port }
    return $value + $Uri.AbsolutePath
}
function Is-OfficialHttpsUri([Uri]$Uri) {
    return ($null -ne $Uri -and $Uri.Scheme -eq 'https' -and $Uri.Port -eq 443 -and $Uri.IdnHost -in $allowedHosts -and [string]::IsNullOrEmpty($Uri.UserInfo))
}
function Safe-ProxyMeta([Uri]$Target) {
    $safe = [ordered]@{ default_proxy_present = ($null -ne $systemProxy); target_is_bypassed = $null; resolved_route_is_direct = $false; resolved_proxy_is_loopback = $false; non_loopback_endpoint_withheld = $false; endpoint = $null; evaluation_error_type = $null }
    try {
        if ($null -eq $systemProxy) { $safe.target_is_bypassed = $true; $safe.resolved_route_is_direct = $true }
        else {
            $safe.target_is_bypassed = [bool]$systemProxy.IsBypassed($Target)
            $resolvedProxy = $systemProxy.GetProxy($Target)
            if ($safe.target_is_bypassed -or $null -eq $resolvedProxy -or $resolvedProxy.Equals($Target)) { $safe.resolved_route_is_direct = $true }
            else {
                $proxyHost = $resolvedProxy.DnsSafeHost.Trim([char[]]'[]')
                $normalizedHost = $null
                if ($proxyHost -ieq 'localhost' -or $proxyHost -ieq 'localhost.') { $normalizedHost = 'localhost' }
                else {
                    $ipAddress = $null
                    if ([Net.IPAddress]::TryParse($proxyHost,[ref]$ipAddress) -and [Net.IPAddress]::IsLoopback($ipAddress)) { $normalizedHost = $ipAddress.ToString() }
                }
                if ($null -ne $normalizedHost -and $resolvedProxy.Port -ge 1 -and $resolvedProxy.Port -le 65535 -and $resolvedProxy.Scheme -in @('http','https','socks','socks4','socks4a','socks5','socks5h')) {
                    $safe.resolved_proxy_is_loopback = $true
                    $safe.endpoint = [ordered]@{ protocol = $resolvedProxy.Scheme; host = $normalizedHost; port = $resolvedProxy.Port }
                } else { $safe.non_loopback_endpoint_withheld = $true }
            }
        }
    } catch { $safe.evaluation_error_type = $_.Exception.GetType().FullName }
    return $safe
}
function Record-Exception($Record, [Exception]$Exception) {
    $Record.exception_type = $Exception.GetType().FullName
    $Record.native_hresult = $Exception.HResult
    $cursor = $Exception
    for ($depth=0; $depth -lt 8 -and $null -ne $cursor; $depth++) {
        if ($cursor -is [Net.WebException]) {
            $Record.web_exception_status = [string]$cursor.Status
            if ($cursor.Response -is [Net.HttpWebResponse]) { $Record.http_code = [int]$cursor.Response.StatusCode }
            break
        }
        $cursor = $cursor.InnerException
    }
}
function Parse-AnchorAttributes([string]$Attributes, $AttributeRegex) {
    $parsed = [ordered]@{ valid = $true; href_seen = $false; href = $null; label_input = '' }
    $cursor = 0
    while ($cursor -lt $Attributes.Length) {
        if ($Attributes.Substring($cursor).Trim([char[]]@(9,10,12,13,32)).Length -eq 0) { break }
        # \G anchors each complete attribute token to the current cursor.
        # A different quoted attribute is consumed whole, never searched for href.
        $attribute = $AttributeRegex.Match($Attributes,$cursor)
        if (-not $attribute.Success -or $attribute.Index -ne $cursor -or $attribute.Length -le 0) { $parsed.valid = $false; return $parsed }
        $cursor += $attribute.Length
        $name = $attribute.Groups['name'].Value.ToLowerInvariant()
        $value = $null
        foreach ($groupName in @('d','s','u')) { if ($attribute.Groups[$groupName].Success) { $value = $attribute.Groups[$groupName].Value; break } }
        if ($name -eq 'href') {
            if ($parsed.href_seen) { $parsed.valid = $false; return $parsed }
            $parsed.href_seen = $true; $parsed.href = $value
        } elseif ($name -in @('aria-label','title') -and $null -ne $value) { $parsed.label_input += ' ' + [Net.WebUtility]::HtmlDecode($value) }
    }
    return $parsed
}
function Get-OfficialResponse([Uri]$StartUri, [string]$Method, $Trace) {
    $currentUri = $StartUri
    for ($redirect=0; $redirect -le 5; $redirect++) {
        if (-not (Is-OfficialHttpsUri $currentUri)) { $Trace.off_vendor_or_non_https_redirect_blocked = $true; throw [InvalidOperationException]::new('Only primary HTTPS vendor hosts may be requested.') }
        $remaining = Remaining-Milliseconds
        $request = [Net.HttpWebRequest]::Create($currentUri)
        $request.Method = $Method
        $request.Proxy = $systemProxy
        $request.AllowAutoRedirect = $false
        $request.UseDefaultCredentials = $false
        $request.PreAuthenticate = $false
        $requestTimeout = 20000
        if ($Method -eq 'HEAD') { $requestTimeout = 5000 }
        $request.Timeout = [Math]::Min($requestTimeout,$remaining)
        $request.ReadWriteTimeout = [Math]::Min(10000,$remaining)
        $request.Accept = 'text/html,application/octet-stream;q=0.5'
        $received = $null
        try { $received = $request.GetResponse() }
        catch {
            $cursor = $_.Exception
            for ($depth=0; $depth -lt 8 -and $null -ne $cursor; $depth++) {
                if ($cursor -is [Net.WebException] -and $cursor.Response -is [Net.HttpWebResponse]) { $received = $cursor.Response; break }
                $cursor = $cursor.InnerException
            }
            if ($null -eq $received) { throw }
        }
        $Trace.http_code = [int]$received.StatusCode
        $Trace.redirects = $redirect
        if ($Trace.http_code -in @(301,302,303,307,308)) {
            if ($redirect -ge 5) { $received.Close(); throw [InvalidOperationException]::new('Official redirect limit exceeded.') }
            $location = $received.Headers['Location']
            if ([string]::IsNullOrEmpty($location)) { $received.Close(); throw [InvalidOperationException]::new('Redirect has no location.') }
            $nextUri = [Uri]::new($currentUri,$location)
            $received.Close()
            $currentUri = $nextUri
            continue
        }
        $Trace.final_url_without_query = Uri-WithoutQuery $currentUri
        $Trace.final_url_query_present = (-not [string]::IsNullOrEmpty($currentUri.Query))
        $Trace.content_type = (($received.ContentType -split ';')[0]).Trim().ToLowerInvariant()
        $Trace.content_length = $received.ContentLength
        return [pscustomobject]@{ Response = $received; FinalUri = $currentUri }
    }
    throw [InvalidOperationException]::new('No final official response.')
}

try {
    if ($null -ne [Net.ServicePointManager]::ServerCertificateValidationCallback) { throw [InvalidOperationException]::new('Unexpected certificate validation callback.') }
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    [Net.ServicePointManager]::CheckCertificateRevocationList = $true
    $systemProxy = [Net.WebRequest]::DefaultWebProxy
    $state.safe_proxy_meta = Safe-ProxyMeta $initialUri
    $state.phase = 'proxy-route-evaluated-before-request'
    Save-State
    $pageTrace = [ordered]@{ http_code = $null; redirects = 0; final_url_without_query = $null; final_url_query_present = $false; content_type = $null; content_length = $null; off_vendor_or_non_https_redirect_blocked = $false }
    $state.page_trace = $pageTrace
    Save-State
    $page = Get-OfficialResponse $initialUri 'GET' $pageTrace
    $response = $page.Response
    $state.http_code = $pageTrace.http_code
    $state.redirects = $pageTrace.redirects
    $state.final_source_url_without_query = $pageTrace.final_url_without_query
    $state.content_type = $pageTrace.content_type
    $state.declared_content_length = $response.ContentLength
    if ($response.ContentLength -gt $maximumHtmlBytes) { throw [InvalidOperationException]::new('Declared page exceeds the HTML byte limit.') }
    if ($state.content_type -ne 'text/html') { $nativeCode = 14; throw [InvalidOperationException]::new('Response MIME type is not text/html; no platform conclusion.') }
    $state.phase = 'reading-official-html-response'
    Save-State
    $sourceStream = $response.GetResponseStream()
    $destination = [IO.FileStream]::new($partialPath,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
    $buffer = New-Object byte[] 65536
    while ($true) {
        $null = Remaining-Milliseconds
        $read = $sourceStream.Read($buffer,0,$buffer.Length)
        if ($read -eq 0) { break }
        if (($state.bytes_written + $read) -gt $maximumHtmlBytes) { throw [InvalidOperationException]::new('Read page exceeds the HTML byte limit.') }
        $destination.Write($buffer,0,$read); $state.bytes_written += $read
    }
    $destination.Flush($true); $destination.Dispose(); $destination = $null
    $sourceStream.Dispose(); $sourceStream = $null
    if ($response.ContentLength -ge 0 -and $state.bytes_written -ne $response.ContentLength) { throw [InvalidOperationException]::new('Body length does not match complete bytes.') }
    if ($state.bytes_written -le 0) { throw [InvalidOperationException]::new('HTML response is empty.') }
    $selectedEncoding = [Text.Encoding]::UTF8
    if (-not [string]::IsNullOrEmpty($response.CharacterSet)) { try { $selectedEncoding = [Text.Encoding]::GetEncoding($response.CharacterSet) } catch { } }
    $state.source_html_encoding_name = $selectedEncoding.WebName
    $html = $selectedEncoding.GetString([IO.File]::ReadAllBytes($partialPath))
    $state.challenge_page_detected = ($html -match '(?is)<title>\s*(Just a moment(?:\.\.\.)?|Attention Required!?)[^<]*</title>' -or $html -match '(?i)cf-chl-|/cdn-cgi/challenge-platform/')
    if ($state.http_code -ne 200) { $nativeCode = 13; throw [InvalidOperationException]::new('Non-200 HTML is response evidence, not a vendor platform listing.') }
    if ($state.challenge_page_detected) { $nativeCode = 14; throw [InvalidOperationException]::new('Challenge HTML is not a vendor platform listing.') }
    Move-Item -LiteralPath $partialPath -Destination $fullPath
    $state.complete_html_read = $true; $state.body_is_partial = $false
    $state.phase = 'extracting-actual-html-hrefs'
    Save-State
    $candidates = New-Object 'System.Collections.Generic.List[object]'
    $candidateUris = New-Object 'System.Collections.Generic.List[System.Uri]'
    $seen = New-Object 'System.Collections.Generic.HashSet[string]'
    $rxOptions = [Text.RegularExpressions.RegexOptions]::IgnoreCase -bor [Text.RegularExpressions.RegexOptions]::Singleline
    $inertRegex = [Text.RegularExpressions.Regex]::new('<!--.*?(?:-->|\z)|<!\[CDATA\[.*?(?:\]\]>|\z)|<[!?](?:"[^"]*(?:"|\z)|''[^'']*(?:''|\z)|[^>''"])*(?:>|\z)|<(?<inert>script|style|textarea|title|xmp|iframe|noembed|noframes|noscript)(?=[ \t\r\n\f/]|>)(?:"[^"]*(?:"|\z)|''[^'']*(?:''|\z)|[^>''"])*(?:>|\z).*?(?:</\k<inert>[ \t\r\n\f]*>|\z)|<plaintext(?=[ \t\r\n\f/]|>)(?:"[^"]*(?:"|\z)|''[^'']*(?:''|\z)|[^>''"])*(?:>|\z).*\z',$rxOptions,[TimeSpan]::FromSeconds(3))
    $htmlForAnchors = $inertRegex.Replace($html,[Text.RegularExpressions.MatchEvaluator]{ param($match) return [string]::new([char]32,$match.Length) })
    $tagRegex = [Text.RegularExpressions.Regex]::new('<(?<closing>/)?(?<name>[A-Za-z][^ \t\r\n\f/>]*)(?=[ \t\r\n\f]|/?>)(?<attrs>(?:"[^"]*(?:"|\z)|''[^'']*(?:''|\z)|[^>''"])*?)(?<end>>|\z)',$rxOptions,[TimeSpan]::FromSeconds(3))
    $attributeRegex = [Text.RegularExpressions.Regex]::new('\G[ \t\r\n\f]+(?<name>[^ \t\r\n\f=<>/''"]+)(?:[ \t\r\n\f]*=[ \t\r\n\f]*(?:"(?<d>[^"]*)"|''(?<s>[^'']*)''|(?<u>[^ \t\r\n\f''"=<>]+)))?',$rxOptions,[TimeSpan]::FromSeconds(3))
    $tags = $tagRegex.Matches($htmlForAnchors)
    # Template contents are inert both as href sources and as surrounding a-label text.
    $templateRanges = New-Object 'System.Collections.Generic.List[object]'
    $rangeDepth = 0; $rangeStart = -1
    foreach ($rangeTag in $tags) {
        if ($rangeTag.Groups['name'].Value -ine 'template') { continue }
        if ($rangeTag.Groups['closing'].Success) {
            if ($rangeDepth -gt 0) {
                $rangeDepth--
                if ($rangeDepth -eq 0) { $templateRanges.Add([pscustomobject]@{ Start = $rangeStart; Length = $rangeTag.Index + $rangeTag.Length - $rangeStart }) }
            }
        } else { if ($rangeDepth -eq 0) { $rangeStart = $rangeTag.Index }; $rangeDepth++ }
    }
    if ($rangeDepth -gt 0) { $templateRanges.Add([pscustomobject]@{ Start = $rangeStart; Length = $htmlForAnchors.Length - $rangeStart }) }
    if ($templateRanges.Count -gt 0) {
        $templateMasked = [Text.StringBuilder]::new($htmlForAnchors)
        foreach ($range in $templateRanges) { $null = $templateMasked.Remove($range.Start,$range.Length); $null = $templateMasked.Insert($range.Start,[string]::new([char]32,$range.Length)) }
        $htmlForAnchors = $templateMasked.ToString()
        $tags = $tagRegex.Matches($htmlForAnchors)
    }
    # A real base element changes relative URL semantics. Do not infer its effect.
    foreach ($baseTag in $tags) {
        if ($baseTag.Groups['name'].Value -ine 'base' -or $baseTag.Groups['closing'].Success -or $baseTag.Groups['end'].Value -ne '>') { continue }
        $baseAttributes = Parse-AnchorAttributes $baseTag.Groups['attrs'].Value $attributeRegex
        if ($baseAttributes.valid -and $baseAttributes.href_seen) { $state.base_href_present = $true; break }
    }
    $templateDepth = 0
    for ($tagIndex=0; $tagIndex -lt $tags.Count; $tagIndex++) {
        $null = Remaining-Milliseconds
        $tag = $tags[$tagIndex]
        $tagName = $tag.Groups['name'].Value.ToLowerInvariant()
        $closingTag = $tag.Groups['closing'].Success
        if ($tagName -eq 'template') {
            if ($closingTag) { if ($templateDepth -gt 0) { $templateDepth-- } }
            else { $templateDepth++ }
            continue
        }
        if ($templateDepth -gt 0 -or $tagName -ne 'a' -or $closingTag -or $tag.Groups['end'].Value -ne '>' -or $tag.Value -notmatch '(?i)^<a(?=[ \t\r\n\f]|>)' -or $tag.Groups['attrs'].Length -gt 12000) { continue }
        $parsedAttributes = Parse-AnchorAttributes $tag.Groups['attrs'].Value $attributeRegex
        if (-not $parsedAttributes.valid -or -not $parsedAttributes.href_seen -or $null -eq $parsedAttributes.href) { continue }
        $rawHref = [Net.WebUtility]::HtmlDecode($parsedAttributes.href).Trim([char[]](0..32))
        $unsupportedWhitespace = $false
        foreach ($hrefCharacter in $rawHref.ToCharArray()) { if ([char]::IsWhiteSpace($hrefCharacter) -and [int]$hrefCharacter -gt 32) { $unsupportedWhitespace = $true; break } }
        if ($unsupportedWhitespace) { $state.unsupported_href_whitespace_omitted = $true; continue }
        if ([string]::IsNullOrEmpty($rawHref) -or $rawHref.Length -gt 8192) { continue }
        if ($state.base_href_present -and $rawHref -notmatch '^[A-Za-z][A-Za-z0-9+.-]*:') { $state.relative_hrefs_withheld_due_to_base = $true; continue }
        $candidateUri = $null
        if (-not [Uri]::TryCreate($page.FinalUri,$rawHref,[ref]$candidateUri)) { continue }
        if (-not [string]::IsNullOrEmpty($candidateUri.UserInfo)) { $state.credential_bearing_hrefs_omitted = $true; continue }
        if (-not (Is-OfficialHttpsUri $candidateUri)) { continue }
        $bodyStart = $tag.Index + $tag.Length
        $bodyEnd = [Math]::Min($htmlForAnchors.Length,$bodyStart + 12000)
        for ($nextTagIndex=$tagIndex+1; $nextTagIndex -lt $tags.Count; $nextTagIndex++) {
            $nextTag = $tags[$nextTagIndex]
            if ($nextTag.Index -ge $bodyEnd) { break }
            if ($nextTag.Groups['name'].Value -ieq 'a') { $bodyEnd = $nextTag.Index; break }
        }
        $labelHtml = $htmlForAnchors.Substring($bodyStart,$bodyEnd-$bodyStart)
        $label = [Net.WebUtility]::HtmlDecode($tagRegex.Replace($labelHtml,' ')) + $parsedAttributes.label_input
        $labelMarkers = New-Object 'System.Collections.Generic.List[string]'
        foreach ($marker in @('Linux','Ubuntu','Debian')) { if ($label -match ('(?i)\b' + $marker + '\b')) { $labelMarkers.Add($marker) } }
        $pathIndicator = ($candidateUri.AbsolutePath -match '(?i)linux|ubuntu|debian|\.(deb|appimage)(?:$|/)')
        $labelIndicator = ($labelMarkers.Count -gt 0)
        if (-not $pathIndicator -and -not $labelIndicator) { continue }
        if (-not $seen.Add($candidateUri.AbsoluteUri)) { continue }
        if ($candidates.Count -ge 64) { $state.candidate_limit_reached = $true; break }
        $exactUrl = $null; $exactHref = $null
        $queryOrFragmentPresent = (-not [string]::IsNullOrEmpty($candidateUri.Query) -or -not [string]::IsNullOrEmpty($candidateUri.Fragment))
        # Query/fragment-bearing URLs remain in the private source HTML and in memory for HEAD.
        # Do not emit possible anonymous tokens or credentials in URL diagnostics.
        if (-not $queryOrFragmentPresent) { $exactUrl = $candidateUri.AbsoluteUri; $exactHref = $rawHref }
        $index = $candidates.Count
        $candidates.Add([pscustomobject][ordered]@{ candidate_index = $index; exact_url = $exactUrl; url_without_query = (Uri-WithoutQuery $candidateUri); exact_href_html_decoded = $exactHref; query_or_fragment_withheld = $queryOrFragmentPresent; source_anchor_character_offset = $tag.Index; source_offset_units = 'decoded HTML UTF-16 code units'; linux_label_markers = @($labelMarkers.ToArray()); href_path_linux_indicator = $pathIndicator; anchor_linux_indicator = $labelIndicator; package_extension_hint = ($candidateUri.AbsolutePath -match '(?i)\.(deb|appimage)$'); source = 'exact href attribute in non-inert actual HTML a tag; scripts/styles/comments/raw text/templates ignored'; payload_bytes_hash_or_signature_verified = $false })
        $candidateUris.Add($candidateUri)
    }
    $state.linux_candidates = @($candidates.ToArray())
    $state.linux_candidates_present = ($candidates.Count -gt 0)
    $state.phase = 'actual-official-linux-href-head-metadata'
    Save-State
    $headResults = New-Object 'System.Collections.Generic.List[object]'
    for ($candidateIndex=0; $candidateIndex -lt [Math]::Min(3,$candidateUris.Count); $candidateIndex++) {
        if ($watch.Elapsed.TotalSeconds -ge ($operationSeconds - 2)) { $state.head_metadata_incomplete = $true; break }
        $headResponse = $null
        $head = [ordered]@{ candidate_index = $candidateIndex; method = 'HEAD'; request_url_without_query = (Uri-WithoutQuery $candidateUris[$candidateIndex]); exact_query_or_fragment_withheld = $candidates[$candidateIndex].query_or_fragment_withheld; safe_proxy_meta = (Safe-ProxyMeta $candidateUris[$candidateIndex]); http_code = $null; redirects = 0; final_url_without_query = $null; final_url_query_present = $false; content_type = $null; content_length = $null; off_vendor_or_non_https_redirect_blocked = $false; exception_type = $null; web_exception_status = $null; native_hresult = $null; body_read = $false; payload_verified = $false }
        try { $headAnswer = Get-OfficialResponse $candidateUris[$candidateIndex] 'HEAD' $head; $headResponse = $headAnswer.Response }
        catch { Record-Exception $head $_.Exception }
        finally { if ($null -ne $headResponse) { $headResponse.Close() } }
        $headResults.Add([pscustomobject]$head)
        $state.head_results = @($headResults.ToArray())
        Save-State
    }
    $state.phase = 'official-html-and-derived-head-metadata-complete'
    $nativeCode = 0
} catch {
    Record-Exception $state $_.Exception
    $scanException = $_.Exception
    for ($depth=0; $depth -lt 8 -and $null -ne $scanException; $depth++) {
        if ($scanException -is [TimeoutException]) { $nativeCode = 12; break }
        $scanException = $scanException.InnerException
    }
    $state.phase = 'failed-no-platform-nonexistence-conclusion'
} finally {
    if ($null -ne $destination) { $destination.Dispose() }
    if ($null -ne $sourceStream) { $sourceStream.Dispose() }
    if ($null -ne $response) { $response.Close() }
    if ([IO.File]::Exists($fullPath)) { $state.retained_body_path = $fullPath }
    elseif ([IO.File]::Exists($partialPath)) { $state.retained_body_path = $partialPath }
    if ($null -ne $state.retained_body_path) { try { $state.body_sha256 = (Get-FileHash -LiteralPath $state.retained_body_path -Algorithm SHA256).Hash; $state.bytes_written = (Get-Item -LiteralPath $state.retained_body_path).Length } catch { } }
    $state.exit_code = $nativeCode
    Save-State
}
exit $nativeCode
'@
    $childCode = $childCode.Replace('__WORK_ROOT_LITERAL__',(As-PSLiteral $workRoot)).Replace('__STATUS_PATH_LITERAL__',(As-PSLiteral $nativeStatusPath))
    [IO.File]::WriteAllText($childScriptPath,$childCode,(New-Object Text.UTF8Encoding($false)))
    # A short encoded loader avoids CreateProcess command-line length limits.
    $loader = '& ([ScriptBlock]::Create([IO.File]::ReadAllText(' + (As-PSLiteral $childScriptPath) + ')))'
    $encodedLoader = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($loader))
    $report.process = Invoke-BoundedProcess $powershellPath @('-NoLogo','-NoProfile','-NonInteractive','-EncodedCommand',$encodedLoader) 90000
    if ([IO.File]::Exists($nativeStatusPath)) { $report.native_status = ([IO.File]::ReadAllText($nativeStatusPath) | ConvertFrom-Json) }
    if ($report.process.timed_out) { $resultCode = 12 }
    elseif ($null -eq $report.native_status) { $resultCode = 15 }
    elseif ($report.process.actual_exit_code -eq 0 -and $report.native_status.complete_html_read) { $resultCode = 0 }
    else { $resultCode = 13 }
} catch { $report.error_type = $_.Exception.GetType().FullName }
finally {
    $artifactPath = $null; $artifactHash = $null; $artifactBytes = [long]0; $artifactPartial = $true
    if ($null -ne $workRoot) {
        $candidateFull = Join-Path $workRoot 'b.html'
        $candidatePartial = Join-Path $workRoot 'b.html.part'
        if ([IO.File]::Exists($candidateFull)) { $artifactPath = $candidateFull; $artifactPartial = $false }
        elseif ([IO.File]::Exists($candidatePartial)) { $artifactPath = $candidatePartial }
        if ($null -ne $artifactPath) { try { $artifactBytes = (Get-Item -LiteralPath $artifactPath).Length; $artifactHash = (Get-FileHash -LiteralPath $artifactPath -Algorithm SHA256).Hash } catch { } }
    }
    $report.artifact = [ordered]@{ directory = $workRoot; retained_html_path = $artifactPath; retained_html_bytes = $artifactBytes; retained_html_sha256 = $artifactHash; partial = $artifactPartial }
    $report.exit_code = $resultCode
    $report.finished_utc = [DateTime]::UtcNow.ToString('o')
    if ($null -ne $resultPath) { Write-JsonFile $resultPath $report }
}
exit $resultCode
