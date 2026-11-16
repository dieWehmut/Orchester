#!/usr/bin/env python3
"""Read only the current boot's selected unit lifecycle and process metadata.

The audit never reads process environments, command lines, service stdout/stderr,
database contents, authentication material, or unrelated journal message text.
It invokes no service lifecycle operation and sends no signals. Its only writes
are exclusive new evidence files beside this source, in a private directory.
"""
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import stat
import subprocess
import sys

UNIT = 'orchester-default-remote-7337-20261112-post-reboot.service'
EXPECTED_BOOT = '04a4390b-1b73-42f2-b3ae-906308618b42'
EXPECTED_PID = 293
EXPECTED_INVOCATION = '5ea55369162d46b085132afbb956c25f'
EXPECTED_ACTIVE_MONOTONIC = 2836055
SOURCE_REVISION = '71def643cb08b208cc79687f79255c504eb14dc6'
UNIT_ARCHIVE = 'docs/design/acceptance/agentrouter-platform-sync/deployments/linux/20261113-userbus-managed-default/runtime/default-remote-7337.service'
CONTROLLER_ARCHIVE = 'docs/design/acceptance/agentrouter-platform-sync/deployments/linux/20261113-userbus-managed-default/runtime/controller-successor.py'
EXPECTED_UNIT_SHA = '3119c552d11c49ca97c9db018aec53af82aa30adcde2a367e3b9479ea438f35c'
EXPECTED_CONTROLLER_SHA = 'b70fcda2c9df0e9e214204b46297849fe4ebcbd0a5f01766586c5e26d6686c4b'
EXPECTED_SERVER_SHA = 'd983bceea7c0797c995c7ffb038eaa984323ee1106293da18433f2a3f22d6649'
OUT = Path(__file__).resolve().parent
REPO = Path.home() / 'project/Orchester'
PRIOR = Path.home() / '.local/state/orchester-delivery/20261116/readonly-wsl-boot-autostart-audit-20261116T072805Z/boot-and-current-process-safe.json'


def now():
    return dt.datetime.now(dt.timezone.utc).isoformat()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def file_sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def require(condition, label):
    if not condition:
        raise RuntimeError(label)


def command(argv):
    result = subprocess.run(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30, check=False)
    require(result.returncode == 0, 'read_only_command_failed')
    require(not result.stderr, 'read_only_command_stderr_present')
    return result.stdout


def properties(unit, names, user=True):
    argv = ['systemctl'] + (['--user'] if user else []) + ['show', unit]
    argv += ['--property=' + key for key in names]
    text = command(argv).decode('utf-8')
    rows = dict(line.split('=', 1) for line in text.splitlines() if '=' in line)
    require(set(rows) == set(names), 'unit_property_set_mismatch')
    return rows


def process(pid):
    raw = (Path('/proc') / str(pid) / 'stat').read_text()
    fields = raw[raw.rfind(')') + 2:].split()
    return {
        'pid': pid, 'ppid': int(fields[1]), 'startTicks': int(fields[19]),
        'state': fields[0],
        'cgroup': (Path('/proc') / str(pid) / 'cgroup').read_text().strip(),
        'exeSha256': file_sha(Path('/proc') / str(pid) / 'exe'),
    }


def save(name, value):
    payload = (json.dumps(value, indent=2, ensure_ascii=False) + '\n').encode()
    fd = os.open(OUT / name, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, 'wb') as stream:
        stream.write(payload)
    return {'path': name, 'bytes': len(payload), 'sha256': digest(payload)}


def audit():
    observed = now()
    require(stat.S_IMODE(OUT.stat().st_mode) == 0o700, 'output_directory_mode_mismatch')
    boot_path = Path('/proc/sys/kernel/random/boot_id')
    boot = boot_path.read_text().strip()
    require(boot == EXPECTED_BOOT, 'expected_boot_changed')
    prior_bytes = PRIOR.read_bytes()
    prior = json.loads(prior_bytes)
    prior_boot = prior['kernelBootId']
    require(prior_boot != boot, 'prior_boot_not_distinct')
    btime = next(int(line.split()[1]) for line in Path('/proc/stat').read_text().splitlines() if line.startswith('btime '))
    names = ['Id', 'MainPID', 'ActiveState', 'SubState', 'UnitFileState', 'ActiveEnterTimestampMonotonic',
             'ExecMainStartTimestampMonotonic', 'InvocationID', 'ControlGroup', 'FragmentPath']
    unit = properties(UNIT, names)
    require(unit['Id'] == UNIT and unit['MainPID'] == str(EXPECTED_PID), 'expected_unit_identity_changed')
    require(unit['InvocationID'] == EXPECTED_INVOCATION, 'expected_unit_invocation_changed')
    require(int(unit['ActiveEnterTimestampMonotonic']) == EXPECTED_ACTIVE_MONOTONIC, 'expected_unit_active_time_changed')
    require((unit['ActiveState'], unit['SubState'], unit['UnitFileState']) == ('active', 'running', 'enabled'), 'unit_not_enabled_running')
    target = properties('default.target', ['ActiveState', 'SubState', 'ActiveEnterTimestampMonotonic', 'InvocationID'])
    manager = properties('user@1000.service', ['MainPID', 'ActiveState', 'SubState', 'InvocationID', 'ExecMainStartTimestampMonotonic'], user=False)
    manager_pid = int(manager['MainPID'])
    manager_proc = process(manager_pid)
    controller_proc = process(EXPECTED_PID)
    require(controller_proc['ppid'] == manager_pid, 'controller_parent_is_not_user_manager')
    require(manager_proc['ppid'] == 1, 'manager_parent_is_not_init')
    require(manager_proc['cgroup'] == '0::/user.slice/user-1000.slice/user@1000.service/init.scope', 'manager_cgroup_mismatch')
    require(controller_proc['cgroup'] == '0::' + unit['ControlGroup'], 'controller_cgroup_mismatch')

    unit_bytes = Path(unit['FragmentPath']).read_bytes()
    require(digest(unit_bytes) == EXPECTED_UNIT_SHA, 'unit_source_hash_changed')
    require(unit_bytes == (REPO / UNIT_ARCHIVE).read_bytes(), 'unit_archive_bytes_differ')
    directives = [line.strip() for line in unit_bytes.decode().splitlines() if line.strip() and not line.lstrip().startswith(('#', ';'))]
    exec_rows = [line[len('ExecStart='):] for line in directives if line.startswith('ExecStart=')]
    require(len(exec_rows) == 1, 'unit_exec_source_ambiguous')
    source_paths = [Path(part) for part in shlex.split(exec_rows[0]) if part.endswith('.py')]
    require(len(source_paths) == 1, 'controller_source_ambiguous')
    source = source_paths[0]
    source_bytes = source.read_bytes()
    require(digest(source_bytes) == EXPECTED_CONTROLLER_SHA, 'controller_source_hash_changed')
    require(source_bytes == (REPO / CONTROLLER_ARCHIVE).read_bytes(), 'controller_archive_bytes_differ')
    link = Path.home() / '.config/systemd/user/default.target.wants' / UNIT
    require(link.is_symlink() and link.resolve() == Path(unit['FragmentPath']).resolve(), 'enabled_target_link_mismatch')

    permitted = ['__REALTIME_TIMESTAMP', '__MONOTONIC_TIMESTAMP', '_BOOT_ID', '_PID', '_COMM',
                 '_SYSTEMD_UNIT', '_SYSTEMD_USER_UNIT', '_SYSTEMD_CGROUP', 'USER_UNIT',
                 'MESSAGE_ID', 'JOB_TYPE', 'JOB_RESULT', 'JOB_ID']
    raw_journal = command(['journalctl', '--user', '--boot=' + boot.replace('-', ''), '_PID=' + str(manager_pid),
                           '_COMM=systemd', '--output=json', '--no-pager', '--quiet',
                           '--output-fields=' + ','.join(permitted + ['MESSAGE'])])
    records = [json.loads(row) for row in raw_journal.splitlines() if row]
    targets = ['basic.target', UNIT, 'default.target']
    selected = []
    safe_messages = {
        'basic.target': 'Reached target Basic System.',
        UNIT: 'Started Orchester persistent default remote 7337 owned controller.',
        'default.target': 'Reached target Main User Target.',
    }
    for target_name in targets:
        matches = [row for row in records if row.get('USER_UNIT') == target_name]
        require(len(matches) == 1, 'lifecycle_record_missing_or_repeated')
        row = matches[0]
        require(row.get('_BOOT_ID') == boot.replace('-', '') and row.get('_PID') == str(manager_pid)
                and row.get('_COMM') == 'systemd', 'journal_boot_manager_mismatch')
        require(row.get('JOB_TYPE') == 'start' and row.get('JOB_RESULT') == 'done'
                and row.get('MESSAGE_ID') == '39f53479d3a045ac8e11786248231fbf', 'typed_unit_start_not_done')
        require(row.get('MESSAGE') == safe_messages[target_name], 'lifecycle_message_not_allowlisted')
        safe = {key: row[key] for key in permitted if key in row}
        safe['safeLifecycleMessage'] = safe_messages[target_name]
        safe['messageSha256'] = digest(row['MESSAGE'].encode())
        selected.append(safe)
    monotonic = [int(row['__MONOTONIC_TIMESTAMP']) for row in selected]
    require(monotonic[0] < monotonic[1] < monotonic[2], 'boot_lifecycle_order_not_strict')
    require(int(unit['ActiveEnterTimestampMonotonic']) <= monotonic[1], 'unit_started_after_journal')
    require(int(target['ActiveEnterTimestampMonotonic']) <= monotonic[2], 'target_started_after_journal')
    require(target['ActiveState'] == 'active', 'default_target_not_active')

    # Only the named unit's cgroup is examined for socket ownership.
    pids = [int(pid) for pid in (Path('/sys/fs/cgroup') / unit['ControlGroup'].lstrip('/') / 'cgroup.procs').read_text().split()]
    unit_processes = {pid: process(pid) for pid in pids}
    require(EXPECTED_PID in unit_processes, 'controller_missing_from_unit_cgroup')
    listeners = []
    for table in ('tcp', 'tcp6'):
        for row in (Path('/proc/net') / table).read_text().splitlines()[1:]:
            fields = row.split()
            if int(fields[1].rsplit(':', 1)[1], 16) != 7337 or fields[3] != '0A':
                continue
            inode = int(fields[9])
            owners = []
            for pid in pids:
                for fd in (Path('/proc') / str(pid) / 'fd').iterdir():
                    try:
                        is_socket = os.readlink(fd) == 'socket:[' + str(inode) + ']'
                    except FileNotFoundError:
                        continue
                    if is_socket:
                        owners.append({'pid': pid, 'fd': int(fd.name), 'startTicks': unit_processes[pid]['startTicks']})
            listeners.append({'port': 7337, 'table': table, 'state': 'LISTEN', 'inode': inode, 'uid': int(fields[7]), 'ownersWithinNamedUnit': owners})
    require(len(listeners) == 1 and len(listeners[0]['ownersWithinNamedUnit']) == 1, 'sole_named_unit_listener_not_proved')
    listener_pid = listeners[0]['ownersWithinNamedUnit'][0]['pid']
    listener_proc = unit_processes[listener_pid]
    require(listener_proc['ppid'] == EXPECTED_PID and listener_proc['cgroup'] == controller_proc['cgroup'], 'listener_ancestry_mismatch')
    require(listener_proc['exeSha256'] == EXPECTED_SERVER_SHA, 'listener_executable_hash_changed')
    require(boot_path.read_text().strip() == boot, 'boot_changed_during_read')
    require(properties(UNIT, names) == unit, 'unit_changed_during_read')
    for before in [manager_proc, controller_proc, listener_proc]:
        after = process(before['pid'])
        require(all(before[key] == after[key] for key in ['pid', 'ppid', 'startTicks', 'cgroup', 'exeSha256']), 'process_identity_changed_during_read')
    require(Path(unit['FragmentPath']).read_bytes() == unit_bytes and source.read_bytes() == source_bytes, 'source_changed_during_read')
    require(link.is_symlink() and link.resolve() == Path(unit['FragmentPath']).resolve(), 'enabled_link_changed_during_read')

    artifacts = []
    artifacts.append(save('boot-and-current-process-safe.json', {
        'schemaVersion': 1, 'observedAtUtc': observed, 'completedAtUtc': now(), 'kernelBootId': boot,
        'journalBootId128': boot.replace('-', ''), 'kernelBtime': btime,
        'kernelBtimeUtc': dt.datetime.fromtimestamp(btime, dt.timezone.utc).isoformat(),
        'priorObservedKernelBootId': prior_boot, 'priorObservedAtUtc': prior['observedAtUtc'],
        'priorSafeEvidenceSha256': digest(prior_bytes), 'actualKernelBootIdentityChanged': True,
        'unit': UNIT, 'unitState': {key: value for key, value in unit.items() if key != 'FragmentPath'},
        'defaultTargetState': target, 'systemManagerUnitState': manager,
        'processes': [manager_proc, controller_proc, listener_proc], 'listeners': listeners,
        'unitManagerParentAncestryExact': True, 'clockTicksPerSecond': os.sysconf('SC_CLK_TCK'),
        'allObservedProcessBirthsAndBootStableThroughReadOnlyAudit': True,
        'socketOwnerSearchScope': 'named unit cgroup only; no ownership claim for other processes holding duplicated descriptors',
    }))
    artifacts.append(save('manager-current-boot-lifecycle-safe.json', {
        'schemaVersion': 1, 'observedAtUtc': observed, 'journalQueryExitCode': 0, 'journalQueryStderrBytes': 0,
        'kernelUuidNormalizedToJournalId128': True, 'systemdManagerPid': manager_pid,
        'systemdManagerRecordCount': len(records), 'selectedRecords': selected, 'selectedLifecycleRecordCount': len(selected),
        'unitStartJobDoneInSameBootAfterBasicTargetBeforeMainUserTargetReached': True,
        'unitAndDefaultTargetBootMonotonicMicroseconds': {
            'unitExecMainStarted': int(unit['ExecMainStartTimestampMonotonic']),
            'unitActiveEntered': int(unit['ActiveEnterTimestampMonotonic']), 'basicTargetReachedJournal': monotonic[0],
            'unitStartedJournal': monotonic[1], 'defaultTargetActiveEntered': int(target['ActiveEnterTimestampMonotonic']),
            'defaultTargetReachedJournal': monotonic[2],
        },
        'journalWallTimeAndCurrentProcBtimeNotAssumedAligned': True,
        'journalUnitStartedWallUtcRaw': dt.datetime.fromtimestamp(int(selected[1]['__REALTIME_TIMESTAMP']) / 1000000, dt.timezone.utc).isoformat(),
        'causalBasis': 'same boot ID and live parent user-manager PID; typed unit start/done strictly between basic.target and default.target journal records; unchanged enabled target link and unit/controller source hashes',
        'serviceStdoutOrStderrRead': False, 'rawUnrelatedJournalMessagesReturned': False,
    }))
    artifacts.append(save('source-unit-enable-link-bindings-safe.json', {
        'schemaVersion': 1, 'unitSha256': digest(unit_bytes), 'unitBytes': len(unit_bytes),
        'unitCurrentBytesExactArchived': True, 'unitArchiveReference': UNIT_ARCHIVE, 'defaultTargetEnableLinkExact': True,
        'controllerSha256': digest(source_bytes), 'controllerBytes': len(source_bytes),
        'controllerCurrentBytesExactArchived': True, 'controllerArchiveReference': CONTROLLER_ARCHIVE,
        'environmentFileDirectivePresent': any(line.startswith('EnvironmentFile=') for line in directives),
        'publicReferenceSourceRevision': SOURCE_REVISION, 'sourceAndEnabledLinkStableThroughReadOnlyAudit': True,
    }))
    artifacts.append(save('summary-safe.json', {
        'schemaVersion': 1, 'observedAtUtc': observed, 'status': 'current-boot-autostart-lifecycle-proved',
        'actualKernelBootIdentityChanged': True, 'kernelBootId': boot, 'unit': UNIT,
        'currentInvocationId': unit['InvocationID'], 'controllerPid': EXPECTED_PID, 'systemdManagerPid': manager_pid,
        'listenerPid': listener_pid, 'listenerStartTicks': listener_proc['startTicks'],
        'typedBootStartupCausalChainProved': True, 'unitEnabledActiveRunning': True,
        'exactArchivedUnitAndControllerBytes': True, 'auditRequestedReboot': False, 'auditRequestedServiceLifecycleOperation': False,
        'auditSentSignals': False, 'processEnvironmentRead': False, 'processCommandLineRead': False,
        'databaseContentsRead': False, 'authenticationMaterialRead': False,
        'androidStoredCredentialHttpWireProvedByThisAudit': False,
        'scope': 'real changed kernel boot, same-boot user-manager startup chain, current named unit and listener metadata only',
    }))
    source_data = Path(__file__).read_bytes()
    artifacts.append({'path': Path(__file__).name, 'bytes': len(source_data), 'sha256': digest(source_data)})
    manifest = save('artifact-manifest-safe.json', {'schemaVersion': 1, 'observedAtUtc': now(), 'files': artifacts})
    print(json.dumps({'status': 'passed', 'artifactCountIncludingManifest': len(artifacts) + 1,
                      'manifest': manifest, 'kernelBootId': boot, 'controllerPid': EXPECTED_PID,
                      'managerPid': manager_pid, 'listenerPid': listener_pid, 'lifecycleMonotonicMicroseconds': monotonic}))


if __name__ == '__main__':
    os.umask(0o077)
    try:
        audit()
    except Exception as exc:
        # Never print exception text or process output: it can contain unexpected data.
        gate = str(exc) if isinstance(exc, RuntimeError) and re.fullmatch(r'[a-z0-9_]+', str(exc)) else None
        print(json.dumps({'status': 'failed', 'exceptionType': type(exc).__name__, 'staticGateLabel': gate}))
        sys.exit(1)
