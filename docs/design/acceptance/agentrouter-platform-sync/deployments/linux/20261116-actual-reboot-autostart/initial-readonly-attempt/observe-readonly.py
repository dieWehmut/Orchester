#!/usr/bin/env python3
"""Read-only source, process, public HTTP and private SQLite retention audit.

Private database bytes are copied to an owned scratch directory after stable
reads. SQLite opens only those copies. No original SHM, credentials, process
environment or command line is read; only aggregate state digests are emitted.
"""
import datetime
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import stat
import subprocess
import sys
import tempfile
import traceback
from urllib.error import HTTPError
from urllib.request import build_opener, ProxyHandler

ROOT = Path('/home/han/project/Orchester')
ACCEPTANCE = ROOT / 'docs/design/acceptance/agentrouter-platform-sync'
LEGACY = Path('/home/han/.local/state/orchester-delivery/20261116/readonly-wsl-boot-autostart-audit-20261116T072805Z')
CONTROL = Path('/home/han/.local/state/orchester-delivery/20261113/linux-service-recovery/persistent-controller')
STATE = Path('/home/han/.local/state/orchester-remote/state')
OUT = Path(__file__).resolve().parent
UNIT = 'orchester-default-remote-7337-20261112-post-reboot.service'
REVISION = '71def643cb08b208cc79687f79255c504eb14dc6'
ELF_SHA = 'd983bceea7c0797c995c7ffb038eaa984323ee1106293da18433f2a3f22d6649'
OLD_GROUP = 'deployments/linux/20261113-userbus-managed-default/'

def sha(data):
    return hashlib.sha256(data).hexdigest()

def file_sha(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for block in iter(lambda: f.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()

def stamp(s):
    return s.st_dev, s.st_ino, s.st_size, s.st_mtime_ns, s.st_ctime_ns

def stable_bytes(path):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd, 'rb') as f:
        before = os.fstat(f.fileno())
        assert stat.S_ISREG(before.st_mode)
        data = f.read()
        after = os.fstat(f.fileno())
    assert stamp(before) == stamp(after) == stamp(path.stat())
    assert len(data) == before.st_size
    return data

def write(name, value):
    path = OUT / name
    with path.open('x') as f:
        json.dump(value, f, indent=2)
        f.write('\n')
    path.chmod(0o600)

def proc(pid):
    p = Path('/proc') / str(pid)
    fields = (p / 'stat').read_text().split(') ', 1)[1].split()
    return {'pid': pid, 'ppid': int(fields[1]), 'startTicks': int(fields[19]),
            'state': fields[0], 'uid': p.stat().st_uid,
            'cgroup': (p / 'cgroup').read_text().strip(),
            'executableSha256': file_sha(p / 'exe')}

def unit_properties():
    names = ['ActiveState', 'SubState', 'MainPID', 'InvocationID', 'ActiveEnterTimestampMonotonic', 'UnitFileState']
    command = ['systemctl', '--user', 'show', UNIT]
    for name in names:
        command += ['-p', name]
    result = subprocess.run(command, capture_output=True, text=True, timeout=15)
    assert result.returncode == 0 and not result.stderr
    return dict(line.split('=', 1) for line in result.stdout.splitlines())

def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode('utf-8')

def logical(copied):
    # The source here is always inside our new scratch directory, never live.
    with sqlite3.connect(copied.as_uri() + '?mode=ro', uri=True) as db:
        db.execute('PRAGMA query_only=ON')
        db.execute('BEGIN')
        assert db.execute('PRAGMA quick_check').fetchall() == [('ok',)]
        runs = db.execute('SELECT workspace,id,value FROM remote_runs ORDER BY workspace,id').fetchall()
        rows = db.execute('SELECT key,value FROM remote_state ORDER BY key').fetchall()
    parsed = {k: json.loads(v) for k, v in rows}
    auth = {k: v for k, v in parsed.items() if k.startswith('auth:')}
    count = sum(len(v['tokens']) for v in auth.values())
    return {'runs': runs, 'rows': rows, 'parsed': parsed, 'auth': auth,
            'authCount': count, 'runDigest': sha(canonical(runs)),
            'stateDigest': sha(canonical(parsed)), 'authDigest': sha(canonical(auth))}

def main():
    os.umask(0o077)
    observed = datetime.datetime.now(datetime.timezone.utc).isoformat()
    boot = Path('/proc/sys/kernel/random/boot_id').read_text().strip()
    legacy_boot = json.loads((LEGACY / 'boot-and-current-process-safe.json').read_text())
    assert boot != legacy_boot['kernelBootId'] != legacy_boot['archivedKernelBootId']
    properties = unit_properties()
    assert properties['ActiveState'] == 'active' and properties['SubState'] == 'running'
    assert properties['UnitFileState'] == 'enabled'
    controller = proc(int(properties['MainPID']))
    manager = proc(controller['ppid'])
    children = [int(x) for x in (Path('/proc') / str(controller['pid']) / 'task' / str(controller['pid']) / 'children').read_text().split()]
    matching = [proc(pid) for pid in children if file_sha(Path('/proc') / str(pid) / 'exe') == ELF_SHA]
    assert len(matching) == 1
    listener = matching[0]
    expected_cgroup = '0::/user.slice/user-1000.slice/user@1000.service/app.slice/' + UNIT
    assert controller['cgroup'] == listener['cgroup'] == expected_cgroup
    assert listener['ppid'] == controller['pid'] and all(x['uid'] == os.getuid() for x in [controller, manager, listener])
    listeners = []
    for table in ['tcp', 'tcp6']:
        for line in (Path('/proc/net') / table).read_text().splitlines()[1:]:
            f = line.split()
            if int(f[1].rsplit(':', 1)[1], 16) == 7337 and f[3] == '0A':
                listeners.append({'table': table, 'inode': int(f[9]), 'uid': int(f[7]), 'port': 7337})
    assert len(listeners) == 1
    socket_name = 'socket:[' + str(listeners[0]['inode']) + ']'
    fds = [int(p.name) for p in (Path('/proc') / str(listener['pid']) / 'fd').iterdir() if os.readlink(p) == socket_name]
    assert len(fds) >= 1
    listeners[0].update(ownerPid=listener['pid'], ownerStartTicks=listener['startTicks'], ownerFileDescriptors=fds)
    source = json.loads((LEGACY / 'source-unit-enable-link-bindings-safe.json').read_text())
    unit_path = Path.home() / '.config/systemd/user' / UNIT
    controller_path = CONTROL / 'controller-successor.py'
    unit_bytes = stable_bytes(unit_path)
    controller_bytes = stable_bytes(controller_path)
    assert sha(unit_bytes) == source['unitSha256'] and sha(controller_bytes) == source['controllerSha256']
    assert unit_bytes == (ROOT / source['unitArchiveReference']).read_bytes()
    assert controller_bytes == (ROOT / source['controllerArchiveReference']).read_bytes()
    enabled = unit_path.parent / 'default.target.wants' / UNIT
    assert enabled.is_symlink() and enabled.resolve() == unit_path.resolve()
    environment = CONTROL / 'environment-current-private.json'
    env = stable_bytes(environment)
    env_expected = json.loads((LEGACY / 'private-environment-hash-comparison-safe.json').read_text())['objects']['currentEnvironmentSnapshot']
    assert sha(env) == env_expected['sha256'] and len(env) == env_expected['bytes']
    assert stat.S_IMODE(environment.stat().st_mode) == 0o600
    env_sha = sha(env)
    del env
    write('private-environment-current-hash-safe.json', {
        'schemaVersion': 1, 'observedAtUtc': observed, 'sha256': env_sha,
        'bytes': env_expected['bytes'], 'mode': '0o600', 'originalArchivedEnvironmentHashEqual': True,
        'valuesParsedOrPublished': False, 'runtimeHeapEnvironmentCompared': False})

    expected = json.loads((LEGACY / 'current-public-resources-safe.json').read_text())['resources']
    http = build_opener(ProxyHandler({}))
    resources = []
    for item in expected:
        path = item['path']
        assert not path.startswith('/') and '..' not in Path(path).parts
        with http.open('http://127.0.0.1:7337/' + path, timeout=20) as response:
            data = response.read()
            status_code = response.status
        assert status_code == 200 and sha(data) == item['sha256']
        assert data == stable_bytes(ROOT / 'apps/mobile/dist' / path)
        resources.append({'path': path, 'status': status_code, 'bytes': len(data), 'sha256': sha(data), 'currentDistAndArchivedHashExact': True})
    assert len(resources) == 17
    try:
        response = http.open('http://127.0.0.1:7337/info', timeout=10)
    except HTTPError as error:
        assert error.code == 401
        error.close()
    else:
        response.close()
        raise AssertionError('protected route accepted anonymous request')
    write('current-public-resources-safe.json', {
        'schemaVersion': 1, 'observedAtUtc': observed, 'sourceRevision': REVISION,
        'kernelBootId': boot, 'listenerPid': listener['pid'], 'listenerStartTicks': listener['startTicks'],
        'resources': resources, 'all17ResourcesExact': True, 'unauthenticatedInfoStatus': 401,
        'protectedReplyBodyRead': False, 'authenticatedRequests': 0})

    producer_path = CONTROL.parent / 'first-managed-start/producer-final-safe.json'
    producer_bytes = stable_bytes(producer_path)
    public_baseline = ACCEPTANCE / OLD_GROUP / 'first-managed-start-producer-public-safe.json'
    historical = json.loads(public_baseline.read_text())
    assert sha(producer_bytes) == historical['publicDerivation']['sourceSha256']
    assert len(producer_bytes) == historical['publicDerivation']['sourceBytes']
    producer = json.loads(producer_bytes)
    original_db = Path(producer['privateBackupDirectory']) / 'state/remote.db'
    assert original_db.resolve().is_relative_to(CONTROL.resolve())
    original_bytes = stable_bytes(original_db)
    backup_record = next(x for x in producer['backupFiles'] if x['path'] == 'state/remote.db')
    assert sha(original_bytes) == backup_record['backupSha256']
    assert len(original_bytes) == backup_record['backupBytes']
    current_receipt_bytes = stable_bytes(CONTROL / 'final-safe.json')
    current_receipt = json.loads(current_receipt_bytes)
    assert current_receipt['status'] == 'deployed-and-verified'
    assert current_receipt['pid'] == listener['pid'] and current_receipt['controllerPid'] == controller['pid']
    assert current_receipt['consistentBackupRowsExactlyEqual'] is True
    current_backup_db = Path(current_receipt['privateBackupDirectory']) / 'state/remote.db'
    assert current_backup_db.resolve().is_relative_to(CONTROL.resolve())
    current_backup_bytes = stable_bytes(current_backup_db)
    current_backup_record = next(x for x in current_receipt['backupFiles'] if x['path'] == 'state/remote.db')
    assert sha(current_backup_bytes) == current_backup_record['backupSha256']
    database_paths = [STATE / 'remote.db']
    if (STATE / 'remote.db-wal').exists():
        database_paths.append(STATE / 'remote.db-wal')
    originals = {p.name: stable_bytes(p) for p in database_paths}
    retained = []
    with tempfile.TemporaryDirectory(prefix='sqlite-private-', dir=OUT) as temporary:
        scratch = Path(temporary)
        for name, data in originals.items():
            (scratch / name).write_bytes(data)
        baseline_path = scratch / 'original-baseline.db'
        baseline_path.write_bytes(original_bytes)
        original = logical(baseline_path)
        current = logical(scratch / 'remote.db')
        assert len(original['runs']) == len(current['runs']) == 5
        assert original['authCount'] == current['authCount'] == 5
        assert original['runs'] == current['runs']
        assert original['parsed'] == current['parsed']
        for i, path in enumerate(sorted(CONTROL.glob('private-backup-*/state/remote.db'))):
            data = stable_bytes(path)
            copied = scratch / ('comparison-' + str(i) + '.db')
            copied.write_bytes(data)
            other = logical(copied)
            assert other['runs'] == original['runs'] and other['parsed'] == original['parsed']
            retained.append({'ordinal': i, 'databaseSha256': sha(data), 'databaseBytes': len(data),
                             'runCount': len(other['runs']), 'authEntryCount': other['authCount'],
                             'allOriginalRunRowsByteExact': True, 'allOriginalRemoteStateJsonExact': True})
        assert len(retained) >= 6
    assert all(stable_bytes(p) == originals[p.name] for p in database_paths)
    assert (STATE / 'remote.db-wal').exists() == ('remote.db-wal' in originals)
    assert stable_bytes(original_db) == original_bytes
    assert stable_bytes(current_backup_db) == current_backup_bytes
    assert stable_bytes(CONTROL / 'final-safe.json') == current_receipt_bytes
    write('original-logical-state-retention-safe.json', {
        'schemaVersion': 1, 'observedAtUtc': observed, 'kernelBootId': boot,
        'baselinePublicReceipt': OLD_GROUP + 'first-managed-start-producer-public-safe.json',
        'baselinePublicReceiptSha256': file_sha(public_baseline),
        'baselinePrivateProducerReceiptSha256': sha(producer_bytes),
        'baselinePrivateProducerMatchesArchivedDerivation': True,
        'baselineDatabaseSha256': sha(original_bytes), 'baselineDatabasePinnedToProducer': True,
        'currentStartupReceiptSha256': sha(current_receipt_bytes),
        'currentStartupReceiptMatchesCurrentControllerAndListener': True,
        'currentStartupBackupDatabaseSha256': sha(current_backup_bytes),
        'currentStartupBackupMatchesProducerReceipt': True,
        'currentMainDatabaseSha256': sha(originals['remote.db']),
        'sourceDatabaseAndWalBytesStableBeforeAndAfterComparison': True,
        'originalSharedMemoryReadOrCopied': False,
        'sqliteOpenedOnlyOwnedPrivateCopies': True, 'privateScratchRemoved': True,
        'originalRunCount': len(original['runs']), 'currentRunCount': len(current['runs']),
        'originalPersistedAuthEntryCount': original['authCount'], 'currentPersistedAuthEntryCount': current['authCount'],
        'originalAndCurrentRunRowsAggregateSha256': original['runDigest'],
        'originalAndCurrentAuthJsonAggregateSha256': original['authDigest'],
        'originalAndCurrentAllRemoteStateJsonAggregateSha256': original['stateDigest'],
        'allOriginalRunRowsByteExact': original['runs'] == current['runs'],
        'allOriginalAuthJsonExactlyEqual': original['auth'] == current['auth'],
        'allOriginalRemoteStateJsonExactlyEqual': original['parsed'] == current['parsed'],
        'allOriginalRemoteStateRawRowsExactlyEqual': original['rows'] == current['rows'],
        'existingPrivateBackupComparisons': retained,
        'rowValuesTokenValuesOrPrivateDatabasePathsPublished': False,
        'newPhoneTokenAuthenticatedHttpVerified': False})
    assert sha(stable_bytes(environment)) == env_sha
    assert stable_bytes(unit_path) == unit_bytes and stable_bytes(controller_path) == controller_bytes
    assert enabled.resolve() == unit_path.resolve()
    assert unit_properties() == properties
    for item in [controller, manager, listener]:
        assert proc(item['pid']) == item
    assert Path('/proc/sys/kernel/random/boot_id').read_text().strip() == boot
    write('current-process-and-source-safe.json', {
        'schemaVersion': 1, 'observedAtUtc': observed, 'kernelBootId': boot,
        'priorObservedKernelBootId': legacy_boot['kernelBootId'],
        'originalArchivedKernelBootId': legacy_boot['archivedKernelBootId'],
        'threeDistinctKernelBootIdentities': True, 'unit': UNIT, 'unitProperties': properties,
        'controller': controller, 'manager': manager, 'listener': listener, 'sole7337Listener': listeners[0],
        'unitCurrentBytesExactArchived': True, 'unitSha256': sha(unit_bytes),
        'controllerCurrentBytesExactArchived': True, 'controllerSha256': sha(controller_bytes),
        'defaultTargetEnableLinkExact': True, 'processSourceUnitEnvironmentAndBootStableThroughAudit': True,
        'listenerSourceRevision': REVISION, 'observedUnitPropertiesNotRebootCauseByThemselves': True,
        'processSignals': 0, 'serviceStartStopRestartEnableReloadCommands': 0,
        'requestedOrPerformedReboot': False, 'modelPairingOrCredentialRequests': 0})
    write('observer-result-safe.json', {
        'schemaVersion': 1, 'status': 'passed', 'observedAtUtc': observed,
        'finishedAtUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'sourceSha256': file_sha(Path(__file__)), 'allOriginalRun5Auth5LogicalStateRetained': True,
        'all17CurrentPublicResourcesExact': True, 'currentBootAndProcessSourceBindingsStable': True,
        'originalPhoneCurrentTokenWireVerified': False, 'runtimeProcessOrServiceMutation': False})
    print(json.dumps({'status': 'passed', 'listenerPid': listener['pid'], 'listenerStartTicks': listener['startTicks'],
                      'kernelBootId': boot, 'originalRunCount': 5, 'originalAuthEntryCount': 5, 'existingBackupCount': len(retained)}))

if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        safe = {'schemaVersion': 1, 'status': 'failed', 'errorType': type(error).__name__,
                'sourceLine': traceback.extract_tb(error.__traceback__)[-1].lineno,
                'privateValuesPublished': False}
        write('observer-failure-safe.json', safe)
        print(json.dumps(safe))
        raise SystemExit(1)
