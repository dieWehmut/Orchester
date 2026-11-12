import datetime
import hashlib
import json
import os
from pathlib import Path
import select
import shutil
import signal
import socket
import stat
import subprocess
import tempfile
import time

OUT = Path(__file__).parent
RUNTIME = Path('/run/user/1000')
CANONICAL = RUNTIME / 'bus'
USER_UNITS = Path('/home/han/.config/systemd/user')
SOURCE = OUT / 'extracted/usr/lib/systemd/user'
REPORT = OUT / 'user-bus-recovery-safe.json'
MANAGER_PID = 269
MANAGER_TICKS = 1374
MANAGER_SHA = '39bbddd818ef6b4b9ae87e7cac22476f60a63efb8dfec77dd11f03672462931d'
DBUS_SHA = '9e5cf3260e8b0d68a88732ebc4386970ff5977f4a9bcb9022a13ab905f832bbc'

os.umask(0o077)
report = {
    'schemaVersion': 1,
    'observedAtUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'status': 'recovery-preflight',
    'managerPid': MANAGER_PID,
    'managerStartTicks': MANAGER_TICKS,
    'managerRestarted': False,
    'systemServicesChanged': False,
    'otherProjectServicesStartedStoppedOrRestarted': False,
    'privateEnvironmentCredentialsStateOrJournalPublished': False,
    'rebootAutostartActuallyTested': False,
    'events': [],
}

def save():
    candidate = REPORT.with_suffix('.tmp')
    candidate.write_text(json.dumps(report, indent=2) + '\n')
    candidate.chmod(0o600)
    candidate.replace(REPORT)

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def identity(pid):
    proc = Path('/proc') / str(pid)
    fields = (proc / 'stat').read_text().rsplit(')', 1)[1].split()
    return {'pid': pid, 'uid': proc.stat().st_uid, 'startTicks': int(fields[19]),
            'exeSha256': digest(proc / 'exe'), 'cgroup': (proc / 'cgroup').read_text().strip()}

def manager_assert():
    info = identity(MANAGER_PID)
    assert info['uid'] == os.getuid() == 1000
    assert info['startTicks'] == MANAGER_TICKS and info['exeSha256'] == MANAGER_SHA
    assert info['cgroup'] == '0::/user.slice/user-1000.slice/user@1000.service/init.scope'
    return info

def event(label, **fields):
    report['events'].append({'label': label, **fields})
    save()

def command(label, args, env, timeout=15, required=True):
    p = subprocess.run(args, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                       timeout=timeout)
    event(label, actualExitCode=p.returncode, stderrBytes=len(p.stderr))
    if required and p.returncode:
        raise RuntimeError(label)
    return p

def unit_state(name, env):
    p = subprocess.run(['/usr/bin/systemctl', '--user', 'show', name,
                        '-p', 'ActiveState', '-p', 'SubState', '-p', 'MainPID',
                        '-p', 'ControlGroup', '-p', 'LoadState', '-p', 'FragmentPath'],
                       env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=5)
    if p.returncode:
        return None
    return dict(line.split('=', 1) for line in p.stdout.decode().splitlines() if '=' in line)

def owner_pid(address):
    p = subprocess.run(['/usr/bin/busctl', '--address=' + address, '--timeout=3', 'call',
                        'org.freedesktop.DBus', '/org/freedesktop/DBus',
                        'org.freedesktop.DBus', 'GetConnectionUnixProcessID', 's',
                        'org.freedesktop.systemd1'], stdout=subprocess.PIPE,
                       stderr=subprocess.PIPE, timeout=5)
    if p.returncode:
        return None
    fields = p.stdout.decode().split()
    return int(fields[1]) if len(fields) == 2 and fields[0] == 'u' else None

def await_condition(predicate, timeout=20):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(0.2)
    raise TimeoutError('condition-not-yet-verified')

def remove_own_symlink(path, target):
    assert path.is_symlink() and os.readlink(path) == str(target)
    path.unlink()

manager_fd = None
child_fd = None
child = None
temporary_root = None
bootstrap_path = None
canonical_link_created = False
formal_socket_started = False
try:
    manager_assert()
    assert digest('/usr/bin/dbus-daemon') == DBUS_SHA
    assert not CANONICAL.exists() and not CANONICAL.is_symlink()
    for name in ['dbus.socket', 'dbus.service']:
        assert not (USER_UNITS / name).exists() and not (USER_UNITS / name).is_symlink()
    assert digest(OUT / 'dbus-user-session_1.12.20-2ubuntu4.1_amd64.deb') == 'fcd6b45ba6577f4ffb395a08fc73aedc0e530f8d72d7b24575e44e4b7fd55d3b'
    manager_fd = os.pidfd_open(MANAGER_PID)
    manager_assert()
    temporary_root = Path(tempfile.mkdtemp(prefix='orchester-user-bus-bootstrap-', dir=RUNTIME))
    bootstrap_path = temporary_root / 'bus'
    address = 'unix:path=' + str(bootstrap_path)
    with (OUT / 'bootstrap-daemon-stderr-private.log').open('ab') as errors:
        child = subprocess.Popen(['/usr/bin/dbus-daemon', '--session', '--nofork',
                                  '--nopidfile', '--nosyslog', '--address=' + address,
                                  '--print-address'], stdout=subprocess.PIPE, stderr=errors,
                                 stdin=subprocess.DEVNULL, close_fds=True)
    child_fd = os.pidfd_open(child.pid)
    assert select.select([child.stdout], [], [], 5)[0], 'daemon-start-address-not-observed'
    published_address = child.stdout.readline().decode().strip()
    assert published_address.startswith(address + ',guid=')
    assert stat.S_ISSOCK(bootstrap_path.stat().st_mode)
    bootstrap_identity = identity(child.pid)
    assert bootstrap_identity['exeSha256'] == DBUS_SHA
    report['bootstrapOriginalPopenAndPidfdIdentity'] = bootstrap_identity
    CANONICAL.symlink_to(bootstrap_path)
    canonical_link_created = True
    manager_assert()
    signal.pidfd_send_signal(manager_fd, signal.SIGUSR1)
    event('accurate-manager-supported-sigusr1-reconnect', actualSignal=signal.SIGUSR1)
    await_condition(lambda: owner_pid(address) == MANAGER_PID)
    event('bootstrap-systemd-api-owner-proven', actualOwnerPid=MANAGER_PID)
    environment = {**os.environ, 'XDG_RUNTIME_DIR': str(RUNTIME),
                   'DBUS_SESSION_BUS_ADDRESS': address}
    for name in ['dbus.socket', 'dbus.service']:
        destination = USER_UNITS / name
        with destination.open('xb') as file:
            file.write((SOURCE / name).read_bytes())
        destination.chmod(0o600)
        assert digest(destination) == digest(SOURCE / name)
        event('official-user-unit-created', unit=name, sha256=digest(destination))
    wants = USER_UNITS / 'sockets.target.wants'
    wants.mkdir(exist_ok=True, mode=0o700)
    link = wants / 'dbus.socket'
    assert not link.exists() and not link.is_symlink()
    link.symlink_to('../dbus.socket')
    drop_directory = USER_UNITS / 'dbus.socket.d'
    assert not drop_directory.exists()
    drop_directory.mkdir(mode=0o700)
    drop_file = drop_directory / 'orchester-bootstrap.conf'
    drop_bytes = b'[Socket]\nExecStartPost=\n'
    drop_file.write_bytes(drop_bytes)
    drop_file.chmod(0o600)
    event('temporary-bootstrap-dropin-created', purpose='defer-official-manager-environment-update-until-formal-api-reconnected')
    command('bootstrap-api-daemon-reload', ['/usr/bin/systemctl', '--user', 'daemon-reload'], environment)
    remove_own_symlink(CANONICAL, bootstrap_path)
    canonical_link_created = False
    command('formal-dbus-socket-start-request', ['/usr/bin/systemctl', '--user', '--no-block', 'start', 'dbus.socket'], environment)
    state = await_condition(lambda: (v if (v := unit_state('dbus.socket', environment)) and v.get('ActiveState') == 'active' else None))
    formal_socket_started = True
    event('formal-dbus-socket-actually-active', state=state)
    assert stat.S_ISSOCK(CANONICAL.stat().st_mode)
    command('formal-dbus-service-start-request', ['/usr/bin/systemctl', '--user', '--no-block', 'start', 'dbus.service'], environment)
    formal_state = await_condition(lambda: (v if (v := unit_state('dbus.service', environment)) and v.get('ActiveState') == 'active' and int(v.get('MainPID', '0')) > 0 else None))
    formal_identity = identity(int(formal_state['MainPID']))
    assert formal_identity['exeSha256'] == DBUS_SHA and formal_identity['uid'] == 1000
    assert formal_identity['cgroup'].endswith('/dbus.service')
    event('formal-dbus-service-owned-by-original-manager', state=formal_state, process=formal_identity)
    assert child.poll() is None
    assert identity(child.pid) == bootstrap_identity
    signal.pidfd_send_signal(child_fd, signal.SIGTERM)
    child.wait(timeout=10)
    event('original-bootstrap-daemon-normal-termination', actualExitCode=child.returncode, signal=signal.SIGTERM)
    await_condition(lambda: owner_pid('unix:path=' + str(CANONICAL)) == MANAGER_PID, timeout=2)
except TimeoutError as error:
    if formal_socket_started and child is not None and child.poll() is not None:
        manager_assert()
        signal.pidfd_send_signal(manager_fd, signal.SIGUSR1)
        event('formal-supported-sigusr1-reconnect', actualSignal=signal.SIGUSR1)
        await_condition(lambda: owner_pid('unix:path=' + str(CANONICAL)) == MANAGER_PID)
    else:
        report['status'] = 'recovery-failed'
        report['errorType'] = type(error).__name__
        save()
        raise
except Exception as error:
    report['status'] = 'recovery-failed'
    report['errorType'] = type(error).__name__
    save()
    raise
finally:
    if child is not None and child.poll() is None:
        signal.pidfd_send_signal(child_fd, signal.SIGTERM)
        child.wait(timeout=10)
        event('original-bootstrap-daemon-finally-normal-termination', actualExitCode=child.returncode)
    if canonical_link_created:
        remove_own_symlink(CANONICAL, bootstrap_path)
    if child_fd is not None:
        os.close(child_fd)
    if temporary_root is not None:
        if bootstrap_path.exists():
            assert stat.S_ISSOCK(bootstrap_path.stat().st_mode)
            bootstrap_path.unlink()
        temporary_root.rmdir()

formal_environment = {**os.environ, 'XDG_RUNTIME_DIR': str(RUNTIME),
                      'DBUS_SESSION_BUS_ADDRESS': 'unix:path=' + str(CANONICAL)}
command('formal-user-api-manager-environment-update', ['/usr/bin/systemctl', '--user', 'set-environment',
                                                     'DBUS_SESSION_BUS_ADDRESS=unix:path=' + str(CANONICAL)], formal_environment)
assert drop_file.read_bytes() == drop_bytes
drop_file.unlink()
drop_directory.rmdir()
command('formal-user-api-daemon-reload-after-temporary-dropin-removal', ['/usr/bin/systemctl', '--user', 'daemon-reload'], formal_environment)
assert owner_pid('unix:path=' + str(CANONICAL)) == MANAGER_PID
for name in ['dbus.socket', 'dbus.service']:
    assert digest(USER_UNITS / name) == digest(SOURCE / name)
assert manager_assert()['startTicks'] == MANAGER_TICKS
report['status'] = 'user-bus-restored-with-original-manager-and-official-user-units'
report['officialUnitsByteExact'] = True
report['socketsTargetAutostartLinkExact'] = (wants / 'dbus.socket').resolve() == (USER_UNITS / 'dbus.socket').resolve()
report['temporaryBootstrapDropinRemoved'] = not drop_directory.exists()
report['formalDbusState'] = unit_state('dbus.service', formal_environment)
report['formalSocketState'] = unit_state('dbus.socket', formal_environment)
report['managerFinalIdentity'] = manager_assert()
report['defaultOrchesterUnitStateBeforeTransfer'] = unit_state('orchester-default-remote-7337-20261112-post-reboot.service', formal_environment)
report['defaultOrchesterControllerStillLive'] = identity(320847)
report['defaultOrchesterListenerStillLive'] = identity(320854)
save()
os.close(manager_fd)
print(json.dumps({'status': report['status'], 'safeReceipt': str(REPORT), 'managerPid': MANAGER_PID,
                  'formalDbusPid': report['formalDbusState']['MainPID'],
                  'OrchesterCurrentUnitState': report['defaultOrchesterUnitStateBeforeTransfer']}))
