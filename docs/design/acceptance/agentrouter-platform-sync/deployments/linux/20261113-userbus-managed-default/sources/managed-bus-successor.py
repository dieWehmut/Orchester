import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import stat
import subprocess
import time

OUT = Path(__file__).parent
UNITS = Path('/home/han/.config/systemd/user')
SOURCE = OUT / 'extracted/usr/lib/systemd/user'
RUNTIME = Path('/run/user/1000')
REPORT = OUT / 'managed-user-bus-resume-safe.json'
MANAGER_SHA = '39bbddd818ef6b4b9ae87e7cac22476f60a63efb8dfec77dd11f03672462931d'
DBUS_SHA = '9e5cf3260e8b0d68a88732ebc4386970ff5977f4a9bcb9022a13ab905f832bbc'
ENV = {**os.environ, 'XDG_RUNTIME_DIR': str(RUNTIME),
       'DBUS_SESSION_BUS_ADDRESS': 'unix:path=' + str(RUNTIME / 'bus')}
os.umask(0o077)
report = {'schemaVersion': 1, 'observedAtUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
          'status': 'managed-user-bus-preflight', 'managerPid': 269, 'managerStartTicks': 1374,
          'managerRestarted': False, 'systemServicesChanged': False,
          'otherProjectServicesStartedStoppedOrRestarted': False,
          'privateEnvironmentCredentialsStateOrJournalPublished': False,
          'rebootAutostartActuallyTested': False, 'events': []}

def digest(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()

def identity(pid):
    p = Path('/proc') / str(pid)
    fields = (p / 'stat').read_text().rsplit(')', 1)[1].split()
    return {'pid': pid, 'uid': p.stat().st_uid, 'startTicks': int(fields[19]),
            'exeSha256': digest(p / 'exe'), 'cgroup': (p / 'cgroup').read_text().strip()}

def manager_assert():
    info = identity(269)
    assert info['uid'] == os.getuid() == 1000 and info['startTicks'] == 1374
    assert info['exeSha256'] == MANAGER_SHA
    assert info['cgroup'] == '0::/user.slice/user-1000.slice/user@1000.service/init.scope'
    return info

def save():
    p = REPORT.with_suffix('.tmp')
    p.write_text(json.dumps(report, indent=2) + '\n')
    p.chmod(0o600)
    p.replace(REPORT)

def event(label, **kwargs):
    report['events'].append({'label': label, **kwargs})
    save()

def command(label, args):
    p = subprocess.run(args, env=ENV, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=15)
    event(label, actualExitCode=p.returncode, stderrBytes=len(p.stderr))
    assert p.returncode == 0, label
    return p.stdout.decode()

def owner_pid():
    p = subprocess.run(['/usr/bin/busctl', '--address=' + ENV['DBUS_SESSION_BUS_ADDRESS'],
                        '--timeout=3', 'call', 'org.freedesktop.DBus', '/org/freedesktop/DBus',
                        'org.freedesktop.DBus', 'GetConnectionUnixProcessID', 's',
                        'org.freedesktop.systemd1'], env=ENV, stdout=subprocess.PIPE,
                       stderr=subprocess.PIPE, timeout=5)
    if p.returncode:
        return None
    values = p.stdout.decode().split()
    return int(values[1]) if len(values) == 2 and values[0] == 'u' else None

def dbus_identity():
    for p in Path('/proc').iterdir():
        if not p.name.isdecimal():
            continue
        try:
            if p.stat().st_uid != 1000 or (p / 'comm').read_text().strip() != 'dbus-daemon':
                continue
            info = identity(int(p.name))
        except (OSError, ValueError):
            continue
        if info['uid'] == 1000 and info['exeSha256'] == DBUS_SHA and info['cgroup'].endswith('/dbus.service'):
            return info
    return None

def await_condition(predicate, seconds=20):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        v = predicate()
        if v:
            return v
        time.sleep(0.2)
    raise TimeoutError('condition-not-yet-verified')

fd = None
try:
    assert not REPORT.exists()
    prior = json.loads((OUT / 'managed-user-bus-recovery-safe.json').read_text())
    report['priorAttemptSafeReceiptSha256'] = digest(OUT / 'managed-user-bus-recovery-safe.json')
    report['priorObservationFailureBoundary'] = 'SIGHUP was delivered and journal Reloading observed; observer incorrectly required an INFO completion text absent in systemd249. Previous attempt ended before SIGUSR1, and is preserved as failed.'
    manager_assert()
    for field, pid in [('originalControllerIdentity',320847),('originalListenerIdentity',320854),('originalDirmngrIdentity',1377876)]:
        assert identity(pid) == prior[field]
        report[field] = prior[field]
    for name in ['dbus.socket', 'dbus.service']:
        assert digest(UNITS / name) == digest(SOURCE / name)
    wants = UNITS / 'sockets.target.wants'
    link = wants / 'dbus.socket'
    assert link.resolve() == (UNITS / 'dbus.socket').resolve()
    dropdir = UNITS / 'dbus.socket.d'
    drop = dropdir / 'orchester-bootstrap.conf'
    dropbytes = b'[Socket]\nExecStartPost=\n'
    assert drop.read_bytes() == dropbytes
    fd = os.pidfd_open(269)
    manager_assert()
    signal.pidfd_send_signal(fd, signal.SIGUSR1)
    event('supported-sigusr1-after-verified-configuration-reload', signal=signal.SIGUSR1)
    dbus = await_condition(dbus_identity)
    assert stat.S_ISSOCK((RUNTIME / 'bus').stat().st_mode)
    event('original-manager-actually-started-official-dbus', process=dbus)
    manager_assert()
    signal.pidfd_send_signal(fd, signal.SIGUSR1)
    event('supported-sigusr1-reconnect-managed-active-dbus', signal=signal.SIGUSR1)
    await_condition(lambda: owner_pid() == 269)
    event('formal-systemd-user-api-owner-proven', actualOwnerPid=269)
    command('formal-manager-environment-update', ['/usr/bin/systemctl', '--user', 'set-environment',
                                                'DBUS_SESSION_BUS_ADDRESS=' + ENV['DBUS_SESSION_BUS_ADDRESS']])
    assert drop.read_bytes() == dropbytes
    drop.unlink()
    dropdir.rmdir()
    command('formal-daemon-reload-after-dropin-removal', ['/usr/bin/systemctl', '--user', 'daemon-reload'])
    for name in ['dbus.socket', 'dbus.service']:
        assert digest(UNITS / name) == digest(SOURCE / name)
        output = command('final-unit-state-' + name, ['/usr/bin/systemctl', '--user', 'show', name,
                                                   '-p', 'ActiveState', '-p', 'SubState', '-p', 'MainPID',
                                                   '-p', 'FragmentPath', '-p', 'ControlGroup'])
        report[name] = dict(l.split('=', 1) for l in output.splitlines() if '=' in l)
    assert report['dbus.service']['ActiveState'] == 'active' and report['dbus.socket']['ActiveState'] == 'active'
    assert manager_assert()['startTicks'] == 1374
    assert identity(320847) == report['originalControllerIdentity']
    assert identity(320854) == report['originalListenerIdentity']
    assert identity(1377876) == report['originalDirmngrIdentity']
    report.update(status='managed-user-bus-restored-original-manager-and-service-identities-preserved',
                  officialUserUnitsByteExact=True, startupSocketLinkExact=link.resolve() == (UNITS / 'dbus.socket').resolve(),
                  temporaryBootstrapDropinRemoved=True, managerFinalIdentity=manager_assert())
    save()
    print(json.dumps({'status': report['status'], 'safeReceipt': str(REPORT),
                      'dbusPid': report['dbus.service']['MainPID'], 'originalControllerUnchanged': True}))
except Exception as error:
    report.update(status='managed-user-bus-recovery-failed', errorType=type(error).__name__)
    save()
    raise
finally:
    if fd is not None:
        os.close(fd)
