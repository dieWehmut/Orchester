using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;

namespace Orchester.FinalRuntime {
    public sealed class UnexpectedImage {
        public string Basename, SystemRootRelativePath, Sha256;
    }
    public sealed class OwnedProcess {
        internal IntPtr Handle;
        public int Pid, ParentPid, Depth;
        public long Created;
        public string Image, Sha256;
    }

    // Every signal uses an owned process HANDLE. PID, creation time, image path
    // and image hash are rechecked on that same handle immediately beforehand.
    // Process command lines are never read. Unknown descendant images are never
    // signalled, even when their parent belongs to this test.
    public sealed class OwnedProcesses : IDisposable {
        [StructLayout(LayoutKind.Sequential)] struct FileTime { public uint Low, High; }
        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)] struct Entry {
            public uint Size, Usage, Id;
            public UIntPtr DefaultHeap;
            public uint Module, Threads, Parent;
            public int Priority;
            public uint Flags;
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)] public string Name;
        }
        [DllImport("kernel32.dll", SetLastError = true)] static extern IntPtr OpenProcess(uint access, bool inherit, uint pid);
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool CloseHandle(IntPtr h);
        [DllImport("kernel32.dll", SetLastError = true)] static extern IntPtr GetCurrentProcess();
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool DuplicateHandle(IntPtr sourceProcess, IntPtr source, IntPtr targetProcess, out IntPtr copy, uint access, bool inherit, uint options);
        [DllImport("kernel32.dll", SetLastError = true)] static extern uint GetProcessId(IntPtr h);
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool GetProcessTimes(IntPtr h, out FileTime created, out FileTime exited, out FileTime kernel, out FileTime user);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern bool QueryFullProcessImageName(IntPtr h, uint flags, StringBuilder image, ref uint size);
        [DllImport("kernel32.dll", SetLastError = true)] static extern IntPtr CreateToolhelp32Snapshot(uint flags, uint pid);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern bool Process32FirstW(IntPtr snapshot, ref Entry entry);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern bool Process32NextW(IntPtr snapshot, ref Entry entry);
        [DllImport("kernel32.dll", SetLastError = true)] static extern uint WaitForSingleObject(IntPtr h, uint milliseconds);
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool TerminateProcess(IntPtr h, uint exitCode);
        const uint QuerySyncTerminate = 0x00101001;
        readonly Dictionary<string, string> permitted = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        readonly List<OwnedProcess> owned = new List<OwnedProcess>();
        readonly Dictionary<string, UnexpectedImage> unexpectedImages = new Dictionary<string, UnexpectedImage>(StringComparer.OrdinalIgnoreCase);
        public OwnedProcess Root { get; private set; }
        public int UnexpectedDescendants { get; private set; }
        bool passiveWslHostRegistered;
        public int Count { get { return owned.Count; } }
        public UnexpectedImage[] SafeUnexpectedImages { get { return new List<UnexpectedImage>(unexpectedImages.Values).ToArray(); } }

        public static string Hash(string path) {
            using (var f = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read))
            using (var h = SHA256.Create()) return BitConverter.ToString(h.ComputeHash(f)).Replace("-", "").ToLowerInvariant();
        }
        public void RegisterPassiveWslHostObservation() {
            const string exactImage = @"C:\Program Files\WSL\wslhost.exe";
            const string exactSha256 = "1f214fbb69efcd138260c4aa4e35182bfe5371afe4055b59a3133d8f88f9c3b7";
            string pin;
            if (!permitted.TryGetValue(exactImage, out pin) || !String.Equals(pin, exactSha256, StringComparison.Ordinal))
                throw new InvalidOperationException("passive-wslhost-pin-differs");
            passiveWslHostRegistered = true;
        }
        bool IsPassiveWslHost(OwnedProcess process) {
            return passiveWslHostRegistered &&
                String.Equals(process.Image, @"C:\Program Files\WSL\wslhost.exe", StringComparison.OrdinalIgnoreCase) &&
                String.Equals(process.Sha256, "1f214fbb69efcd138260c4aa4e35182bfe5371afe4055b59a3133d8f88f9c3b7", StringComparison.Ordinal);
        }
        public bool HasLivePassiveWslHost() {
            foreach (var record in owned) if (IsPassiveWslHost(record) && IsAlive(record)) return true;
            return false;
        }
        public OwnedProcesses(string[] permittedImages) {
            foreach (var path in permittedImages) permitted.Add(Path.GetFullPath(path), Hash(path));
        }
        void NoteUnexpected(OwnedProcess child) {
            UnexpectedDescendants++;
            if (unexpectedImages.ContainsKey(child.Image)) return;
            string systemRoot = Path.GetFullPath(Environment.GetEnvironmentVariable("SystemRoot")).TrimEnd('\\') + "\\";
            var item = new UnexpectedImage { Basename = Path.GetFileName(child.Image) };
            if (child.Image.StartsWith(systemRoot, StringComparison.OrdinalIgnoreCase))
                item.SystemRootRelativePath = child.Image.Substring(systemRoot.Length).Replace('\\', '/');
            try { item.Sha256 = Hash(child.Image); } catch (IOException) { } catch (UnauthorizedAccessException) { }
            unexpectedImages.Add(child.Image, item);
        }
        public static string LastIdentityDiagnostic { get; private set; }
        public OwnedProcess[] Identities { get { return owned.ToArray(); } }
        static OwnedProcess Describe(IntPtr h) {
            FileTime c, e, k, u;
            var path = new StringBuilder(32768); uint length = 32768;
            uint pid = GetProcessId(h);
            if (pid == 0) { LastIdentityDiagnostic = "GetProcessId:" + Marshal.GetLastWin32Error(); throw new InvalidOperationException("owned-identity-unreadable"); }
            if (!GetProcessTimes(h, out c, out e, out k, out u)) { LastIdentityDiagnostic = "GetProcessTimes:" + Marshal.GetLastWin32Error(); throw new InvalidOperationException("owned-identity-unreadable"); }
            if (!QueryFullProcessImageName(h, 0, path, ref length)) { LastIdentityDiagnostic = "QueryFullProcessImageName:" + Marshal.GetLastWin32Error(); throw new InvalidOperationException("owned-identity-unreadable"); }
            return new OwnedProcess { Pid = (int)pid, Created = ((long)c.High << 32) | c.Low, Image = path.ToString() };
        }
        static bool Same(OwnedProcess a, OwnedProcess b) {
            return a.Pid == b.Pid && a.Created == b.Created && String.Equals(a.Image, b.Image, StringComparison.OrdinalIgnoreCase);
        }
        static bool TryDescribeAlive(IntPtr handle, out OwnedProcess current) {
            current = null;
            uint wait = WaitForSingleObject(handle, 0);
            if (wait == 0) return false;
            if (wait != 258) throw new InvalidOperationException("owned-wait-failed");
            try { current = Describe(handle); return true; }
            catch (InvalidOperationException error) {
                // A short-lived child may exit between waiting and querying.
                // Ignore only an exit proven on that same original HANDLE.
                if (error.Message == "owned-identity-unreadable" && WaitForSingleObject(handle, 5000) == 0) return false;
                throw;
            }
        }
        public bool IsAlive(OwnedProcess process) {
            OwnedProcess current;
            if (!TryDescribeAlive(process.Handle, out current)) return false;
            if (!Same(process, current)) throw new InvalidOperationException("owned-identity-mismatch");
            return true;
        }
        public void CaptureRoot(Process process, string expectedImage, string expectedSha256) {
            if (Root != null) throw new InvalidOperationException("owned-root-already-set");
            IntPtr copy;
            if (!DuplicateHandle(GetCurrentProcess(), process.Handle, GetCurrentProcess(), out copy, QuerySyncTerminate, false, 0))
                throw new InvalidOperationException("owned-root-handle-failed");
            try {
                var record = Describe(copy);
                record.Handle = copy; record.Depth = 0;
                record.Sha256 = Hash(record.Image);
                if (record.Pid != process.Id || !String.Equals(record.Image, expectedImage, StringComparison.OrdinalIgnoreCase) ||
                    !String.Equals(record.Sha256, expectedSha256, StringComparison.Ordinal))
                    throw new InvalidOperationException("owned-root-image-mismatch");
                Root = record; owned.Add(record); copy = IntPtr.Zero;
            } finally { if (copy != IntPtr.Zero) CloseHandle(copy); }
        }
        public void CaptureNativeRoot(IntPtr originalHandle, int expectedPid, long expectedCreated, string expectedImage, string expectedSha256) {
            if (Root != null) throw new InvalidOperationException("owned-root-already-set");
            IntPtr copy;
            if (!DuplicateHandle(GetCurrentProcess(), originalHandle, GetCurrentProcess(), out copy, QuerySyncTerminate, false, 0)) throw new InvalidOperationException("owned-root-handle-failed");
            try {
                var record = Describe(copy);
                record.Handle = copy; record.Depth = 0; record.Sha256 = Hash(record.Image);
                if (record.Pid != expectedPid || record.Created != expectedCreated || !String.Equals(record.Image, expectedImage, StringComparison.OrdinalIgnoreCase) || !String.Equals(record.Sha256, expectedSha256, StringComparison.Ordinal)) throw new InvalidOperationException("owned-root-image-mismatch");
                Root = record; owned.Add(record); copy = IntPtr.Zero;
            } finally { if (copy != IntPtr.Zero) CloseHandle(copy); }
        }
        public void CaptureAllowedDescendants() {
            var entries = new List<Entry>();
            // Reject processes born after the snapshot begins. A PID reused
            // between the snapshot and OpenProcess cannot become an owned
            // child merely because its new executable is on the allowlist.
            // Children created during capture are eligible on the next pass.
            long snapshotStarted = DateTime.UtcNow.ToFileTimeUtc();
            var snapshot = CreateToolhelp32Snapshot(2, 0);
            if (snapshot == new IntPtr(-1)) throw new InvalidOperationException("owned-snapshot-failed");
            try {
                var entry = new Entry { Size = (uint)Marshal.SizeOf(typeof(Entry)) };
                if (!Process32FirstW(snapshot, ref entry)) throw new InvalidOperationException("owned-snapshot-empty");
                do { entries.Add(entry); entry.Size = (uint)Marshal.SizeOf(typeof(Entry)); } while (Process32NextW(snapshot, ref entry));
                if (Marshal.GetLastWin32Error() != 18) throw new InvalidOperationException("owned-snapshot-incomplete");
            } finally { CloseHandle(snapshot); }
            bool changed;
            do {
                changed = false;
                foreach (var entry in entries) {
                    OwnedProcess parent = null;
                    foreach (var record in owned) if (record.Pid == (int)entry.Parent && IsAlive(record)) { parent = record; break; }
                    if (parent == null) continue;
                    var h = OpenProcess(QuerySyncTerminate, false, entry.Id);
                    if (h == IntPtr.Zero) {
                        if (Marshal.GetLastWin32Error() == 87) continue;
                        throw new InvalidOperationException("owned-descendant-handle-denied");
                    }
                    try {
                        OwnedProcess child;
                        if (!TryDescribeAlive(h, out child)) continue;
                        if (child.Pid != (int)entry.Id || child.Created < parent.Created ||
                            child.Created >= snapshotStarted || !IsAlive(parent)) continue;
                        bool captured = false;
                        foreach (var record in owned) if (Same(record, child)) { captured = true; break; }
                        if (captured) continue;
                        string expectedHash;
                        if (!permitted.TryGetValue(child.Image, out expectedHash)) { NoteUnexpected(child); continue; }
                        child.Sha256 = Hash(child.Image);
                        if (!String.Equals(child.Sha256, expectedHash, StringComparison.Ordinal)) {
                            NoteUnexpected(child); continue;
                        }
                        child.Handle = h; child.ParentPid = parent.Pid; child.Depth = parent.Depth + 1;
                        owned.Add(child); h = IntPtr.Zero; changed = true;
                    } finally { if (h != IntPtr.Zero) CloseHandle(h); }
                }
            } while (changed);
        }
        public OwnedProcess[] DirectChildren(OwnedProcess parent, string image) {
            var result = new List<OwnedProcess>();
            foreach (var record in owned) if (record.ParentPid == parent.Pid && record.Created >= parent.Created &&
                String.Equals(record.Image, image, StringComparison.OrdinalIgnoreCase) && IsAlive(record)) result.Add(record);
            return result.ToArray();
        }
        public bool AllExited() {
            foreach (var record in owned) if (IsAlive(record)) return false;
            return true;
        }
        public string[] TerminateRemaining() {
            var outcomes = new List<string>();
            var ordered = new List<OwnedProcess>(owned);
            ordered.Sort((a, b) => b.Depth.CompareTo(a.Depth));
            foreach (var record in ordered) {
                // The exact pinned system broker is observed and awaited, never signalled.
                if (IsPassiveWslHost(record)) continue;
                if (!IsAlive(record)) continue;
                OwnedProcess current;
                if (!TryDescribeAlive(record.Handle, out current)) continue;
                if (!Same(record, current)) { outcomes.Add("identity-mismatch"); continue; }
                if (!String.Equals(Hash(current.Image), record.Sha256, StringComparison.Ordinal)) {
                    outcomes.Add("image-hash-mismatch"); continue;
                }
                if (!TerminateProcess(record.Handle, 1)) { outcomes.Add("terminate-failed"); continue; }
                outcomes.Add(WaitForSingleObject(record.Handle, 5000) == 0 ? "terminated-exact-owned-handle" : "exit-timeout");
            }
            return outcomes.ToArray();
        }
        public void Dispose() {
            foreach (var record in owned) { if (record.Handle != IntPtr.Zero) CloseHandle(record.Handle); record.Handle = IntPtr.Zero; }
        }
    }
}
