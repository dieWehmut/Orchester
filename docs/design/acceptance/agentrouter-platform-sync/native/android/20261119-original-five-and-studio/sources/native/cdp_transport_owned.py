"""Task-owned loopback CDP transport. No network work occurs at import."""
import base64
import hashlib
import json
import os
import socket
import struct
import time
import urllib.parse
import urllib.request


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise RuntimeError('owned-cdp-redirect-refused')


class CDP:
    def __init__(self, port=19330):
        if port != 19330:
            raise RuntimeError('fixed-owned-cdp-port-required')
        self.s = None
        self._call_deadline = None
        try:
            opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
            with opener.open('http://127.0.0.1:19330/json/list', timeout=5) as response:
                raw = response.read(1024 * 1024 + 1)
            if len(raw) > 1024 * 1024:
                raise RuntimeError('owned-cdp-target-list-too-large')
            pages = [p for p in json.loads(raw) if p.get('url') == 'https://localhost/' and p.get('type') == 'page']
            if len(pages) != 1:
                raise RuntimeError('owned-cdp-page-not-unique')
            u = urllib.parse.urlparse(pages[0]['webSocketDebuggerUrl'])
            if u.scheme != 'ws' or u.hostname not in ('127.0.0.1', 'localhost') or u.port != 19330 or u.username or u.password or u.fragment:
                raise RuntimeError('unexpected-owned-cdp-origin')
            self.s = socket.create_connection(('127.0.0.1', 19330), 5)
            self.s.settimeout(45)
            self.buf, self.seq = b'', 0
            key = base64.b64encode(os.urandom(16)).decode()
            target = u.path + ('?' + u.query if u.query else '')
            if '\r' in target or '\n' in target:
                raise RuntimeError('owned-cdp-handshake-target-invalid')
            self.s.sendall(('GET ' + target + ' HTTP/1.1\r\nHost: 127.0.0.1:19330\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: ' + key + '\r\nSec-WebSocket-Version: 13\r\n\r\n').encode())
            data = b''
            while b'\r\n\r\n' not in data:
                chunk = self.s.recv(4096)
                if not chunk or len(data) + len(chunk) > 65536:
                    raise RuntimeError('owned-cdp-handshake-invalid')
                data += chunk
            header, self.buf = data.split(b'\r\n\r\n', 1)
            lines = header.decode('ascii').split('\r\n')
            fields = {}
            for line in lines[1:]:
                name, value = line.split(':', 1)
                name = name.strip().lower()
                if name in fields:
                    raise RuntimeError('owned-cdp-handshake-duplicate-header')
                fields[name] = value.strip()
            expected = base64.b64encode(hashlib.sha1((key + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').encode()).digest()).decode()
            if ' 101 ' not in lines[0] or fields.get('sec-websocket-accept') != expected or fields.get('upgrade', '').lower() != 'websocket':
                raise RuntimeError('owned-cdp-handshake-validation-failed')
        except BaseException:
            self.close()
            raise

    def read(self, n):
        if n > 4 * 1024 * 1024:
            raise RuntimeError('owned-cdp-frame-too-large')
        while len(self.buf) < n:
            if self._call_deadline is not None:
                remaining = self._call_deadline - time.monotonic()
                if remaining <= 0:
                    raise RuntimeError('owned-cdp-call-total-timeout')
                self.s.settimeout(remaining)
            chunk = self.s.recv(65536)
            if not chunk:
                raise RuntimeError('owned-cdp-socket-eof')
            self.buf += chunk
        result, self.buf = self.buf[:n], self.buf[n:]
        return result

    def send(self, data, op=1):
        raw = data.encode() if isinstance(data, str) else data
        n, mask = len(raw), os.urandom(4)
        header = bytes([128 | op, 128 | (n if n < 126 else 126 if n < 65536 else 127)])
        if n >= 126:
            header += struct.pack('!H' if n < 65536 else '!Q', n)
        self.s.sendall(header + mask + bytes(x ^ mask[i % 4] for i, x in enumerate(raw)))

    def recv(self):
        parts, size = [], 0
        while True:
            a, b = self.read(2)
            n = b & 127
            if n == 126:
                n = struct.unpack('!H', self.read(2))[0]
            elif n == 127:
                n = struct.unpack('!Q', self.read(8))[0]
            mask = self.read(4) if b & 128 else None
            payload = self.read(n)
            if mask:
                payload = bytes(x ^ mask[i % 4] for i, x in enumerate(payload))
            if a & 15 == 9:
                self.send(payload, 10)
                continue
            if a & 15 == 10:
                continue
            if a & 15 == 8:
                raise RuntimeError('owned-cdp-socket-closed')
            size += len(payload)
            if size > 4 * 1024 * 1024:
                raise RuntimeError('owned-cdp-message-too-large')
            parts.append(payload)
            if a & 128:
                return json.loads(b''.join(parts))

    def call(self, method, params):
        self.seq += 1
        seq = self.seq
        self._call_deadline = time.monotonic() + 45
        try:
            self.send(json.dumps({'id': seq, 'method': method, 'params': params}))
            while True:
                if time.monotonic() >= self._call_deadline:
                    raise RuntimeError('owned-cdp-call-total-timeout')
                result = self.recv()
                if result.get('id') == seq:
                    if 'error' in result:
                        raise RuntimeError('owned-cdp-protocol-error')
                    return result.get('result')
        finally:
            self._call_deadline = None
            self.s.settimeout(45)

    def eval(self, expression):
        result = self.call('Runtime.evaluate', {'expression': expression, 'returnByValue': True, 'awaitPromise': True})
        if 'exceptionDetails' in result:
            raise RuntimeError('owned-cdp-page-expression-failed')
        return result['result'].get('value')

    def close(self):
        if self.s is not None:
            self.s.close()
            self.s = None
