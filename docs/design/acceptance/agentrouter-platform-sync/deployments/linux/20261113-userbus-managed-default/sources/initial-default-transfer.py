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
REPORT = OUT / 'persistent-service-transfer-safe.json'
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

old = load_module('old_owned_controller', OLD / 'controller.py')
new = load_module('new_owned_controller', NEW / 'controller.py')
old_controller_fd = None
old_listener_fd = None
try:
    assert not REPORT.exists()
    preparation = json.loads((OUT / 'new-controller-preparation-safe.json').read_text())
    assert digest(NEW / 'controller.py') == preparation['newControllerSha256']
    assert digest(OLD / 'controller.py') == preparation['originalControllerSourceSha256']
    assert digest(UNIT_PATH) == preparation['originalUnitSha256']
    assert (OLD / 'environment-current-private.json').read_bytes() == (NEW / 'environment-current-private.json').read_bytes()
    assert new.digest(new.BINARY) == new.EXPECTED_SHA
    assert state()['ActiveState'] == 'inactive' and state()['MainPID'] == '0'
    prior_report_sha = digest(OLD / 'final-safe.json')
    prior = json.loads((OLD / 'final-safe.json').read_text())
    assert old.identity(320847)[0] == prior['controllerStartTicks'] == 451383
    assert old.identity(320854)[0] == prior['startTicks'] == 451458
    old_status, _ = old.private_client('status')
    assert old_status['status'] == 'live-owned' and old_status['controllerPid'] == 320847 and old_status['listenerPid'] == 320854
    assert old_status['listenerExecutableSha256'] == new.EXPECTED_SHA
    old_controller_fd = os.pidfd_open(320847)
    old_listener_fd = os.pidfd_open(320854)
    assert old.identity(320847)[0] == 451383 and old.identity(320854)[0] == 451458
    before_runs, before_raw, before_parsed, before_counts = new.snapshot()
    directory, records, copied_runs, copied_state = new.backup_state()
    assert copied_runs == before_runs and copied_state == before_raw
    report['consistentPrivateBackupBeforeTransfer'] = {'fileCount': len(records),
        'allSourceBeforeAfterExact': all(r['sourceBeforeAfterExact'] for r in records),
        'allBackupRunRowsExact': copied_runs == before_runs, 'allBackupStateRowsExact': copied_state == before_raw,
        'originalRunCount': len(before_runs), 'originalAuthEntryCount': sum(len(v.get('tokens', {})) for k, v in before_parsed.items() if k.startswith('auth:'))}
    event('new-consistent-private-backup-verified-before-any-stop')
    unit_bytes = ('[Unit]\nDescription=Orchester persistent default remote 7337 owned controller\n\n'
        '[Service]\nType=simple\n'
        f'ExecStart=/usr/bin/python3 {NEW / "controller.py"} serve\n'
        f'ExecStop=/usr/bin/python3 {NEW / "controller.py"} quit\n'
        'WorkingDirectory=/home/han/project/Orchester\nStandardInput=null\nStandardOutput=null\nStandardError=null\n'
        'Restart=on-failure\nRestartSec=5\nKillMode=process\nSendSIGKILL=no\nTimeoutStopSec=infinity\nUMask=0077\n\n'
        '[Install]\nWantedBy=default.target\n').encode()
    candidate = UNIT_PATH.with_suffix('.candidate')
    with candidate.open('xb') as f:
        f.write(unit_bytes)
    candidate.chmod(0o600)
    assert digest(UNIT_PATH) == preparation['originalUnitSha256']
    candidate.replace(UNIT_PATH)
    command('updated-default-unit-systemd-verify', ['/usr/bin/systemd-analyze', '--user', 'verify', str(UNIT_PATH)])
    command('updated-default-unit-daemon-reload', ['/usr/bin/systemctl', '--user', 'daemon-reload'])
    assert state()['ActiveState'] == 'inactive'
    assert old.private_client('status')[0]['status'] == 'live-owned'
    event('current-original-controller-status-before-normal-quit-proven', controllerPid=320847, listenerPid=320854)
    quit_reply, _ = old.private_client('quit')
    assert quit_reply == {'status': 'owned-normal-quit-complete', 'listenerExitCode': 0,
                          'samePopenWriterUsed': True, 'userPidSignals': 0}
    assert select.select([old_listener_fd], [], [], 15)[0]
    assert select.select([old_controller_fd], [], [], 15)[0]
    assert not new.open_port()
    assert digest(OLD / 'final-safe.json') == prior_report_sha
    event('original-controller-normal-quit-proven-by-original-writer-and-pidfd-terminal',
          control=quit_reply, originalControllerTerminal=True, originalListenerTerminal=True,
          port7337ReleasedBeforeNewStart=True, originalFinalReceiptUnchanged=True)
    command('first-managed-default-service-start', ['/usr/bin/systemctl', '--user', 'start', UNIT])
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
    assert digest(OLD / 'final-safe.json') == prior_report_sha
    report.update(status='default-service-managed-start-stop-start-verified-and-left-running',
                  finalUnitState=second_state, finalControllerStatus=second_status,
                  currentRunRowsByteExact=True, currentRemoteStateJsonExact=True,
                  currentRemoteStateRawRowsByteExact=final_raw == before_raw,
                  originalPersistedAuthValuesExact=True, originalPrivateEnvironmentCopyExact=True,
                  oldFinalReceiptUnchanged=True, canonicalProviderCount=second['canonicalProviderCount'],
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
finally:
    if old_controller_fd is not None:
        os.close(old_controller_fd)
    if old_listener_fd is not None:
        os.close(old_listener_fd)
