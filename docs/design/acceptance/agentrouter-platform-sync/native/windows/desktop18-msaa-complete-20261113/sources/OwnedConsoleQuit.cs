using System;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;

namespace Orchester.FinalRuntime {
    public sealed class ConsoleQuitResult {
        public int TargetPid, ConsoleParticipants;
        public string TargetCreatedAt, TargetImageSha256;
        public bool IdentityMatched, ConsoleMembersMatched, CommandWritten, OriginalHandleExited;
    }

    // Sends the existing CLI's ordinary quit command to its own console only.
    // It never signals, terminates, or closes another process or console window.
    public static class OwnedConsoleQuit {
        [StructLayout(LayoutKind.Sequential)] struct FileTime { public uint Low, High; }
        [StructLayout(LayoutKind.Explicit, CharSet = CharSet.Unicode, Size = 20)] struct InputRecord {
            [FieldOffset(0)] public ushort EventType;
            [FieldOffset(4)] public int KeyDown;
            [FieldOffset(8)] public ushort Repeat;
            [FieldOffset(10)] public ushort VirtualKey;
            [FieldOffset(12)] public ushort Scan;
            [FieldOffset(14)] public char Character;
            [FieldOffset(16)] public uint Control;
        }
        [DllImport("kernel32.dll", SetLastError = true)] static extern IntPtr OpenProcess(uint access, bool inherit, uint pid);
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool CloseHandle(IntPtr handle);
        [DllImport("kernel32.dll", SetLastError = true)] static extern uint GetProcessId(IntPtr handle);
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool GetProcessTimes(IntPtr handle, out FileTime created, out FileTime exited, out FileTime kernel, out FileTime user);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern bool QueryFullProcessImageName(IntPtr handle, uint flags, StringBuilder image, ref uint size);
        [DllImport("kernel32.dll", SetLastError = true)] static extern uint WaitForSingleObject(IntPtr handle, uint milliseconds);
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool FreeConsole();
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool AttachConsole(uint pid);
        [DllImport("kernel32.dll", SetLastError = true)] static extern uint GetConsoleProcessList([Out] uint[] processes, uint count);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern IntPtr CreateFile(string path, uint access, uint share, IntPtr security, uint disposition, uint flags, IntPtr template);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern bool WriteConsoleInputW(IntPtr input, InputRecord[] records, uint count, out uint written);

        static string Hash(string path) {
            using (var file = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read))
            using (var hash = SHA256.Create()) return BitConverter.ToString(hash.ComputeHash(file)).Replace("-", "").ToLowerInvariant();
        }
        static void Match(IntPtr handle, int pid, long created, string image, string sha256) {
            FileTime c, e, k, u;
            var path = new StringBuilder(32768); uint length = 32768;
            if (WaitForSingleObject(handle, 0) != 258 || GetProcessId(handle) != (uint)pid ||
                !GetProcessTimes(handle, out c, out e, out k, out u) ||
                !QueryFullProcessImageName(handle, 0, path, ref length) ||
                (((long)c.High << 32) | c.Low) != created ||
                !String.Equals(path.ToString(), Path.GetFullPath(image), StringComparison.OrdinalIgnoreCase) ||
                !String.Equals(Hash(path.ToString()), sha256, StringComparison.Ordinal))
                throw new InvalidOperationException("console-process-identity-mismatch");
        }
        public static ConsoleQuitResult Run(int targetPid, string targetCreatedAt, string targetImage, string targetSha256,
            int parentPid, string parentCreatedAt, string parentImage, string parentSha256, bool execute) {
            var target = OpenProcess(0x00101000, false, (uint)targetPid);
            var parent = OpenProcess(0x00101000, false, (uint)parentPid);
            if (target == IntPtr.Zero || parent == IntPtr.Zero) {
                if (target != IntPtr.Zero) CloseHandle(target);
                if (parent != IntPtr.Zero) CloseHandle(parent);
                throw new InvalidOperationException("console-process-handle-unavailable");
            }
            bool attached = false; IntPtr input = new IntPtr(-1);
            try {
                long targetCreated = DateTime.Parse(targetCreatedAt).ToUniversalTime().ToFileTimeUtc();
                long parentCreated = DateTime.Parse(parentCreatedAt).ToUniversalTime().ToFileTimeUtc();
                Match(target, targetPid, targetCreated, targetImage, targetSha256);
                Match(parent, parentPid, parentCreated, parentImage, parentSha256);
                var result = new ConsoleQuitResult { TargetPid = targetPid, TargetCreatedAt = targetCreatedAt,
                    TargetImageSha256 = targetSha256, IdentityMatched = true };
                FreeConsole();
                if (!AttachConsole((uint)targetPid)) throw new InvalidOperationException("console-attach-unavailable");
                attached = true;
                var processes = new uint[512];
                uint count = GetConsoleProcessList(processes, (uint)processes.Length);
                if (count == 0 || count > processes.Length) throw new InvalidOperationException("console-members-unavailable");
                bool foundTarget = false, foundParent = false, foundSelf = false;
                uint self = (uint)Process.GetCurrentProcess().Id;
                for (int i = 0; i < count; i++) {
                    if (processes[i] == (uint)targetPid) foundTarget = true;
                    else if (processes[i] == (uint)parentPid) foundParent = true;
                    else if (processes[i] == self) foundSelf = true;
                    else throw new InvalidOperationException("console-has-unrelated-participants");
                }
                if (!foundTarget || !foundParent || !foundSelf) throw new InvalidOperationException("console-members-differ");
                result.ConsoleParticipants = (int)count; result.ConsoleMembersMatched = true;
                if (!execute) return result;
                Match(target, targetPid, targetCreated, targetImage, targetSha256);
                Match(parent, parentPid, parentCreated, parentImage, parentSha256);
                input = CreateFile("CONIN$", 0x40000000, 3, IntPtr.Zero, 3, 0, IntPtr.Zero);
                if (input == new IntPtr(-1)) throw new InvalidOperationException("console-input-unavailable");
                const string command = "quit\r";
                var records = new InputRecord[command.Length * 2];
                for (int i = 0; i < command.Length; i++) {
                    ushort key = command[i] == '\r' ? (ushort)13 : (ushort)Char.ToUpperInvariant(command[i]);
                    records[i * 2] = new InputRecord { EventType = 1, KeyDown = 1, Repeat = 1, VirtualKey = key, Character = command[i] };
                    records[i * 2 + 1] = records[i * 2]; records[i * 2 + 1].KeyDown = 0;
                }
                uint written;
                if (!WriteConsoleInputW(input, records, (uint)records.Length, out written) || written != records.Length)
                    throw new InvalidOperationException("console-command-write-incomplete");
                result.CommandWritten = true;
                result.OriginalHandleExited = WaitForSingleObject(target, 30000) == 0;
                return result;
            } finally {
                if (input != new IntPtr(-1)) CloseHandle(input);
                if (attached) FreeConsole();
                CloseHandle(parent); CloseHandle(target);
            }
        }
    }
}
