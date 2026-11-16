#!/usr/bin/env python3
"""Archive selected safe WSL reboot evidence without private state values."""
import datetime
import hashlib
import json
from pathlib import Path
import shutil

ROOT = Path('/home/han/project/Orchester')
ACCEPTANCE = ROOT / 'docs/design/acceptance/agentrouter-platform-sync'
PREFIX = 'deployments/linux/20261116-actual-reboot-autostart'
OUT = ACCEPTANCE / PREFIX
BASE = Path(__file__).resolve().parent
LEGACY = BASE.parent / 'readonly-wsl-boot-autostart-audit-20261116T072805Z'
LIFECYCLE = BASE.parent / 'wsl-current-reboot-autostart-lifecycle-private'
SUCCESS = BASE / 'attempt2'

def sha(data):
    return hashlib.sha256(data).hexdigest()

def decode(path):
    return json.loads(path.read_text())

def write_private(name, value):
    path = BASE / name
    with path.open('x') as stream:
        json.dump(value, stream, indent=2)
        stream.write('\n')
    path.chmod(0o600)
    return path

def main():
    assert not OUT.exists()
    prior_manifest = decode(LEGACY / 'private-audit-manifest.json')
    for item in prior_manifest['files']:
        payload = (LEGACY / item['path']).read_bytes()
        assert len(payload) == item['bytes'] and sha(payload) == item['sha256']
    life_manifest = decode(LIFECYCLE / 'artifact-manifest-safe.json')
    for item in life_manifest['files']:
        payload = (LIFECYCLE / item['path']).read_bytes()
        assert len(payload) == item['bytes'] and sha(payload) == item['sha256']
    first_source = (BASE / 'observe-readonly.py').read_bytes()
    second_source = (SUCCESS / 'observe-readonly.py').read_bytes()
    old = b"http.open('http://127.0.0.1:7337/info', timeout=10)"
    new = b"http.open('http://127.0.0.1:7337/remote/v1/info', timeout=10)"
    assert first_source.count(old) == 1 and first_source.replace(old, new) == second_source
    state = decode(SUCCESS / 'original-logical-state-retention-safe.json')
    assert state['originalRunCount'] == state['currentRunCount'] == 5
    assert state['originalPersistedAuthEntryCount'] == state['currentPersistedAuthEntryCount'] == 5
    assert state['allOriginalRunRowsByteExact'] and state['allOriginalAuthJsonExactlyEqual']
    assert state['allOriginalRemoteStateJsonExactlyEqual']
    assert decode(SUCCESS / 'observer-result-safe.json')['status'] == 'passed'
    life = decode(LIFECYCLE / 'summary-safe.json')
    current = decode(SUCCESS / 'current-process-and-source-safe.json')
    assert life['kernelBootId'] == current['kernelBootId'] == state['kernelBootId']
    assert life['listenerPid'] == current['listener']['pid']
    assert life['listenerStartTicks'] == current['listener']['startTicks']
    assert life['typedBootStartupCausalChainProved'] is True
    execution = write_private('execution-provenance-safe.json', {
        'schemaVersion': 1,
        'provenance': 'Actual completed exec_command tool results observed by the executing agent; these values are not inferred from script assertions.',
        'initialReadonlyObserver': {
            'actualProcessExitCode': 1, 'processExited': True, 'toolChunkId': '3e50f6',
            'sourceSha256': sha(first_source), 'failureSourceLine': 166,
            'failureType': 'AssertionError', 'dataRetentionComparisonReached': False,
            'diagnosis': 'Initial anonymous HTTP probe omitted /remote/v1; separate read-only diagnosis returned HTTP 404 for /info and /api/info.'},
        'correctedReadonlyObserver': {
            'actualProcessExitCode': 0, 'processExited': True, 'toolChunkId': 'f35d24',
            'sourceSha256': sha(second_source), 'onlySourceChange': 'anonymous endpoint /info -> /remote/v1/info',
            'singleEndpointReplacementReproducesExactCorrectedSourceBytes': True},
        'independentCurrentBootLifecycleObserver': {
            'actualProcessExitCode': 0, 'processExited': True,
            'sourceSha256': sha((LIFECYCLE / 'current-boot-lifecycle-source.py').read_bytes()),
            'provenance': 'Actual completed tool result reported by the independent lifecycle observer agent.'},
        'priorBootAudit': {'sourceFilesHashAndSizeVerified': True, 'retrospectiveNativeProcessExitCodeClaimed': False}})
    summary = write_private('reboot-and-retention-summary-safe.json', {
        'schemaVersion': 1, 'status': 'actual-wsl-reboot-autostart-and-original-logical-state-verified',
        'observedAtUtc': state['observedAtUtc'], 'sourceRevision': current['listenerSourceRevision'],
        'kernelBootId': current['kernelBootId'], 'priorObservedKernelBootId': current['priorObservedKernelBootId'],
        'originalArchivedKernelBootId': current['originalArchivedKernelBootId'],
        'actualWslKernelBootTransitionVerified': True,
        'sameEnabledNamedUnitAutostartDuringUserManagerDefaultTargetBootVerified': True,
        'systemdManagerPid': life['systemdManagerPid'], 'controllerPid': life['controllerPid'],
        'listenerPid': life['listenerPid'], 'listenerStartTicks': life['listenerStartTicks'],
        'listenerExecutableSha256': current['listener']['executableSha256'],
        'originalUnitAndControllerSourceBytesExact': True,
        'originalEnvironmentSnapshotHashExact': True,
        'originalRunCount': 5, 'currentRunCount': 5,
        'originalPersistedAuthEntryCount': 5, 'currentPersistedAuthEntryCount': 5,
        'originalRunRowsByteExact': True, 'originalAuthAndAllRemoteStateJsonExact': True,
        'originalRemoteStateRawRowsExact': state['allOriginalRemoteStateRawRowsExactlyEqual'],
        'wholeDatabaseByteIdentityClaimed': False, 'existingConsistentBackupComparisons': len(state['existingPrivateBackupComparisons']),
        'current17PublicResourcesHttp200AndHashesExact': True, 'unauthenticatedInfoHttpStatus': 401,
        'currentWindowsPhoneOriginalCredentialTokenWireVerified': False,
        'windowsOperatingSystemRebootVerified': False,
        'auditRequestedOrPerformedReboot': False, 'serviceStartStopRestartEnableReloadCommands': 0,
        'processSignals': 0, 'pairingOrModelInvocations': 0,
        'databaseTokenRowOrEnvironmentValuesPublished': False,
        'scopeBoundary': 'WSL kernel reboot, named-unit automatic startup, retained Linux state and public service resources. Windows original-phone credential retrieval and its authenticated request to this new instance remain a separate test.'})
    readme = BASE / 'README.md'
    readme.write_text('''# Actual WSL reboot, automatic startup, and retained original state

The original named user service automatically started on two later observed WSL
kernel boots. The historical audit at 07:28 UTC records boot `ce7a729b…`; the
current independent lifecycle audit and state audit record boot `04a4390b…`.
Both differ from the archived November 12 boot `d67ae493…`. The current journal
contains typed `start` / `done` records from the same live user manager: Basic
System at 2,846,295 microseconds, the named service at 2,846,345, and default.target
at 2,846,357. Current PID ancestry is manager 266 → controller 293 → listener 427
(birth ticks 342). The original enabled unit, controller source, runtime binary,
and private environment snapshot remain hash-identical to their archived bindings.
Raw wall-clock and kernel btime observations are retained independently; the causal
claim uses boot identity and monotonic journal ordering.

The state audit binds its original private consistent backup to the SHA-256 of
the November 13 task's producer receipt already referenced by the public archive.
It compares that baseline, all six existing consistent backups, and a stable copy
of the current database. The five original run rows remain byte-identical; all
five persisted authentication entries and every remote-state JSON value remain
logically equal. Raw JSON serialization and whole SQLite file bytes differ; those
differences are preserved and are not reported as data loss or byte preservation.
SQLite opened only new private scratch copies. Original DB/WAL bytes stayed
stable, original SHM was not read or copied, and the private scratch was removed.
The archive contains aggregate digests and counts, without database files, rows,
tokens, environment values, or private database paths in the safe results.

All 17 public runtime resources returned HTTP 200 and matched the original build
hashes and current mobile dist bytes. Anonymous `/remote/v1/info` returned 401.
The first readonly observer accidentally used `/info` and exited 1 at the HTTP
gate before state comparison. Its unchanged source and failure are retained.
The corrected observer changed only that endpoint and completed with actual exit
0. `execution-provenance-safe.json` records both outcomes and the independent
lifecycle observer's actual exit 0; no retroactive exit code is assigned to the
older 07:28 evidence.

This proves the WSL reboot/autostart and original Linux logical-state gates. It
does not prove a Windows OS reboot or the original Windows phone's current
NativeCredentials.get and authenticated request to the new Linux instance.
No reboot, service lifecycle operation, signal, pairing, or model call was issued
by these readonly audits. Scripts are archived as executed sources; their output
directories were private task-owned directories, not this public evidence folder.
''')
    readme.chmod(0o600)
    selected = []
    for item in prior_manifest['files']:
        selected.append((LEGACY / item['path'], 'prior-boot-0728/' + item['path']))
    selected.append((LEGACY / 'private-audit-manifest.json', 'prior-boot-0728/source-manifest-safe.json'))
    for item in life_manifest['files']:
        selected.append((LIFECYCLE / item['path'], 'current-boot/' + item['path']))
    selected.append((LIFECYCLE / 'artifact-manifest-safe.json', 'current-boot/artifact-manifest-safe.json'))
    for name in ['observe-readonly.py', 'private-environment-current-hash-safe.json', 'current-public-resources-safe.json',
                 'original-logical-state-retention-safe.json', 'current-process-and-source-safe.json', 'observer-result-safe.json']:
        selected.append((SUCCESS / name, 'current-state/' + name))
    for name in ['observe-readonly.py', 'observer-failure-safe.json', 'private-environment-current-hash-safe.json']:
        selected.append((BASE / name, 'initial-readonly-attempt/' + name))
    selected += [(execution, execution.name), (summary, summary.name), (readme, readme.name),
                 (Path(__file__).resolve(), 'sources/assemble-safe-archive.py')]
    records = []
    for source, destination in selected:
        data = source.read_bytes()
        target = OUT / destination
        target.parent.mkdir(parents=True, exist_ok=True)
        with target.open('xb') as stream:
            stream.write(data)
        assert target.read_bytes() == data
        if target.suffix == '.json':
            json.loads(data)
        records.append({'sourcePath': str(source), 'sourceSha256': sha(data), 'sourceBytes': len(data),
                        'archivePath': PREFIX + '/' + destination, 'archiveSha256': sha(data),
                        'archiveBytes': len(data), 'derivation': False})
    mapping = {'schemaVersion': 1, 'status': 'selected-safe-artifacts-byte-exact',
               'archivePrefix': PREFIX + '/', 'files': records,
               'allSelectedSourceAndArchiveBytesExactlyEqual': True,
               'privateDatabaseEnvironmentAndTokenValuesArchived': False,
               'sourceManifestsVerified': True, 'initialFailedAttemptPreserved': True}
    mapping_path = write_private('source-mappings-safe.json', mapping)
    shutil.copyfile(mapping_path, OUT / mapping_path.name)
    assert (OUT / mapping_path.name).read_bytes() == mapping_path.read_bytes()
    print(json.dumps({'status': 'archived', 'fileCount': len(records) + 1,
                      'archivePrefix': PREFIX, 'mappingSha256': sha(mapping_path.read_bytes())}))

if __name__ == '__main__':
    main()
