# Windows PowerShell 5.1. Local proxy configuration metadata only.
# No network, PAC evaluation, environment values, credentials, or global changes.
Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$outRoot = 'C:\Users\han\AppData\Local\Orchester-build\continuation-native-access-20261113\owned-native-worker-attempt3-3fa131682683515d5d18a4e5\jobs\e7c481a8d190795acdb939dab33541a9'
$resultPath = $null
$resultCode = 1
$report = [ordered]@{
    schema_version = 1
    task = 'windows-system-proxy-metadata-readonly'
    started_utc = [DateTime]::UtcNow.ToString('o')
    finished_utc = $null
    exit_code = 1
    query_succeeded = $false
    wininet_current_user = $null
    winhttp_machine_default = $null
    error_type = $null
    pac_evaluated = $false
    network_requests_made = $false
    environment_values_read_or_emitted = $false
    credentials_emitted = $false
    global_proxy_dns_or_tls_modified = $false
}

function Pointer-ContainsText([IntPtr]$Pointer) {
    return ($Pointer -ne [IntPtr]::Zero -and [Runtime.InteropServices.Marshal]::ReadInt16($Pointer) -ne 0)
}

function Safe-ProxyEndpoints([string]$ProxyText) {
    $state = [ordered]@{
        proxy_configured = (-not [string]::IsNullOrWhiteSpace($ProxyText))
        loopback_proxy_configured = $false
        non_loopback_proxy_configured = $false
        endpoint_details_withheld = $false
        unparsed_proxy_configured = $false
        loopback_endpoints = @()
    }
    if (-not $state.proxy_configured) { return $state }
    $entries = $ProxyText.Split([char]';')
    if ($entries.Length -gt 64) { $state.unparsed_proxy_configured = $true; $state.endpoint_details_withheld = $true; return $state }
    $safeEndpoints = New-Object 'System.Collections.Generic.List[object]'
    $seen = New-Object 'System.Collections.Generic.HashSet[string]'
    foreach ($rawEntry in $entries) {
        $entry = $rawEntry.Trim()
        if ($entry.Length -eq 0) { continue }
        $targetProtocol = 'all'
        $address = $entry
        $mapping = [regex]::Match($entry,'^(?i:(http|https|ftp|socks|socks4|socks5))\s*=(.+)$')
        if ($mapping.Success) { $targetProtocol = $mapping.Groups[1].Value.ToLowerInvariant(); $address = $mapping.Groups[2].Value.Trim() }
        $uriText = $address
        if ($address -notmatch '^[A-Za-z][A-Za-z0-9+.-]*://') {
            # https= selects HTTPS destinations; an unqualified proxy is HTTP CONNECT.
            if ($targetProtocol -in @('socks','socks4','socks5')) { $uriText = $targetProtocol + '://' + $address }
            else { $uriText = 'http://' + $address }
        }
        $proxyUri = $null
        if (-not [Uri]::TryCreate($uriText,[UriKind]::Absolute,[ref]$proxyUri) -or $proxyUri.Scheme -notin @('http','https','socks','socks4','socks4a','socks5','socks5h') -or $proxyUri.Port -lt 1 -or $proxyUri.Port -gt 65535) {
            $state.unparsed_proxy_configured = $true; $state.endpoint_details_withheld = $true; continue
        }
        $proxyHost = $proxyUri.DnsSafeHost.Trim([char[]]'[]')
        $normalizedHost = $null
        if ($proxyHost -ieq 'localhost' -or $proxyHost -ieq 'localhost.') { $normalizedHost = 'localhost' }
        else {
            $ipAddress = $null
            if ([Net.IPAddress]::TryParse($proxyHost,[ref]$ipAddress) -and [Net.IPAddress]::IsLoopback($ipAddress)) { $normalizedHost = $ipAddress.ToString() }
        }
        if ($null -eq $normalizedHost) {
            $state.non_loopback_proxy_configured = $true; $state.endpoint_details_withheld = $true; continue
        }
        $proxyProtocol = $proxyUri.Scheme.ToLowerInvariant()
        $key = $proxyProtocol + '|' + $targetProtocol + '|' + $normalizedHost + '|' + $proxyUri.Port
        if ($seen.Add($key)) {
            $safeEndpoints.Add([pscustomobject][ordered]@{ protocol = $proxyProtocol; target_protocol = $targetProtocol; host = $normalizedHost; port = $proxyUri.Port })
        }
    }
    $state.loopback_endpoints = @($safeEndpoints.ToArray())
    $state.loopback_proxy_configured = ($state.loopback_endpoints.Count -gt 0)
    return $state
}

try {
    if (-not [IO.Path]::IsPathRooted($outRoot)) { throw 'Output directory placeholder was not replaced.' }
    $null = [IO.Directory]::CreateDirectory($outRoot)
    $resultPath = Join-Path $outRoot 'windows-system-proxy-metadata-readonly.json'
    if (-not ('Orchester.SafeProxyNativeV1' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace Orchester {
    [StructLayout(LayoutKind.Sequential)]
    public struct IeProxyConfigV1 {
        [MarshalAs(UnmanagedType.Bool)] public bool AutoDetect;
        public IntPtr AutoConfigUrl;
        public IntPtr Proxy;
        public IntPtr ProxyBypass;
    }
    [StructLayout(LayoutKind.Sequential)]
    public struct DefaultProxyConfigV1 {
        public uint AccessType;
        public IntPtr Proxy;
        public IntPtr ProxyBypass;
    }
    public static class SafeProxyNativeV1 {
        [DllImport("winhttp.dll", SetLastError=true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool WinHttpGetIEProxyConfigForCurrentUser(out IeProxyConfigV1 config);
        [DllImport("winhttp.dll", SetLastError=true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool WinHttpGetDefaultProxyConfiguration(out DefaultProxyConfigV1 config);
        [DllImport("kernel32.dll", SetLastError=true)]
        public static extern IntPtr GlobalFree(IntPtr pointer);
    }
}
'@
    }
    $ieConfig = New-Object Orchester.IeProxyConfigV1
    $ieState = [ordered]@{ query_succeeded = $false; native_error_code = 0; auto_detect_enabled = $false; pac_configured = $false; bypass_configured = $false; proxy = $null; error_type = $null }
    try {
        $ieSuccess = [Orchester.SafeProxyNativeV1]::WinHttpGetIEProxyConfigForCurrentUser([ref]$ieConfig)
        $ieError = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
        $ieState.query_succeeded = $ieSuccess
        if ($ieSuccess) {
            $ieState.auto_detect_enabled = $ieConfig.AutoDetect
            # Never decode or emit PAC/bypass strings, paths, queries, or credentials.
            $ieState.pac_configured = Pointer-ContainsText $ieConfig.AutoConfigUrl
            $ieState.bypass_configured = Pointer-ContainsText $ieConfig.ProxyBypass
            $proxyText = $null
            if (Pointer-ContainsText $ieConfig.Proxy) { $proxyText = [Runtime.InteropServices.Marshal]::PtrToStringUni($ieConfig.Proxy) }
            $ieState.proxy = Safe-ProxyEndpoints $proxyText
            $proxyText = $null
        } else { $ieState.native_error_code = $ieError }
    } catch { $ieState.error_type = $_.Exception.GetType().FullName }
    finally {
        foreach ($pointer in @($ieConfig.AutoConfigUrl,$ieConfig.Proxy,$ieConfig.ProxyBypass)) { if ($pointer -ne [IntPtr]::Zero) { $null = [Orchester.SafeProxyNativeV1]::GlobalFree($pointer) } }
    }
    $report.wininet_current_user = $ieState
    $machineConfig = New-Object Orchester.DefaultProxyConfigV1
    $machineState = [ordered]@{ query_succeeded = $false; native_error_code = 0; direct_configured = $false; automatic_proxy_mode_configured = $false; named_proxy_mode_configured = $false; bypass_configured = $false; proxy = $null; error_type = $null }
    try {
        $machineSuccess = [Orchester.SafeProxyNativeV1]::WinHttpGetDefaultProxyConfiguration([ref]$machineConfig)
        $machineError = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
        $machineState.query_succeeded = $machineSuccess
        if ($machineSuccess) {
            $machineState.direct_configured = ($machineConfig.AccessType -eq 1)
            $machineState.named_proxy_mode_configured = ($machineConfig.AccessType -eq 3)
            $machineState.automatic_proxy_mode_configured = ($machineConfig.AccessType -eq 4)
            $machineState.bypass_configured = Pointer-ContainsText $machineConfig.ProxyBypass
            $proxyText = $null
            if (Pointer-ContainsText $machineConfig.Proxy) { $proxyText = [Runtime.InteropServices.Marshal]::PtrToStringUni($machineConfig.Proxy) }
            $machineState.proxy = Safe-ProxyEndpoints $proxyText
            $proxyText = $null
        } else { $machineState.native_error_code = $machineError }
    } catch { $machineState.error_type = $_.Exception.GetType().FullName }
    finally {
        foreach ($pointer in @($machineConfig.Proxy,$machineConfig.ProxyBypass)) { if ($pointer -ne [IntPtr]::Zero) { $null = [Orchester.SafeProxyNativeV1]::GlobalFree($pointer) } }
    }
    $report.winhttp_machine_default = $machineState
    $report.query_succeeded = ($ieState.query_succeeded -and $machineState.query_succeeded -and $null -eq $ieState.error_type -and $null -eq $machineState.error_type)
    if ($report.query_succeeded) { $resultCode = 0 }
} catch { $report.error_type = $_.Exception.GetType().FullName }
finally {
    $report.finished_utc = [DateTime]::UtcNow.ToString('o')
    $report.exit_code = $resultCode
    if ($null -ne $resultPath) { [IO.File]::WriteAllText($resultPath, ($report | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false))) }
}
exit $resultCode
