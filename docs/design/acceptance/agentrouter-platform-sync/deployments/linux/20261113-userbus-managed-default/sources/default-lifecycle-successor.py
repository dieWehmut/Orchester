import datetime
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import select
import shutil
import sqlite3
import stat
import subprocess
import time

OUT = Path(__file__).parent
NEW = OUT / 'persistent-controller'
OLD = Path('/home/han/.local/state/orchester-delivery/20261112-post-reboot/linux-restore')
UNIT = 'orchester-default-remote-7337-20261112-post-reboot.service'
UNIT_PATH = Path('/home/han/.config/systemd/user') / UNIT
UNIT_ENV = {**os.environ, 'XDG_RUNTIME_DIR': '/run/user/1000',
            'DBUS_SESSION_BUS_ADDRESS': 'unix:path=/run/user/1000/bus'}
REPORT = OUT / 'persistent-service-lifecycle-safe.json'
os.umask(0o077)
report = {'schemaVersion': 1, 'observedAtUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
          'status': 'transfer-preflight', 'sourceRevision': '71def643cb08b208cc79687f79255c504eb14dc6',
          'userManagerRestarted': False, 'WSLOrWindowsRebooted': False,
          'otherProjectServicesStartedStoppedOrRestarted': False,
          'privateCredentialsStateEnvironmentValuesPublished': False, 'processSignals': 0,
          'rebootAutostartActuallyTested': False, 'events': []}

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def save():
    t = REPORT.with_suffix('.tmp')
    t.write_text(json.dumps(report, indent=2) + '\n')
    t.chmod(0o600)
    t.replace(REPORT)

def event(label, **fields):
    report['events'].append({'label': label, **fields})
    save()

def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod

def command(label, args, timeout=20):
    p = subprocess.run(args, env=UNIT_ENV, stdout=subprocess.PIPE,
                       stderr=subprocess.PIPE, timeout=timeout)
    event(label, actualExitCode=p.returncode, stderrBytes=len(p.stderr))
    assert p.returncode == 0, label
    return p.stdout

def state():
    p = subprocess.run(['/usr/bin/systemctl', '--user', 'show', UNIT, '-p', 'MainPID',
                        '-p', 'ControlGroup', '-p', 'ActiveState', '-p', 'SubState', '-p', 'UnitFileState'],
                       env=UNIT_ENV, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10)
    assert p.returncode == 0
    return dict(line.split('=', 1) for line in p.stdout.decode().splitlines() if '=' in line)

def wait_ready(seconds=120):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        s = state()
        if s['ActiveState'] == 'failed':
            raise RuntimeError('new-managed-controller-failed')
        if (NEW / 'final-safe.json').exists():
            current = json.loads((NEW / 'final-safe.json').read_text())
            if current.get('status') == 'deployed-and-verified' and current.get('controllerPid') == int(s['MainPID']):
                return current, s
            if current.get('controllerPid') == int(s['MainPID']) and current.get('status', '').endswith('failed-owned-listener-stopped'):
                raise RuntimeError('new-managed-controller-validation-failed')
        time.sleep(0.2)
    raise TimeoutError('new-controller-readiness-not-yet-observed')

new = load_module('new_owned_controller', NEW / 'controller-successor.py')
try:
    assert not REPORT.exists()
    prior = json.loads((OUT / 'persistent-service-transfer-safe.json').read_text())
    report['priorTransferFailureReceiptSha256'] = digest(OUT / 'persistent-service-transfer-safe.json')
    report['oldExactOwnedNormalQuitVerified'] = next(e for e in prior['events'] if e['label'] == 'original-controller-normal-quit-proven-by-original-writer-and-pidfd-terminal')
    current = json.loads((NEW / 'final-safe.json').read_text())
    assert current['status'] == 'deployed-and-verified'
    old_final_sha = digest(OLD / 'final-safe.json')
    before_runs, before_raw, before_parsed, before_counts = new.snapshot()
    backups = sorted(NEW.glob('private-backup-*'))
    assert backups
    with sqlite3.connect((backups[0] / 'state/remote.db').as_uri() + '?mode=ro', uri=True) as db:
        original_runs = db.execute('SELECT workspace,id,value FROM remote_runs ORDER BY workspace,id').fetchall()
        original_raw = db.execute('SELECT key,value FROM remote_state ORDER BY key').fetchall()
    original_parsed = {k: json.loads(v) for k,v in original_raw}
    assert before_runs == original_runs and before_parsed == original_parsed
    report['originalRunCount'] = len(original_runs)
    report['originalAuthEntryCount'] = sum(len(v.get('tokens',{})) for k,v in original_parsed.items() if k.startswith('auth:'))
    report['preTransferPrivateBackupRowsAndJsonExact'] = True
    diagnostic_dir = UNIT_PATH.parent / (UNIT + '.d')
    diagnostic = diagnostic_dir / 'orchester-initial-failure-diagnostic.conf'
    assert diagnostic.read_text() == '[Service]\nRestart=no\nStandardError=append:/home/han/.local/state/orchester-delivery/20261113/linux-service-recovery/managed-controller-stderr-private.log\n'
    diagnostic.unlink()
    diagnostic_dir.rmdir()
    command('diagnostic-dropin-removed-final-unit-daemon-reload', ['/usr/bin/systemctl','--user','daemon-reload'])
    command('final-default-unit-systemd-verify', ['/usr/bin/systemd-analyze','--user','verify',str(UNIT_PATH)])
    first, first_state = wait_ready()
    assert first_state['ActiveState'] == 'active' and first_state['SubState'] == 'running'
    assert first_state['ControlGroup'].endswith('/' + UNIT)
    first_status, _ = new.private_client('status')
    assert first_status['controllerPid'] == int(first_state['MainPID']) == first['controllerPid']
    assert first_status['status'] == 'live-owned' and first_status['listening7337']
    after_runs, after_raw, after_parsed, _ = new.snapshot()
    assert after_runs == before_runs and after_parsed == before_parsed
    first_archive = OUT / 'first-managed-start'
    first_archive.mkdir(mode=0o700)
    shutil.copy2(NEW / 'final-safe.json', first_archive / 'producer-final-safe.json')
    event('first-managed-start-fully-verified', unitState=first_state, controllerStatus=first_status,
          originalRunRowsExact=True, allRemoteStateJsonExact=True, allRemoteStateRawRowsExact=after_raw == before_raw,
          currentCanonicalProviders=first['canonicalProviderCount'], publicResourcesVerified=len(first['publicResources']))
    first_controller_fd = os.pidfd_open(first['controllerPid'])
    first_listener_fd = os.pidfd_open(first['pid'])
    assert new.private_client('status')[0]['controllerStartTicks'] == first['controllerStartTicks']
    command('managed-default-service-normal-systemctl-stop', ['/usr/bin/systemctl', '--user', 'stop', UNIT], timeout=45)
    assert select.select([first_controller_fd], [], [], 0)[0] and select.select([first_listener_fd], [], [], 0)[0]
    os.close(first_controller_fd)
    os.close(first_listener_fd)
    stopped = state()
    assert stopped['ActiveState'] == 'inactive' and stopped['MainPID'] == '0' and not new.open_port()
    exit_record = json.loads((NEW / 'owned-exit-safe.json').read_text())
    assert exit_record['listenerPid'] == first['pid'] and exit_record['exitCode'] == 0
    shutil.copy2(NEW / 'owned-exit-safe.json', first_archive / 'normal-exit-safe.json')
    shutil.copy2(NEW / 'final-safe.json', first_archive / 'producer-final-after-stop-safe.json')
    stopped_runs, stopped_raw, stopped_parsed, _ = new.snapshot()
    assert stopped_runs == before_runs and stopped_parsed == before_parsed
    event('managed-normal-stop-original-processes-terminal-and-state-retained', unitState=stopped,
          listenerActualExitCode=0, originalRunRowsExact=True, allRemoteStateJsonExact=True, portReleased=True)
    command('second-managed-default-service-start', ['/usr/bin/systemctl', '--user', 'start', UNIT])
    second, second_state = wait_ready()
    assert second['controllerPid'] != first['controllerPid'] and second['pid'] != first['pid']
    second_status, _ = new.private_client('status')
    assert second_status['status'] == 'live-owned' and second_status['controllerPid'] == int(second_state['MainPID'])
    final_runs, final_raw, final_parsed, _ = new.snapshot()
    assert final_runs == before_runs and final_parsed == before_parsed
    assert second_state['ControlGroup'].endswith('/' + UNIT)
    command('default-unit-is-enabled', ['/usr/bin/systemctl', '--user', 'is-enabled', UNIT])
    default_link = UNIT_PATH.parent / 'default.target.wants' / UNIT
    assert default_link.resolve() == UNIT_PATH.resolve()
    assert digest(OLD / 'final-safe.json') == old_final_sha
    report.update(status='default-service-managed-start-stop-start-verified-and-left-running',
                  finalUnitState=second_state, finalControllerStatus=second_status,
                  currentRunRowsByteExact=True, currentRemoteStateJsonExact=True,
                  currentRemoteStateRawRowsByteExact=final_raw == before_raw,
                  originalPersistedAuthValuesExact=True, originalPrivateEnvironmentCopyExact=True,
                  oldFinalReceiptUnchanged=True, diagnosticDropinRemoved=True, canonicalProviderCount=second['canonicalProviderCount'],
                  publicResourceCount=len(second['publicResources']), finalProducerReceiptSha256=digest(NEW / 'final-safe.json'),
                  defaultTargetStartupLinkExact=True, actualWSLRebootAutostartVerified=False,
                  currentUnitSha256=digest(UNIT_PATH), systemctlLifecycleVerified=True)
    save()
    print(json.dumps({'status': report['status'], 'safeReceipt': str(REPORT),
                      'controllerPid': second['controllerPid'], 'listenerPid': second['pid'],
                      'sameOriginal5RunsAnd5AuthEntriesPreserved': True,
                      'actualWSLRebootAutostartVerified': False}))
except Exception as error:
    report.update(status='persistent-service-transfer-failed', errorType=type(error).__name__)
    save()
    raise
