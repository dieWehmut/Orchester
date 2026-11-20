#!/usr/bin/env python3
"""Run the existing native observer and preserve its actual return status."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
from datetime import datetime, timezone

ROOT = Path('/home/han/.local/state/orchester-delivery/20261119/default-finish-0e6df6887414')
NATIVE = Path('/mnt/c/Users/han/AppData/Local/Orchester-build/default-finish-0e6df6887414')
EXE = Path('/mnt/c/WINDOWS/System32/WindowsPowerShell/v1.0/powershell.exe')
SCRIPT = NATIVE / '08-verify-current-stable-managed.ps1'
EXPECTED_EXE = '60c9b29843624dd8af6b6a7b26147753be6aeef65098b9a503cc50850afe410f'
EXPECTED_SCRIPT = '12256f8e55bd690a85f74a2bf157c2f73f6c07d35f22275671f3d09a56301ae5'

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def utc():
    return datetime.now(timezone.utc).isoformat()

assert sha(EXE) == EXPECTED_EXE
assert sha(SCRIPT) == EXPECTED_SCRIPT
out = ROOT / ('refresh-' + datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S%fZ'))
out.mkdir(mode=0o700)
argv = [str(EXE), '-NoLogo', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File',
        r'C:\Users\han\AppData\Local\Orchester-build\default-finish-0e6df6887414\08-verify-current-stable-managed.ps1',
        '-ObserveSeconds', '30']
started = utc()
result = subprocess.run(argv, cwd='/mnt/c/Users/han', capture_output=True, check=False)
ended = utc()
(out / 'native-observer-stdout-safe.log').write_bytes(result.stdout)
(out / 'native-observer-stderr-safe.log').write_bytes(result.stderr)
native = json.loads(result.stdout.decode('utf-8-sig')) if result.returncode == 0 else None
passed = bool(result.returncode == 0 and native and
              native['status'] == 'stable-original-state-managed-7338-resources-retention-and-real-cycles-verified')
receipt = {
    'schemaVersion': 1,
    'status': 'independent-wsl-executor-observed-native-observer-exit-zero' if passed else 'native-observer-execution-failed',
    'startedAtUtc': started,
    'endedAtUtc': ended,
    'executorSourceSha256': sha(__file__),
    'nativePowerShellExeSha256': EXPECTED_EXE,
    'nativeObserverSourceSha256': EXPECTED_SCRIPT,
    'nativeCommandWorkingDirectory': r'C:\Users\han',
    'nativeObserverArgumentObserveSeconds': 30,
    'wslInteropObservedNativePowerShellExitCode': result.returncode,
    'nativeObserverStdoutSha256': sha(out / 'native-observer-stdout-safe.log'),
    'nativeObserverStderrSha256': sha(out / 'native-observer-stderr-safe.log'),
    'observerResultDirectory': native['observerResultDirectory'] if native else None,
    'existingRuntimeReused': True,
    'runtimeRestartCount': 0,
    'processSignals': 0,
    'credentialCommands': 0,
    'modelCalls': 0,
    'privateValuesIncluded': False,
}
(out / 'native-observer-execution-safe.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps({'executionDirectory': str(out), **receipt}, indent=2))
raise SystemExit(0 if passed else 1)
