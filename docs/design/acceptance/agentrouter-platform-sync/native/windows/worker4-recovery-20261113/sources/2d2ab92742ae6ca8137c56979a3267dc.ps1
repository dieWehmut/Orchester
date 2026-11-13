$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

$expectedGuiPid = [uint32]37324
$expectedGuiCreated = [Int64]134389718070239855
$expectedGuiSession = [uint32]1
$expectedGuiSha256 = 'e4aa765befa85c7fb053075844be5693685c00677dd62b4cfff4592ec68c1d05'
$expectedGuiImage = 'C:\Users\han\AppData\Local\Orchester-build\monitor-final-20261110\monitors-final21-71def64-20261111\desktop-candidate\orchester-desktop.exe'
$resultDirectory = 'C:\Users\han\AppData\Local\Orchester-build\continuation-native-access-20261113\owned-native-worker-attempt4-1ed007a58623f12c8e1c1223\jobs\2d2ab92742ae6ca8137c56979a3267dc'
$resultName = 'input-desktop-readiness-safe.json'
$stage = 'validate-owned-output-directory'
$outputValidated = $false
$freshGuiHandle = [IntPtr]::Zero
$imageStream = $null
$nativeLoaded = $false
$exitCode = 1
$result = [ordered]@{
    schemaVersion = 1
    status = 'native-input-desktop-readiness-observation-failed'
    actualTargetHost = 'Windows'
    observedAtUtc = $null
    expectedGuiPid = $expectedGuiPid
    expectedGuiCreatedFileTimeExact = $expectedGuiCreated.ToString()
    actualGuiCreatedFileTimeExact = $null
    expectedGuiBinarySha256 = $expectedGuiSha256
    expectedGuiSessionId = $expectedGuiSession
    freshOriginalGuiHandleOpened = $false
    freshOriginalGuiExactBirthMatched = $false
    freshOriginalGuiImagePathMatched = $false
    freshOriginalGuiBinarySha256Matched = $false
    freshOriginalGuiLiveThroughObservation = $false
    freshOriginalGuiHandleReleased = $null
    guiSessionId = $null
    workerSessionId = $null
    nativeContext = $null
    readiness = $null
    errorStage = $null
    errorKind = $null
    failedNativeOperation = $null
    failedNativeError = $null
    cursorCoordinatesIncluded = $false
    rawOtherObjectNamesIncluded = $false
    usernamesOrSidsIncluded = $false
    otherWindowTitlesRead = $false
    mouseOrKeyboardInputSent = 0
    desktopOrWindowStationChanges = 0
    explicitApplicationLaunchOrTerminationRequests = 0
    helperCompilationMayStartOwnCompilerProcess = $true
    systemConfigurationWrites = 0
    observationIsInstantaneous = $true
    lockOrUnlockStateClaimed = $false
}

function Write-OwnedJson($Value, [string]$Path) {
    $temporary = $Path + '.tmp-' + [Guid]::NewGuid().ToString('N')
    $stream = $null
    try {
        $bytes = ([Text.UTF8Encoding]::new($false)).GetBytes((ConvertTo-Json -InputObject $Value -Depth 10))
        $stream = [IO.File]::Open($temporary, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Flush($true)
        $stream.Dispose()
        $stream = $null
        [IO.File]::Move($temporary, $Path)
    } finally {
        if ($null -ne $stream) { $stream.Dispose() }
        if ([IO.File]::Exists($temporary)) { [IO.File]::Delete($temporary) }
    }
}

try {
    if ($resultDirectory -cnotmatch '\A[A-Za-z]:\\') {
        throw [InvalidOperationException]::new('job-output-placeholder-not-expanded')
    }
    $resultDirectory = [IO.Path]::GetFullPath($resultDirectory)
    $attributes = [IO.File]::GetAttributes($resultDirectory)
    if (($attributes -band [IO.FileAttributes]::Directory) -eq 0 -or
        ($attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw [IO.IOException]::new('owned-output-directory-not-plain')
    }
    $resultPath = Join-Path $resultDirectory $resultName
    if ([IO.File]::Exists($resultPath)) { throw [IO.IOException]::new('owned-output-already-used') }
    $outputValidated = $true
    $stage = 'load-read-only-native-probes'

    if ($null -eq ('OrchesterInputDesktopReadiness20261113.Native' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;

namespace OrchesterInputDesktopReadiness20261113 {
    public sealed class NameProbe {
        public bool handleAvailable;
        public bool nameAvailable;
        public string nameClass;
        public int nativeError;
        public int nameQueryNativeError;
    }
    public sealed class ContextProbe {
        public uint workerPid;
        public uint workerThreadId;
        public NameProbe windowStation;
        public NameProbe threadDesktop;
        public NameProbe inputDesktop;
        public uint openInputDesktopFlags;
        public uint openInputDesktopDesiredAccess;
        public bool openInputDesktopInherit;
        public bool? inputDesktopCloseSucceeded;
        public int inputDesktopCloseNativeError;
        public bool getCursorPosSucceeded;
        public int getCursorPosNativeError;
        public bool activeConsoleSessionAvailable;
        public uint? activeConsoleSessionId;
        public bool activeConsoleSessionEqualsExpectedGuiSession;
    }
    public static class Native {
        private const uint PROCESS_QUERY_LIMITED_INFORMATION = 0x1000;
        private const uint SYNCHRONIZE = 0x00100000;
        private const uint WAIT_TIMEOUT = 258;
        private const uint WAIT_FAILED = 0xFFFFFFFF;
        private const uint DESKTOP_READOBJECTS = 0x0001;
        private const int UOI_NAME = 2;
        [StructLayout(LayoutKind.Sequential)]
        private struct POINT { public int X; public int Y; }

        [DllImport("kernel32.dll", EntryPoint = "SetLastError", SetLastError = true)]
        private static extern void ResetLastError(uint error);
        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern IntPtr OpenProcess(uint access, [MarshalAs(UnmanagedType.Bool)] bool inherit, uint pid);
        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool GetProcessTimes(IntPtr process, out long creation, out long exit, out long kernel, out long user);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool QueryFullProcessImageNameW(IntPtr process, uint flags, StringBuilder image, ref uint size);
        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern uint WaitForSingleObject(IntPtr handle, uint milliseconds);
        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool CloseHandle(IntPtr handle);
        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool ProcessIdToSessionId(uint pid, out uint session);
        [DllImport("kernel32.dll")]
        private static extern uint GetCurrentProcessId();
        [DllImport("kernel32.dll")]
        private static extern uint GetCurrentThreadId();
        [DllImport("kernel32.dll")]
        private static extern uint WTSGetActiveConsoleSessionId();
        [DllImport("user32.dll", SetLastError = true)]
        private static extern IntPtr GetProcessWindowStation();
        [DllImport("user32.dll", SetLastError = true)]
        private static extern IntPtr GetThreadDesktop(uint threadId);
        [DllImport("user32.dll", SetLastError = true)]
        private static extern IntPtr OpenInputDesktop(uint flags, [MarshalAs(UnmanagedType.Bool)] bool inherit, uint access);
        [DllImport("user32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool CloseDesktop(IntPtr desktop);
        [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool GetUserObjectInformationW(IntPtr handle, int index, StringBuilder value, uint lengthBytes, out uint neededBytes);
        [DllImport("user32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool GetCursorPos(out POINT point);

        public static IntPtr OpenReadOnlyGui(uint pid, out int error) {
            ResetLastError(0);
            IntPtr handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, false, pid);
            error = handle == IntPtr.Zero ? Marshal.GetLastWin32Error() : 0;
            return handle;
        }
        public static bool ReadCreationTime(IntPtr handle, out long creation, out int error) {
            long exit, kernel, user;
            ResetLastError(0);
            bool success = GetProcessTimes(handle, out creation, out exit, out kernel, out user);
            error = success ? 0 : Marshal.GetLastWin32Error();
            return success;
        }
        public static bool ReadImagePath(IntPtr handle, out string image, out int error) {
            StringBuilder buffer = new StringBuilder(32768);
            uint size = (uint)buffer.Capacity;
            ResetLastError(0);
            bool success = QueryFullProcessImageNameW(handle, 0, buffer, ref size);
            error = success ? 0 : Marshal.GetLastWin32Error();
            image = success ? buffer.ToString() : null;
            return success;
        }
        public static bool IsLive(IntPtr handle, out int error) {
            ResetLastError(0);
            uint status = WaitForSingleObject(handle, 0);
            error = status == WAIT_FAILED ? Marshal.GetLastWin32Error() : 0;
            return status == WAIT_TIMEOUT;
        }
        public static bool ReleaseGui(IntPtr handle, out int error) {
            ResetLastError(0);
            bool success = CloseHandle(handle);
            error = success ? 0 : Marshal.GetLastWin32Error();
            return success;
        }
        public static bool ReadSession(uint pid, out uint session, out int error) {
            ResetLastError(0);
            bool success = ProcessIdToSessionId(pid, out session);
            error = success ? 0 : Marshal.GetLastWin32Error();
            return success;
        }
        private static NameProbe ReadName(IntPtr handle, bool station, int handleError) {
            NameProbe probe = new NameProbe();
            probe.handleAvailable = handle != IntPtr.Zero;
            probe.nameClass = station ? "unavailable" : "other";
            probe.nativeError = handleError;
            if (!probe.handleAvailable) return probe;
            // UOI_NAME only. Raw names and UOI_USER_SID are never returned.
            StringBuilder buffer = new StringBuilder(512);
            uint needed;
            ResetLastError(0);
            bool success = GetUserObjectInformationW(handle, UOI_NAME, buffer, (uint)buffer.Capacity * 2, out needed);
            probe.nameQueryNativeError = success ? 0 : Marshal.GetLastWin32Error();
            probe.nameAvailable = success;
            if (!success) return probe;
            string value = buffer.ToString();
            if (station) {
                probe.nameClass = String.Equals(value, "WinSta0", StringComparison.OrdinalIgnoreCase) ? "WinSta0" : "other";
            } else if (String.Equals(value, "Default", StringComparison.OrdinalIgnoreCase)) {
                probe.nameClass = "Default";
            } else if (String.Equals(value, "Winlogon", StringComparison.OrdinalIgnoreCase)) {
                probe.nameClass = "Winlogon";
            } else {
                probe.nameClass = "other";
            }
            return probe;
        }
        public static ContextProbe Observe(uint expectedSession) {
            ContextProbe probe = new ContextProbe();
            probe.workerPid = GetCurrentProcessId();
            probe.workerThreadId = GetCurrentThreadId();
            // Current window station and thread desktop handles are borrowed.
            ResetLastError(0);
            IntPtr station = GetProcessWindowStation();
            int stationError = station == IntPtr.Zero ? Marshal.GetLastWin32Error() : 0;
            probe.windowStation = ReadName(station, true, stationError);
            ResetLastError(0);
            IntPtr threadDesktop = GetThreadDesktop(probe.workerThreadId);
            int threadError = threadDesktop == IntPtr.Zero ? Marshal.GetLastWin32Error() : 0;
            probe.threadDesktop = ReadName(threadDesktop, false, threadError);

            probe.openInputDesktopFlags = 0;
            probe.openInputDesktopDesiredAccess = DESKTOP_READOBJECTS;
            probe.openInputDesktopInherit = false;
            ResetLastError(0);
            IntPtr inputDesktop = OpenInputDesktop(0, false, DESKTOP_READOBJECTS);
            int inputError = inputDesktop == IntPtr.Zero ? Marshal.GetLastWin32Error() : 0;
            try {
                probe.inputDesktop = ReadName(inputDesktop, false, inputError);
                POINT point;
                ResetLastError(0);
                probe.getCursorPosSucceeded = GetCursorPos(out point);
                probe.getCursorPosNativeError = probe.getCursorPosSucceeded ? 0 : Marshal.GetLastWin32Error();
                // The cursor coordinates are intentionally discarded.
                uint activeConsole = WTSGetActiveConsoleSessionId();
                probe.activeConsoleSessionAvailable = activeConsole != 0xFFFFFFFF;
                probe.activeConsoleSessionId = probe.activeConsoleSessionAvailable ? (uint?)activeConsole : null;
                probe.activeConsoleSessionEqualsExpectedGuiSession = probe.activeConsoleSessionAvailable && activeConsole == expectedSession;
            } finally {
                if (inputDesktop != IntPtr.Zero) {
                    ResetLastError(0);
                    bool closed = CloseDesktop(inputDesktop);
                    probe.inputDesktopCloseNativeError = closed ? 0 : Marshal.GetLastWin32Error();
                    probe.inputDesktopCloseSucceeded = closed;
                }
            }
            return probe;
        }
    }
}
'@
    }
    $nativeLoaded = $true
    $stage = 'fresh-read-only-original-gui-handle'
    $nativeError = [int]0
    $freshGuiHandle = [OrchesterInputDesktopReadiness20261113.Native]::OpenReadOnlyGui($expectedGuiPid, [ref]$nativeError)
    if ($freshGuiHandle -eq [IntPtr]::Zero) {
        $result.failedNativeOperation = 'OpenProcess-query-limited-synchronize'
        $result.failedNativeError = $nativeError
        throw [InvalidOperationException]::new('original-gui-handle-unavailable')
    }
    $result.freshOriginalGuiHandleOpened = $true
    if (-not [OrchesterInputDesktopReadiness20261113.Native]::IsLive($freshGuiHandle, [ref]$nativeError)) {
        $result.failedNativeOperation = 'WaitForSingleObject-before'
        $result.failedNativeError = $nativeError
        throw [InvalidOperationException]::new('original-gui-not-live')
    }
    $stage = 'fresh-original-gui-exact-birth'
    $created = [Int64]0
    if (-not [OrchesterInputDesktopReadiness20261113.Native]::ReadCreationTime($freshGuiHandle, [ref]$created, [ref]$nativeError)) {
        $result.failedNativeOperation = 'GetProcessTimes'
        $result.failedNativeError = $nativeError
        throw [InvalidOperationException]::new('original-gui-birth-unavailable')
    }
    if ($created -ne $expectedGuiCreated) { throw [InvalidOperationException]::new('original-gui-exact-birth-differs') }
    $result.actualGuiCreatedFileTimeExact = $created.ToString()
    $result.freshOriginalGuiExactBirthMatched = $true
    $stage = 'fresh-original-gui-image-pin'
    $image = [string]$null
    if (-not [OrchesterInputDesktopReadiness20261113.Native]::ReadImagePath($freshGuiHandle, [ref]$image, [ref]$nativeError)) {
        $result.failedNativeOperation = 'QueryFullProcessImageNameW'
        $result.failedNativeError = $nativeError
        throw [InvalidOperationException]::new('original-gui-image-unavailable')
    }
    if (-not [string]::Equals($image, $expectedGuiImage, [StringComparison]::OrdinalIgnoreCase)) {
        throw [InvalidOperationException]::new('original-gui-image-path-differs')
    }
    $result.freshOriginalGuiImagePathMatched = $true
    $imageSharing = [IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete
    $imageStream = [IO.File]::Open($image, [IO.FileMode]::Open, [IO.FileAccess]::Read, $imageSharing)
    $algorithm = [Security.Cryptography.SHA256]::Create()
    try { $imageSha256 = ([BitConverter]::ToString($algorithm.ComputeHash($imageStream))).Replace('-', '').ToLowerInvariant() }
    finally {
        $algorithm.Dispose()
        $imageStream.Dispose()
        $imageStream = $null
    }
    if ($imageSha256 -cne $expectedGuiSha256) { throw [InvalidOperationException]::new('original-gui-binary-sha256-differs') }
    $result.freshOriginalGuiBinarySha256Matched = $true
    $stage = 'fresh-original-gui-and-worker-sessions'
    $guiSession = [uint32]0
    $workerSession = [uint32]0
    if (-not [OrchesterInputDesktopReadiness20261113.Native]::ReadSession($expectedGuiPid, [ref]$guiSession, [ref]$nativeError)) {
        $result.failedNativeOperation = 'ProcessIdToSessionId-gui'
        $result.failedNativeError = $nativeError
        throw [InvalidOperationException]::new('gui-session-unavailable')
    }
    if (-not [OrchesterInputDesktopReadiness20261113.Native]::ReadSession([uint32]$PID, [ref]$workerSession, [ref]$nativeError)) {
        $result.failedNativeOperation = 'ProcessIdToSessionId-worker'
        $result.failedNativeError = $nativeError
        throw [InvalidOperationException]::new('worker-session-unavailable')
    }
    $result.guiSessionId = $guiSession
    $result.workerSessionId = $workerSession
    $stage = 'observe-native-window-station-input-desktop-and-cursor'
    $context = [OrchesterInputDesktopReadiness20261113.Native]::Observe($expectedGuiSession)
    $result.nativeContext = $context
    if ($context.inputDesktopCloseSucceeded -eq $false) {
        $result.failedNativeOperation = 'CloseDesktop-owned-input-desktop-query-handle'
        $result.failedNativeError = $context.inputDesktopCloseNativeError
        throw [InvalidOperationException]::new('owned-input-desktop-handle-close-failed')
    }
    $stationReady = $context.windowStation.nameAvailable -and $context.windowStation.nameClass -ceq 'WinSta0'
    $threadReady = $context.threadDesktop.nameAvailable -and $context.threadDesktop.nameClass -ceq 'Default'
    $inputReady = $context.inputDesktop.nameAvailable -and $context.inputDesktop.nameClass -ceq 'Default'
    $sameSession = $guiSession -eq $workerSession
    $expectedSessions = $sameSession -and $guiSession -eq $expectedGuiSession
    $contextReady = $stationReady -and $threadReady -and $inputReady -and $context.getCursorPosSucceeded
    $result.readiness = [ordered]@{
        freshExactOriginalGuiIdentityMatched = $true
        guiAndWorkerSameSession = $sameSession
        guiAndWorkerInExpectedSession1 = $expectedSessions
        workerWindowStationObservedWinSta0 = $stationReady
        workerThreadDesktopObservedDefault = $threadReady
        inputDesktopOpenedReadOnly = $context.inputDesktop.handleAvailable
        inputDesktopNameAvailable = $context.inputDesktop.nameAvailable
        inputDesktopObservedDefault = $inputReady
        inputDesktopObservedWinlogon = $context.inputDesktop.nameAvailable -and $context.inputDesktop.nameClass -ceq 'Winlogon'
        inputDesktopReadAccessDenied = -not $context.inputDesktop.handleAvailable -and $context.inputDesktop.nativeError -eq 5
        getCursorPosSucceeded = $context.getCursorPosSucceeded
        nativeTrayInputPrerequisitesObserved = $expectedSessions -and $contextReady
        activePhysicalConsoleSessionAvailable = $context.activeConsoleSessionAvailable
        activePhysicalConsoleSessionIsSession1 = $context.activeConsoleSessionEqualsExpectedGuiSession
        physicalConsoleAlignedInputPrerequisitesObserved = $expectedSessions -and $contextReady -and $context.activeConsoleSessionEqualsExpectedGuiSession
        successfulTrayMenuOrInputGuaranteed = $false
    }
    $stage = 'confirm-fresh-original-gui-still-live'
    if (-not [OrchesterInputDesktopReadiness20261113.Native]::IsLive($freshGuiHandle, [ref]$nativeError)) {
        $result.failedNativeOperation = 'WaitForSingleObject-after'
        $result.failedNativeError = $nativeError
        throw [InvalidOperationException]::new('original-gui-exited-during-observation')
    }
    $result.freshOriginalGuiLiveThroughObservation = $true
    $result.observedAtUtc = [DateTime]::UtcNow.ToString('o')
    $result.status = 'native-input-desktop-readiness-observed'
    $exitCode = 0
} catch {
    $result.errorStage = $stage
    $result.errorKind = $_.Exception.GetType().FullName
    $result.observedAtUtc = [DateTime]::UtcNow.ToString('o')
} finally {
    if ($null -ne $imageStream) { $imageStream.Dispose() }
    if ($nativeLoaded -and $freshGuiHandle -ne [IntPtr]::Zero) {
        $closeNativeError = [int]0
        $result.freshOriginalGuiHandleReleased = [OrchesterInputDesktopReadiness20261113.Native]::ReleaseGui($freshGuiHandle, [ref]$closeNativeError)
        if (-not $result.freshOriginalGuiHandleReleased) {
            $result.failedNativeOperation = 'CloseHandle-owned-gui-query-handle'
            $result.failedNativeError = $closeNativeError
            $result.status = 'native-input-desktop-readiness-observation-failed'
            $exitCode = 1
        }
    }
}

if ($outputValidated) {
    try { Write-OwnedJson $result (Join-Path $resultDirectory $resultName) }
    catch {
        $result.status = 'native-input-desktop-readiness-output-failed'
        $result.errorStage = 'write-owned-output-json'
        $result.errorKind = $_.Exception.GetType().FullName
        $exitCode = 1
    }
}
Write-Output (ConvertTo-Json -InputObject $result -Depth 10 -Compress)
exit $exitCode
