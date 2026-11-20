# Public review template; native execution is disabled. Original source digest is in provenance-safe.json.
if __name__ == "__main__":
    raise RuntimeError("public-review-template-native-execution-disabled")
"""Prepared Windows-only read helpers; importing this module starts nothing."""
from pathlib import Path
import ctypes
import hashlib
import json
import os
import re
import socket
import subprocess
import time
from datetime import datetime, timezone

SOURCE = '71def643cb08b208cc79687f79255c504eb14dc6'
APK_SHA = 'f83fa2a75c712a3d2e1f3f203cf73ce89204fb4ca0bce4d8c2ef0c4fe3596d3c'
CERT_SHA = 'df6df435ccf10a685fb109668da8cdf7b022b481ba346f3143190aeda0c4468e'
PACKAGE = 'dev.orchester.mobile'
NATIVE_PYTHON = r'<USERPROFILE>\AppData\Roaming\uv\python\cpython-3.14.9-windows-x86_64-none\python.exe'
BASELINE_XML = r'<USERPROFILE>\AppData\Local\Temp\Orchester-final20-android-20261111\credentials-before-private-stdout.log'
BASELINE_XML_SHA = 'f708ff5441e06dbe1c542d778ba70a270409bdef14a1fec508d88eeeb63abdbd'
IMAGE_PINS = {
    'worker': '60c9b29843624dd8af6b6a7b26147753be6aeef65098b9a503cc50850afe410f',
    'manager': '204d51f19707da17766bccecce2ceececadab350663a8190420d1377bac1150d',
    'server': 'b4a6b455702684652cccf7b46258b29e653538904359a58fd4931cf3ef286b3f',
    'head': '06af3c5da3c2cef7266822fe653f54371fc746d78db6f4282e1b2338a70970c8',
    'qemu': '161d3bcef3d5a93698a77f292c899d131d3e92e335c7b5b2360bbc5ce364cf4c',
}


class GuardError(RuntimeError):
    def __init__(self, code):
        self.code = code
        super().__init__(code)


def require(condition, code):
    if not condition:
        raise GuardError(code)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def sha_file(path):
    digest = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def pinned_json(path, expected_sha):
    require(re.fullmatch('[a-f0-9]{64}', expected_sha or '') is not None, 'pin-format')
    raw = Path(path).read_bytes()
    require(sha(raw) == expected_sha, 'file-pin')
    def pairs(items):
        result = {}
        for key, value in items:
            require(key not in result, 'duplicate-json-field')
            result[key] = value
        return result
    return json.loads(raw.decode('utf-8-sig'), object_pairs_hook=pairs)


def fresh_receipt(value, maximum_age=300):
    observed = datetime.fromisoformat(value['observedAtUtc'].replace('Z', '+00:00'))
    age = (datetime.now(timezone.utc) - observed).total_seconds()
    require(0 <= age <= maximum_age, 'fresh-receipt-required')


def load_binding(path, expected_sha):
    b = pinned_json(path, expected_sha)
    require(b.get('schemaVersion') == 1 and b.get('sourceRevision') == SOURCE, 'binding-schema')
    require(re.fullmatch('[a-f0-9]{32}', b.get('namespaceId', '')) is not None, 'namespace-format')
    identity = pinned_json(b['identityFile'], b['identitySha256'])
    require(identity.get('status') == 'current-owned-windows-medium-target-verified', 'target-gate-status')
    require(identity.get('schemaVersion') == 1 and identity.get('sourceRevision') == SOURCE, 'target-gate-schema')
    require(identity.get('namespaceId') == b['namespaceId'], 'target-namespace')
    require(identity.get('actualAvdHost') == 'Windows' and identity.get('actualAvdName') == 'Medium_Phone_API_37.0', 'target-avd')
    require(identity.get('serial') == 'emulator-5572' and identity.get('serverPort') == 5041 and identity.get('consolePort') == 5572
            and identity.get('guestAdbPort') == 5573 and identity.get('cdpPort') == 19330, 'target-ports')
    require(identity.get('originalCreatorHandlesHeld') is True and identity.get('qemuQueryAndSynchronizeHandleHeld') is True
            and identity.get('transportIdentityCommandsCompleted') is True, 'target-native-ownership-proof')
    require(identity.get('bindingSha256') == b.get('managerBindingSha256'), 'manager-binding-pin')
    fresh_receipt(identity)
    for key in ('pid', 'createdExact', 'sha256'):
        require(identity['worker'][key] == b['worker'][key], 'worker-gate-binding')
    return b, identity


def write_safe(path, value):
    # Parent namespace is task-owned; no original data is written here.
    with Path(path).open('x', encoding='utf-8') as stream:
        json.dump(value, stream, ensure_ascii=True, indent=2)
        stream.write('\n')


class FT(ctypes.Structure):
    _fields_ = [('low', ctypes.c_uint32), ('high', ctypes.c_uint32)]


class SID_AND_ATTRIBUTES(ctypes.Structure):
    _fields_ = [('sid', ctypes.c_void_p), ('attributes', ctypes.c_uint32)]


class Lease:
    def __init__(self, identity):
        require(os.name == 'nt', 'actual-Windows-required')
        self.identity = dict(identity)
        self.handles = {}
        self.clients = []
        self.k = ctypes.WinDLL('kernel32', use_last_error=True)
        self.a = ctypes.WinDLL('advapi32', use_last_error=True)
        self.ip = ctypes.WinDLL('iphlpapi', use_last_error=True)
        H, U, B = ctypes.c_void_p, ctypes.c_uint32, ctypes.c_int32
        signatures = [
            (self.k, 'OpenProcess', H, [U, B, U]), (self.k, 'CloseHandle', B, [H]),
            (self.k, 'WaitForSingleObject', U, [H, U]),
            (self.k, 'GetExitCodeProcess', B, [H, ctypes.POINTER(U)]),
            (self.k, 'GetProcessTimes', B, [H] + [ctypes.POINTER(FT)] * 4),
            (self.k, 'QueryFullProcessImageNameW', B, [H, U, ctypes.c_wchar_p, ctypes.POINTER(U)]),
            (self.k, 'ProcessIdToSessionId', B, [U, ctypes.POINTER(U)]),
            (self.a, 'OpenProcessToken', B, [H, U, ctypes.POINTER(H)]),
            (self.a, 'GetTokenInformation', B, [H, U, H, U, ctypes.POINTER(U)]),
            (self.a, 'EqualSid', B, [H, H]),
            (self.ip, 'GetExtendedTcpTable', U, [H, ctypes.POINTER(U), B, U, U, U]),
        ]
        for library, name, result, args in signatures:
            fn = getattr(library, name)
            fn.restype, fn.argtypes = result, args
        try:
            for role in ('worker', 'manager', 'server', 'head', 'qemu'):
                item = identity[role]
                require(item['sha256'] == IMAGE_PINS[role], 'native-image-pin')
                handle = self.k.OpenProcess(0x1000 | 0x100000, False, int(item['pid']))
                require(bool(handle), 'native-handle-unreadable')
                self.handles[role] = handle
                self._identity(role, include_image=True)
            caller = self.k.OpenProcess(0x1000 | 0x100000, False, os.getpid())
            require(bool(caller), 'current-observer-native-handle-unreadable')
            self.handles['observer'] = caller
            fields = [FT() for _ in range(4)]
            require(bool(self.k.GetProcessTimes(caller, *[ctypes.byref(v) for v in fields])), 'current-observer-birth-unreadable')
            self.identity['observer'] = {'pid': os.getpid(), 'createdExact': str(fields[0].high << 32 | fields[0].low),
                                         'image': NATIVE_PYTHON, 'sha256': IMAGE_PINS['manager'],
                                         'sessionId': self.identity['worker']['sessionId']}
            self._identity('observer', include_image=True)
            self.guard()
        except BaseException:
            self.close()
            raise

    def _identity(self, role, include_image=False):
        item, h = self.identity[role], self.handles[role]
        require(self.k.WaitForSingleObject(h, 0) == 258, 'held-native-process-not-live')
        values = [FT() for _ in range(4)]
        require(bool(self.k.GetProcessTimes(h, *[ctypes.byref(v) for v in values])), 'held-native-birth-unreadable')
        created = values[0].high << 32 | values[0].low
        require(str(created) == str(item['createdExact']), 'held-native-birth-differs')
        session = ctypes.c_uint32()
        require(bool(self.k.ProcessIdToSessionId(int(item['pid']), ctypes.byref(session))), 'native-session-unreadable')
        require(session.value == int(self.identity['worker']['sessionId']), 'native-session-differs')
        if include_image:
            size = ctypes.c_uint32(32768)
            buffer = ctypes.create_unicode_buffer(size.value)
            require(bool(self.k.QueryFullProcessImageNameW(h, 0, buffer, ctypes.byref(size))), 'held-native-image-unreadable')
            require(os.path.normcase(buffer.value) == os.path.normcase(item['image']), 'held-native-image-differs')
            require(sha_file(buffer.value) == item['sha256'], 'held-native-image-sha-differs')

    def _same_user(self, role):
        tokens, buffers = [], []
        try:
            for h in (self.handles['worker'], self.handles[role]):
                token = ctypes.c_void_p()
                require(bool(self.a.OpenProcessToken(h, 8, ctypes.byref(token))), 'native-user-token-unreadable')
                tokens.append(token)
                size = ctypes.c_uint32()
                self.a.GetTokenInformation(token, 1, None, 0, ctypes.byref(size))
                require(size.value > 0, 'native-user-size-unreadable')
                buf = ctypes.create_string_buffer(size.value)
                require(bool(self.a.GetTokenInformation(token, 1, buf, size.value, ctypes.byref(size))), 'native-user-unreadable')
                buffers.append(buf)
            sid1, sid2 = [ctypes.cast(b, ctypes.POINTER(SID_AND_ATTRIBUTES)).contents.sid for b in buffers]
            return bool(self.a.EqualSid(sid1, sid2))
        finally:
            for token in tokens:
                self.k.CloseHandle(token)

    def tcp_rows(self):
        import struct
        result = []
        for family, row_size in ((socket.AF_INET, 24), (socket.AF_INET6, 56)):
            size = ctypes.c_uint32()
            rc = self.ip.GetExtendedTcpTable(None, ctypes.byref(size), False, family, 5, 0)
            require(rc in (0, 122) and 4 <= size.value <= 16 * 1024 * 1024, 'native-tcp-size-unreadable')
            for _ in range(4):
                table = ctypes.create_string_buffer(size.value)
                rc = self.ip.GetExtendedTcpTable(table, ctypes.byref(size), False, family, 5, 0)
                if rc != 122:
                    break
                require(4 <= size.value <= 16 * 1024 * 1024, 'native-tcp-size-unreadable')
            require(rc == 0, 'native-tcp-table-unreadable')
            raw = table.raw[:size.value]
            count = struct.unpack_from('<I', raw)[0]
            require(4 + count * row_size <= len(raw), 'native-tcp-table-shape')
            for index in range(count):
                off = 4 + index * row_size
                if family == socket.AF_INET:
                    state, addr, port, _, _, owner = struct.unpack_from('<6I', raw, off)
                    loopback = (addr & 255) == 127
                else:
                    addr = raw[off:off + 16]
                    port = struct.unpack_from('<I', raw, off + 20)[0]
                    state, owner = struct.unpack_from('<2I', raw, off + 48)
                    loopback = addr == b'\0' * 15 + b'\1'
                result.append({'port': socket.ntohs(port & 65535), 'state': state, 'owner': owner, 'loopback': loopback})
        return result

    def guard(self, require_cdp=False):
        for role in self.handles:
            self._identity(role)
            require(self._same_user(role), 'native-user-differs')
        expected = {5041: self.identity['server']['pid'], 5572: self.identity['qemu']['pid'], 5573: self.identity['qemu']['pid']}
        if require_cdp:
            expected[19330] = self.identity['server']['pid']
        rows = self.tcp_rows()
        for port, owner in expected.items():
            listeners = [r for r in rows if r['port'] == port and r['state'] == 2]
            require(bool(listeners) and all(r['owner'] == owner and r['loopback'] for r in listeners), 'native-private-port-owner-differs')

    def adb(self, args, allowed=(0,), require_cdp=False):
        self.guard(require_cdp=require_cdp)
        env = os.environ.copy()
        for key in ('ADB_SERVER_SOCKET', 'ANDROID_SERIAL', 'ADB_SERVER_PORT'):
            env.pop(key, None)
        env['ANDROID_ADB_SERVER_PORT'] = '5041'
        env['ADB_LOCAL_TRANSPORT_MAX_PORT'] = '0'
        p = subprocess.Popen([self.identity['server']['image'], '-H', '127.0.0.1', '-P', '5041', '-s', 'emulator-5572'] + args,
                             stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
        self.clients.append(p)  # Keep every original creator object through real exit.
        while True:
            try:
                out, _err = p.communicate(timeout=10)
                break
            except subprocess.TimeoutExpired:
                # A timeout is not terminal, and no process is forcefully stopped.
                self.guard(require_cdp=require_cdp)
        require(p.returncode in allowed, 'native-adb-command-exit')
        return out, p.returncode

    def close(self):
        for p in getattr(self, 'clients', []):
            while True:
                # Query the original Popen HANDLE directly. Unreadable is held
                # unknown and cannot release a manager with a live client.
                try:
                    handle = int(p._handle)
                    wait = self.k.WaitForSingleObject(handle, 0)
                    code = ctypes.c_uint32()
                    if wait == 0 and self.k.GetExitCodeProcess(handle, ctypes.byref(code)):
                        p.returncode = code.value
                        break
                except Exception:
                    pass
                time.sleep(.2)
        remaining = {}
        for role, h in getattr(self, 'handles', {}).items():
            if not self.k.CloseHandle(h):
                remaining[role] = h
        self.handles = remaining
        require(not remaining, 'held-query-handle-close-failed')
