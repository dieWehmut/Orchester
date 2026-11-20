using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;

namespace Orchester.FinalDesktop {
    public static class NativeWindows {
        delegate bool Callback(IntPtr hwnd, IntPtr unused);
        [StructLayout(LayoutKind.Sequential)] struct FileTime { public uint Low, High; }
        struct Rect { public int Left, Top, Right, Bottom; }
        struct Point { public int X, Y; }
        [DllImport("kernel32.dll")] static extern uint GetProcessId(IntPtr process);
        [DllImport("kernel32.dll")] static extern uint WaitForSingleObject(IntPtr process, uint milliseconds);
        [DllImport("kernel32.dll")] static extern bool GetProcessTimes(IntPtr process, out FileTime created, out FileTime exited, out FileTime kernel, out FileTime user);
        [DllImport("kernel32.dll", CharSet=CharSet.Unicode)] static extern bool QueryFullProcessImageName(IntPtr process, uint flags, StringBuilder image, ref uint size);
        [DllImport("user32.dll")] static extern bool EnumWindows(Callback callback, IntPtr unused);
        [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint process);
        [DllImport("user32.dll")] static extern bool IsWindow(IntPtr hwnd);
        [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
        [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int max);
        [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr hwnd, out Rect rect);
        [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
        [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr hwnd, ref Point point);
        [DllImport("user32.dll")] static extern uint GetDpiForWindow(IntPtr hwnd);
        [DllImport("user32.dll")] static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
        [DllImport("user32.dll", SetLastError=true)] static extern IntPtr SendMessageTimeout(IntPtr hwnd, uint message, IntPtr wparam, IntPtr lparam, uint flags, uint timeout, out UIntPtr result);
        [DllImport("user32.dll", SetLastError=true)] static extern bool SetWindowPos(IntPtr hwnd, IntPtr previous, int x, int y, int width, int height, uint flags);

        static string Hash(string path) {
            using (var file = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read))
            using (var sha = SHA256.Create()) return BitConverter.ToString(sha.ComputeHash(file)).Replace("-", "").ToLowerInvariant();
        }
        public static void Require(Process process, uint pid, long created, string image, string sha256) {
            // The caller keeps the Process returned by Process.Start alive.
            // All identity queries below use that original process HANDLE.
            var handle = process.Handle;
            var path = new StringBuilder(32768); uint size = 32768;
            FileTime c, e, k, u;
            if (GetProcessId(handle) != pid || WaitForSingleObject(handle, 0) != 258 ||
                !GetProcessTimes(handle, out c, out e, out k, out u) ||
                !QueryFullProcessImageName(handle, 0, path, ref size)) throw new InvalidOperationException("window-process-unavailable");
            long actualCreated = ((long)c.High << 32) | c.Low;
            if (actualCreated != created || !String.Equals(path.ToString(), image, StringComparison.OrdinalIgnoreCase) ||
                !String.Equals(Hash(path.ToString()), sha256, StringComparison.Ordinal)) throw new InvalidOperationException("window-process-identity-mismatch");
        }
        static void RequireWindow(Process process, uint pid, long created, string image, string sha256, IntPtr hwnd) {
            Require(process, pid, created, image, sha256);
            uint owner;
            if (!IsWindow(hwnd) || GetWindowThreadProcessId(hwnd, out owner) == 0 || owner != pid) throw new InvalidOperationException("window-owner-mismatch");
        }
        public static object[] Snapshot(Process process, uint pid, long created, string image, string sha256) {
            Require(process, pid, created, image, sha256);
            var previous = SetThreadDpiAwarenessContext(new IntPtr(-4));
            var result = new List<object>(); Exception failure = null;
            try {
                bool complete = EnumWindows((hwnd, unused) => {
                    try {
                        uint owner; GetWindowThreadProcessId(hwnd, out owner);
                        if (owner != pid) return true;
                        var title = new StringBuilder(256); GetWindowText(hwnd, title, title.Capacity);
                        if (title.ToString() != "Orchester") return true;
                        Rect bounds;
                        if (!GetClientRect(hwnd, out bounds)) throw new InvalidOperationException("window-client-rect-failed");
                        uint dpi = GetDpiForWindow(hwnd);
                        var point = new Point { X = bounds.Right - (int)(69 * dpi / 96), Y = (int)(16 * dpi / 96) };
                        if (!ClientToScreen(hwnd, ref point)) throw new InvalidOperationException("window-client-screen-failed");
                        long packed = (long)(((ulong)(ushort)point.Y << 16) | (ulong)(ushort)point.X);
                        RequireWindow(process, pid, created, image, sha256, hwnd);
                        UIntPtr hit;
                        if (SendMessageTimeout(hwnd, 0x0084, IntPtr.Zero, new IntPtr(packed), 2, 1000, out hit) == IntPtr.Zero) throw new InvalidOperationException("window-hit-test-timeout");
                        result.Add(new { Handle = hwnd.ToInt64(), Title = title.ToString(), Visible = IsWindowVisible(hwnd), Width = bounds.Right, Height = bounds.Bottom, Dpi = dpi, MaximizeHit = (long)hit.ToUInt64() });
                        return true;
                    } catch (Exception error) { failure = error; return false; }
                }, IntPtr.Zero);
                if (failure != null) throw failure;
                if (!complete) throw new InvalidOperationException("window-enumeration-failed");
                Require(process, pid, created, image, sha256);
                return result.ToArray();
            } finally { SetThreadDpiAwarenessContext(previous); }
        }
        public static int Resize(Process process, uint pid, long created, string image, string sha256) {
            Require(process, pid, created, image, sha256);
            var previous = SetThreadDpiAwarenessContext(new IntPtr(-4)); int count = 0; Exception failure = null;
            try {
                bool complete = EnumWindows((hwnd, unused) => {
                    try {
                        uint owner; GetWindowThreadProcessId(hwnd, out owner);
                        if (owner != pid) return true;
                        var title = new StringBuilder(256); GetWindowText(hwnd, title, title.Capacity);
                        if (title.ToString() != "Orchester") return true;
                        uint dpi = GetDpiForWindow(hwnd); Rect client, outer;
                        if (!GetClientRect(hwnd, out client) || !GetWindowRect(hwnd, out outer)) throw new InvalidOperationException("window-rect-failed");
                        int width = (int)(1280 * dpi / 96) + outer.Right - outer.Left - client.Right;
                        int height = (int)(800 * dpi / 96) + outer.Bottom - outer.Top - client.Bottom;
                        RequireWindow(process, pid, created, image, sha256, hwnd);
                        if (!SetWindowPos(hwnd, IntPtr.Zero, 0, 0, width, height, 0x0002 | 0x0004 | 0x0010)) throw new InvalidOperationException("window-resize-failed");
                        count++; return true;
                    } catch (Exception error) { failure = error; return false; }
                }, IntPtr.Zero);
                if (failure != null) throw failure;
                if (!complete) throw new InvalidOperationException("window-enumeration-failed");
                Require(process, pid, created, image, sha256);
                return count;
            } finally { SetThreadDpiAwarenessContext(previous); }
        }
    }
}
