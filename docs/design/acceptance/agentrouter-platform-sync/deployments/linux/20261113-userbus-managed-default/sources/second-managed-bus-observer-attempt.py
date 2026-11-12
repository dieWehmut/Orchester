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
REPORT = OUT / 'managed-user-bus-recovery-safe.json'
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
    assert not REPORT.exists(), 'attempt2-receipt-already-exists'
    manager_assert()
    report['originalControllerIdentity'] = identity(320847)
    report['originalListenerIdentity'] = identity(320854)
    report['originalDirmngrIdentity'] = identity(1377876)
    assert not (RUNTIME / 'bus').exists() and not (RUNTIME / 'bus').is_symlink()
    assert digest(OUT / 'dbus-user-session_1.12.20-2ubuntu4.1_amd64.deb') == 'fcd6b45ba6577f4ffb395a08fc73aedc0e530f8d72d7b24575e44e4b7fd55d3b'
    for name in ['dbus.socket', 'dbus.service']:
        p = UNITS / name
        assert not p.exists() and not p.is_symlink()
        with p.open('xb') as f:
            f.write((SOURCE / name).read_bytes())
        p.chmod(0o600)
        assert digest(p) == digest(SOURCE / name)
        event('official-unit-created', unit=name, sha256=digest(p))
    wants = UNITS / 'sockets.target.wants'
    wants.mkdir(exist_ok=True, mode=0o700)
    link = wants / 'dbus.socket'
    assert not link.exists() and not link.is_symlink()
    link.symlink_to('../dbus.socket')
    dropdir = UNITS / 'dbus.socket.d'
    assert not dropdir.exists()
    dropdir.mkdir(mode=0o700)
    drop = dropdir / 'orchester-bootstrap.conf'
    dropbytes = b'[Socket]\nExecStartPost=\n'
    drop.write_bytes(dropbytes)
    drop.chmod(0o600)
    fd = os.pidfd_open(269)
    manager_assert()
    since = datetime.datetime.now(datetime.timezone.utc).isoformat()
    signal.pidfd_send_signal(fd, signal.SIGHUP)
    event('supported-sighup-daemon-reload-original-manager', signal=signal.SIGHUP)
    def reload_completed():
        p = subprocess.run(['journalctl', '--user', '_PID=269', '--since=' + since,
                            '--no-pager', '-o', 'cat'], stdout=subprocess.PIPE,
                           stderr=subprocess.PIPE, timeout=5)
        return p.returncode == 0 and b'Reloading finished' in p.stdout
    await_condition(reload_completed)
    event('reload-finished-confirmed-from-narrow-journal', journalRawValuesPublished=False)
    manager_assert()
    signal.pidfd_send_signal(fd, signal.SIGUSR1)
    event('supported-sigusr1-managed-dbus-start', signal=signal.SIGUSR1)
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
