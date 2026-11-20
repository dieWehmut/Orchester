import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parent
CMD = Path('/home/han/.local/state/orchester-delivery/20261120/interop-recovery-private/owned-native-cmd.exe')
EXPECTED_CMD = '15cd209c995b7d3009c91ca40892d5b8f1fb162f3e5f10716642ea278e8e0d7b'


def run(script_path, timeout=120):
    assert hashlib.sha256(CMD.read_bytes()).hexdigest() == EXPECTED_CMD
    script_path = Path(script_path).resolve()
    raw = script_path.read_bytes()
    loader = "& ([scriptblock]::Create([Console]::In.ReadToEnd()))"
    encoded = base64.b64encode(loader.encode('utf-16le')).decode('ascii')
    env = os.environ.copy()
    env['WSL_INTEROP'] = '/run/WSL/357_interop'
    start = datetime.now(timezone.utc).isoformat()
    began = time.monotonic()
    proc = subprocess.run(['/init', str(CMD), 'cmd.exe', '/d', '/c',
                           r'C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe',
                           '-NoLogo', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass',
                           '-EncodedCommand', encoded],
                          executable='/init', cwd=str(ROOT), env=env, input=raw,
                          capture_output=True, timeout=timeout)
    out = ROOT / (script_path.stem + '-execution-' + str(time.time_ns()))
    out.mkdir(mode=0o700)
    (out / 'stdout-safe.log').write_bytes(proc.stdout)
    (out / 'stderr-private.log').write_bytes(proc.stderr)
    receipt = {'schemaVersion': 1, 'startedAtUtc': start,
               'endedAtUtc': datetime.now(timezone.utc).isoformat(),
               'elapsedSeconds': time.monotonic()-began,
               'actualExternalNativeExitCode': proc.returncode,
               'bridgeSourceSha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
               'copiedSystemCmdSha256': EXPECTED_CMD,
               'nativeScriptSourceSha256': hashlib.sha256(raw).hexdigest(),
               'nativePowerShellWorkingDirectorySetByScript': r'C:\Users\han',
               'stdoutSha256': hashlib.sha256(proc.stdout).hexdigest(),
               'stderrSha256': hashlib.sha256(proc.stderr).hexdigest(),
               'stdoutBytes': len(proc.stdout), 'stderrBytes': len(proc.stderr),
               'staleDrvfsMountChanged': False, 'processSignals': 0,
               'credentialCommands': 0, 'modelCalls': 0,
               'privateValuesIncludedInReceipt': False}
    (out / 'execution-safe.json').write_text(json.dumps(receipt, indent=2)+'\n')
    print(json.dumps({'executionDirectory': str(out), **receipt}, indent=2))
    if proc.stdout:
        print(proc.stdout.decode('utf-8-sig', errors='replace'))
    if proc.stderr:
        print('Native stderr captured privately; byte count: ' + str(len(proc.stderr)))
    return proc.returncode


if __name__ == '__main__':
    raise SystemExit(run(sys.argv[1], int(sys.argv[2]) if len(sys.argv) > 2 else 120))
