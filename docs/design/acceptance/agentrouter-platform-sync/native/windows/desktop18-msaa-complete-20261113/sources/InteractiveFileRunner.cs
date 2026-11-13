using System;
using System.ComponentModel;
using System.Globalization;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;

public sealed class InteractiveFileResult
{
    public int ChildPid;
    public string ChildCreatedAt;
    public bool TimedOut;
    public bool ProcessExited;
    public bool TerminationAttempted;
    public bool TerminationSucceeded;
    public int ExitCode;
    public int TerminationError;
    public int WaitError;
    public bool AuditWriteFailed;
    public string StdoutEncoding;
    public string StderrEncoding;
    public string StdoutErrorType;
    public string StderrErrorType;
}

// Uses files rather than pipes. Descendants may inherit the file handles, but
// their continued existence cannot hold up this proxy's wait or output snapshot.
public static class InteractiveFileRunner
{
    private const uint GENERIC_READ = 0x80000000;
    private const uint GENERIC_WRITE = 0x40000000;
    private const uint SHARE_ALL = 7;
    private const uint CREATE_ALWAYS = 2;
    private const uint OPEN_EXISTING = 3;
    private const uint FILE_ATTRIBUTE_NORMAL = 0x80;
    private const uint STARTF_USESHOWWINDOW = 1;
    private const uint STARTF_USESTDHANDLES = 0x100;
    private const uint EXTENDED_STARTUPINFO_PRESENT = 0x80000;
    private const uint CREATE_NO_WINDOW = 0x8000000;
    private const uint WAIT_OBJECT_0 = 0;
    private const uint WAIT_TIMEOUT = 258;
    private static readonly IntPtr InvalidHandle = new IntPtr(-1);

    [StructLayout(LayoutKind.Sequential)]
    private struct SecurityAttributes { public int Length; public IntPtr Descriptor; public int Inherit; }
    [StructLayout(LayoutKind.Sequential)]
    private struct StartupInfo {
        public int Size; public IntPtr Reserved; public IntPtr Desktop; public IntPtr Title;
        public uint X; public uint Y; public uint XSize; public uint YSize;
        public uint XCount; public uint YCount; public uint Fill; public uint Flags;
        public ushort ShowWindow; public ushort ReservedSize; public IntPtr Reserved2;
        public IntPtr StdInput; public IntPtr StdOutput; public IntPtr StdError;
    }
    [StructLayout(LayoutKind.Sequential)]
    private struct StartupInfoEx { public StartupInfo Startup; public IntPtr Attributes; }
    [StructLayout(LayoutKind.Sequential)]
    private struct ProcessInformation { public IntPtr Process; public IntPtr Thread; public uint Pid; public uint Tid; }
    [StructLayout(LayoutKind.Sequential)]
    private struct NativeFileTime { public uint Low; public uint High; }

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, ExactSpelling = true, SetLastError = true)]
    private static extern IntPtr CreateFileW(string name, uint access, uint share, ref SecurityAttributes security, uint creation, uint flags, IntPtr template);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool InitializeProcThreadAttributeList(IntPtr list, int count, uint flags, ref IntPtr size);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool UpdateProcThreadAttribute(IntPtr list, uint flags, IntPtr attribute, IntPtr value, IntPtr size, IntPtr previous, IntPtr returnedSize);
    [DllImport("kernel32.dll")]
    private static extern void DeleteProcThreadAttributeList(IntPtr list);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, ExactSpelling = true, SetLastError = true)]
    private static extern bool CreateProcessW(string application, StringBuilder commandLine, IntPtr processSecurity, IntPtr threadSecurity, bool inheritHandles, uint flags, IntPtr environment, string currentDirectory, ref StartupInfoEx startup, out ProcessInformation process);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern uint WaitForSingleObject(IntPtr handle, uint milliseconds);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool TerminateProcess(IntPtr handle, uint exitCode);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool GetExitCodeProcess(IntPtr handle, out uint code);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool GetProcessTimes(IntPtr handle, out NativeFileTime creation, out NativeFileTime exit, out NativeFileTime kernel, out NativeFileTime user);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool CloseHandle(IntPtr handle);

    private static IntPtr OpenInheritable(string path, uint access, uint creation)
    {
        var security = new SecurityAttributes { Length = Marshal.SizeOf(typeof(SecurityAttributes)), Inherit = 1 };
        IntPtr handle = CreateFileW(path, access, SHARE_ALL, ref security, creation, FILE_ATTRIBUTE_NORMAL, IntPtr.Zero);
        if (handle == InvalidHandle) throw new Win32Exception(Marshal.GetLastWin32Error());
        return handle;
    }

    private static string Json(string value)
    {
        return "\"" + value.Replace("\\", "\\\\").Replace("\"", "\\\"").Replace("\r", "\\r").Replace("\n", "\\n") + "\"";
    }

    private static void Audit(string path, string executable, int ownerPid, InteractiveFileResult result, string phase)
    {
        string data = "{\"ownerPid\":" + ownerPid + ",\"childPid\":" + result.ChildPid +
            ",\"image\":" + Json(executable) + ",\"createdAt\":" + Json(result.ChildCreatedAt) +
            ",\"phase\":" + Json(phase) + ",\"observedAt\":" + Json(DateTime.UtcNow.ToString("o", CultureInfo.InvariantCulture)) +
            ",\"timedOut\":" + (result.TimedOut ? "true" : "false") +
            ",\"processExited\":" + (result.ProcessExited ? "true" : "false") +
            ",\"terminationAttempted\":" + (result.TerminationAttempted ? "true" : "false") +
            ",\"terminationSucceeded\":" + (result.TerminationSucceeded ? "true" : "false") +
            ",\"terminationError\":" + result.TerminationError + ",\"exitCode\":" + result.ExitCode + "}";
        try {
            string temporary = path + ".tmp";
            File.WriteAllText(temporary, data, new UTF8Encoding(false));
            if (File.Exists(path)) File.Replace(temporary, path, null); else File.Move(temporary, path);
        } catch { result.AuditWriteFailed = true; }
    }

    // A finite-length view: appending descendants cannot extend this read.
    private sealed class SnapshotStream : Stream
    {
        private readonly Stream source;
        private long remaining;
        public SnapshotStream(Stream source, long length) { this.source = source; remaining = length; }
        public override int Read(byte[] buffer, int offset, int count) {
            if (remaining == 0) return 0;
            int read = source.Read(buffer, offset, (int)Math.Min(remaining, count));
            remaining -= read; return read;
        }
        public override bool CanRead { get { return true; } }
        public override bool CanSeek { get { return false; } }
        public override bool CanWrite { get { return false; } }
        public override long Length { get { throw new NotSupportedException(); } }
        public override long Position { get { throw new NotSupportedException(); } set { throw new NotSupportedException(); } }
        public override void Flush() { }
        public override long Seek(long offset, SeekOrigin origin) { throw new NotSupportedException(); }
        public override void SetLength(long length) { throw new NotSupportedException(); }
        public override void Write(byte[] buffer, int offset, int count) { throw new NotSupportedException(); }
    }

    private static string ConvertSnapshot(string raw, string destination, Encoding encoding, long length)
    {
        using (var source = new FileStream(raw, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete))
        using (var reader = new StreamReader(new SnapshotStream(source, length), encoding, true, 4096))
        using (var writer = new StreamWriter(destination, false, new UTF8Encoding(false))) {
            var buffer = new char[4096]; int count;
            while ((count = reader.Read(buffer, 0, buffer.Length)) != 0) writer.Write(buffer, 0, count);
            return reader.CurrentEncoding.WebName;
        }
    }

    private static string Snapshot(string raw, string destination, int fallbackCodePage)
    {
        long length;
        using (var source = new FileStream(raw, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete)) length = source.Length;
        try {
            return ConvertSnapshot(raw, destination, new UTF8Encoding(false, true), length);
        } catch (DecoderFallbackException) {
            Encoding fallback = Encoding.GetEncoding(fallbackCodePage);
            return ConvertSnapshot(raw, destination, fallback, length);
        }
    }

    private static ProcessInformation CreateOwned(string executable, string arguments, string directory, string stdoutPath, string stderrPath) { return CreateOwned(executable, arguments, directory, stdoutPath, stderrPath, IntPtr.Zero, false); }
    private static ProcessInformation CreateOwned(string executable, string arguments, string directory, string stdoutPath, string stderrPath, IntPtr providedInput, bool suspended)
    {
        IntPtr input = IntPtr.Zero, output = IntPtr.Zero, error = IntPtr.Zero;
        IntPtr attributeList = IntPtr.Zero, handleList = IntPtr.Zero;
        bool attributeInitialized = false;
        var process = new ProcessInformation();
        try {
            input = providedInput != IntPtr.Zero ? providedInput : OpenInheritable("NUL", GENERIC_READ, OPEN_EXISTING);
            output = OpenInheritable(stdoutPath, GENERIC_WRITE, CREATE_ALWAYS);
            error = OpenInheritable(stderrPath, GENERIC_WRITE, CREATE_ALWAYS);
            IntPtr attributeBytes = IntPtr.Zero;
            InitializeProcThreadAttributeList(IntPtr.Zero, 1, 0, ref attributeBytes);
            if (attributeBytes == IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error());
            attributeList = Marshal.AllocHGlobal(attributeBytes);
            if (!InitializeProcThreadAttributeList(attributeList, 1, 0, ref attributeBytes)) throw new Win32Exception(Marshal.GetLastWin32Error());
            attributeInitialized = true;
            handleList = Marshal.AllocHGlobal(IntPtr.Size * 3);
            Marshal.WriteIntPtr(handleList, 0, input);
            Marshal.WriteIntPtr(handleList, IntPtr.Size, output);
            Marshal.WriteIntPtr(handleList, IntPtr.Size * 2, error);
            if (!UpdateProcThreadAttribute(attributeList, 0, new IntPtr(0x20002), handleList, new IntPtr(IntPtr.Size * 3), IntPtr.Zero, IntPtr.Zero)) throw new Win32Exception(Marshal.GetLastWin32Error());
            var startup = new StartupInfoEx();
            startup.Startup.Size = Marshal.SizeOf(typeof(StartupInfoEx));
            startup.Startup.Flags = STARTF_USESTDHANDLES | STARTF_USESHOWWINDOW;
            startup.Startup.StdInput = input; startup.Startup.StdOutput = output; startup.Startup.StdError = error;
            startup.Attributes = attributeList;
            var commandLine = new StringBuilder("\"" + executable + "\" " + arguments);
            if (!CreateProcessW(executable, commandLine, IntPtr.Zero, IntPtr.Zero, true,
                EXTENDED_STARTUPINFO_PRESENT | CREATE_NO_WINDOW | (suspended ? 4u : 0u), IntPtr.Zero, directory, ref startup, out process)) throw new Win32Exception(Marshal.GetLastWin32Error());
            return process;
        } finally {
            if (attributeInitialized) DeleteProcThreadAttributeList(attributeList);
            if (attributeList != IntPtr.Zero) Marshal.FreeHGlobal(attributeList);
            if (handleList != IntPtr.Zero) Marshal.FreeHGlobal(handleList);
            if (input != IntPtr.Zero && input != InvalidHandle) CloseHandle(input);
            if (output != IntPtr.Zero && output != InvalidHandle) CloseHandle(output);
            if (error != IntPtr.Zero && error != InvalidHandle) CloseHandle(error);
        }
    }

    private static InteractiveFileResult Identity(ProcessInformation process)
    {
        var result = new InteractiveFileResult { ChildPid = checked((int)process.Pid), ExitCode = -1, ChildCreatedAt = "" };
        NativeFileTime created, exited, kernel, user;
        if (GetProcessTimes(process.Process, out created, out exited, out kernel, out user)) {
            long fileTime = ((long)created.High << 32) | created.Low;
            result.ChildCreatedAt = DateTime.FromFileTimeUtc(fileTime).ToString("o", CultureInfo.InvariantCulture);
        }
        return result;
    }

    public static InteractiveFileResult StartDetached(string executable, string arguments, string directory,
        string stdoutPath, string stderrPath, string auditPath, int ownerPid)
    {
        ProcessInformation process = CreateOwned(executable, arguments, directory, stdoutPath, stderrPath);
        try {
            InteractiveFileResult result = Identity(process);
            Audit(auditPath, executable, ownerPid, result, "detached");
            return result;
        } finally {
            CloseHandle(process.Thread);
            CloseHandle(process.Process);
        }
    }

    public static InteractiveFileResult Run(string executable, string arguments, string directory,
        string stdoutRaw, string stderrRaw, string stdoutText, string stderrText,
        string auditPath, int ownerPid, int timeoutSeconds, int fallbackCodePage)
    {
        ProcessInformation process = CreateOwned(executable, arguments, directory, stdoutRaw, stderrRaw);
        InteractiveFileResult result = Identity(process);
        try {
            Audit(auditPath, executable, ownerPid, result, "running");
            uint wait = WaitForSingleObject(process.Process, checked((uint)timeoutSeconds * 1000));
            result.TimedOut = wait == WAIT_TIMEOUT;
            if (wait != WAIT_OBJECT_0) {
                if (wait != WAIT_TIMEOUT) result.WaitError = Marshal.GetLastWin32Error();
                // This exact owned process handle, never a PID lookup or process tree.
                result.TerminationAttempted = true;
                result.TerminationSucceeded = TerminateProcess(process.Process, 124);
                if (!result.TerminationSucceeded) result.TerminationError = Marshal.GetLastWin32Error();
                wait = WaitForSingleObject(process.Process, 5000);
            }
            result.ProcessExited = wait == WAIT_OBJECT_0;
            uint exitCode;
            if (result.ProcessExited && GetExitCodeProcess(process.Process, out exitCode)) result.ExitCode = unchecked((int)exitCode);
            Audit(auditPath, executable, ownerPid, result, "bounded_wait_complete");
            try { result.StdoutEncoding = Snapshot(stdoutRaw, stdoutText, fallbackCodePage); }
            catch (Exception exception) { result.StdoutErrorType = exception.GetType().FullName; }
            try { result.StderrEncoding = Snapshot(stderrRaw, stderrText, fallbackCodePage); }
            catch (Exception exception) { result.StderrErrorType = exception.GetType().FullName; }
            Audit(auditPath, executable, ownerPid, result, "output_snapshot_complete");
            return result;
        } finally {
            CloseHandle(process.Thread);
            CloseHandle(process.Process);
        }
    }
    [DllImport("kernel32.dll", SetLastError = true)] private static extern uint ResumeThread(IntPtr handle);
    [DllImport("kernel32.dll", SetLastError = true)] private static extern bool CreatePipe(out IntPtr read, out IntPtr write, ref SecurityAttributes security, uint size);
    [DllImport("kernel32.dll", SetLastError = true)] private static extern bool SetHandleInformation(IntPtr handle, uint mask, uint flags);
    [DllImport("kernel32.dll", SetLastError = true)] private static extern bool WriteFile(IntPtr handle, byte[] buffer, uint count, out uint written, IntPtr overlapped);
    public sealed class InteractiveProcess : IDisposable {
        public IntPtr RawHandle { get; private set; }
        IntPtr inputWriter, primaryThread;
        public bool HasResumed { get; private set; }
        public int Id { get; private set; }
        public long Created { get; private set; }
        internal InteractiveProcess(IntPtr handle, IntPtr writer, IntPtr thread, int pid, long created) { RawHandle=handle; inputWriter=writer;primaryThread=thread; Id=pid; Created=created; }
        public bool HasExited { get { uint wait=WaitForSingleObject(RawHandle,0); if(wait==0) return true; if(wait==258) return false; throw new InvalidOperationException("interactive-wait-failed"); } }
        public int ExitCode { get { uint value; if(!GetExitCodeProcess(RawHandle,out value)) throw new InvalidOperationException("interactive-exit-unreadable"); return unchecked((int)value); } }
        public bool WaitForExit(int milliseconds) { uint wait=WaitForSingleObject(RawHandle,checked((uint)milliseconds)); if(wait==0) return true; if(wait==258) return false; throw new InvalidOperationException("interactive-wait-failed"); }
        public void WriteLine(string line) { byte[] bytes=new UTF8Encoding(false).GetBytes(line+"\r\n"); if(bytes.Length>1024) throw new InvalidOperationException("interactive-input-bound"); uint written; if(!WriteFile(inputWriter,bytes,(uint)bytes.Length,out written,IntPtr.Zero) || written!=bytes.Length) throw new InvalidOperationException("interactive-input-failed"); }
        public void Resume() { if(HasResumed || ResumeThread(primaryThread)==0xffffffff) throw new InvalidOperationException("interactive-resume-failed"); HasResumed=true;CloseHandle(primaryThread);primaryThread=IntPtr.Zero; }
        public bool TerminateSuspended() { if(HasResumed) throw new InvalidOperationException("interactive-already-resumed"); if(HasExited)return true; return TerminateProcess(RawHandle,124) && WaitForExit(5000); }
        public void Dispose() { if(primaryThread!=IntPtr.Zero){CloseHandle(primaryThread);primaryThread=IntPtr.Zero;} if(inputWriter!=IntPtr.Zero) {CloseHandle(inputWriter);inputWriter=IntPtr.Zero;} if(RawHandle!=IntPtr.Zero){CloseHandle(RawHandle);RawHandle=IntPtr.Zero;} }
    }
    public static InteractiveProcess StartInteractive(string executable, string arguments, string directory, string stdoutPath, string stderrPath) {
        IntPtr read=IntPtr.Zero, write=IntPtr.Zero; bool transferred=false;
        var security=new SecurityAttributes{Length=Marshal.SizeOf(typeof(SecurityAttributes)),Inherit=1};
        if(!CreatePipe(out read,out write,ref security,4096)) throw new InvalidOperationException("interactive-pipe-failed");
        try {
            if(!SetHandleInformation(write,1,0)) throw new InvalidOperationException("interactive-writer-noninherit-failed");
            transferred=true;
            var process=CreateOwned(executable,arguments,directory,stdoutPath,stderrPath,read,true);
            NativeFileTime c,e,k,u;
            if(!GetProcessTimes(process.Process,out c,out e,out k,out u)){var pending=new InteractiveProcess(process.Process,write,process.Thread,checked((int)process.Pid),0);write=IntPtr.Zero;return pending;}
            var result=new InteractiveProcess(process.Process,write,process.Thread,checked((int)process.Pid),((long)c.High<<32)|c.Low);write=IntPtr.Zero;return result;
        } finally { if(!transferred && read!=IntPtr.Zero)CloseHandle(read);if(write!=IntPtr.Zero)CloseHandle(write); }
    }

}
