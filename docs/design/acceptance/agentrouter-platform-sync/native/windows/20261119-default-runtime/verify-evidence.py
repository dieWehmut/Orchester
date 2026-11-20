#!/usr/bin/env python3
"""Offline integrity and cross-receipt verification; makes no native state query."""
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def read(relative):
    return json.loads((ROOT / relative).read_text(encoding='utf-8-sig'))


def digest(relative):
    return hashlib.sha256((ROOT / relative).read_bytes()).hexdigest()


def validate_digest_fields(value, label):
    if isinstance(value, dict):
        for key, child in value.items():
            if key.lower().endswith('sha256') and isinstance(child, str):
                assert re.fullmatch('[a-f0-9]{64}', child), f'{label}/{key}'
            validate_digest_fields(child, f'{label}/{key}')
    elif isinstance(value, list):
        for index, child in enumerate(value):
            validate_digest_fields(child, f'{label}/{index}')


for path in sorted(ROOT.rglob('*.json')):
    validate_digest_fields(json.loads(path.read_text(encoding='utf-8-sig')), str(path.relative_to(ROOT)))


manifest = read('manifest-safe.json')
assert manifest['status'] == 'historical-default-runtime-verified-current-final-revalidation-incomplete'
listed = {item['path'] for item in manifest['files']}
actual = {str(path.relative_to(ROOT)) for path in ROOT.rglob('*') if path.is_file()}
assert actual - set(manifest['integrityExcludedPaths']) == listed
for item in manifest['files']:
    assert (ROOT / item['path']).stat().st_size == item['sizeBytes'], item['path']
    assert digest(item['path']) == item['sha256'], item['path']

bindings = read('source-bindings-safe.json')
for item in bindings['files']:
    assert digest(item['path']) == item['sha256'], item['path']
    assert (ROOT / item['path']).stat().st_size == item['sizeBytes']

sha_paths = {}
for path in ROOT.rglob('*'):
    if path.is_file():
        sha_paths.setdefault(hashlib.sha256(path.read_bytes()).hexdigest(), []).append(path)

exit_codes = []
for path in sorted((ROOT / 'executions').glob('*/native-observer-execution-safe.json')):
    receipt = json.loads(path.read_text())
    assert receipt['executorSourceSha256'] in sha_paths
    assert receipt['nativeObserverSourceSha256'] in sha_paths
    assert hashlib.sha256((path.parent / 'native-observer-stdout-safe.log').read_bytes()).hexdigest() == receipt['nativeObserverStdoutSha256']
    assert hashlib.sha256((path.parent / 'native-observer-stderr-safe.log').read_bytes()).hexdigest() == receipt['nativeObserverStderrSha256']
    assert receipt['nativeCommandWorkingDirectory'] == r'C:\Users\han'
    assert receipt['runtimeRestartCount'] == receipt['processSignals'] == receipt['credentialCommands'] == receipt['modelCalls'] == 0
    exit_codes.append(receipt['wslInteropObservedNativePowerShellExitCode'])
assert exit_codes == [1, 2, 1, 1], exit_codes

historical = read('historical-native-tool/bindings-safe.json')
for item in historical['events']:
    assert digest(item['path']) == item['sha256']
    assert (ROOT / item['path']).stat().st_size == item['sizeBytes']
for name in ('defaultDesktop', 'managed60'):
    binding = historical[name]
    launch = read('historical-native-tool/' + binding['launchEvent'])['payload']
    started = read('historical-native-tool/' + binding['launchResult'])['payload']
    poll = read('historical-native-tool/' + binding['pollEvent'])['payload']
    ended = read('historical-native-tool/' + binding['nativeTerminalToolResult'])['payload']
    assert launch['call_id'] == started['call_id']
    assert poll['call_id'] == ended['call_id'] == binding['toolCallId']
    assert digest(binding['sourcePath']) == binding['sourceSha256']
    assert Path(binding['sourcePath']).name in launch['input']
    assert f"session_id:{binding['nativeSessionId']}" in poll['input']
    starts = [json.loads(block['text']) for block in started['output'] if block['text'].startswith('{')]
    assert any(item.get('session_id') == binding['nativeSessionId'] for item in starts)
    finishes = [json.loads(block['text']) for block in ended['output'] if block['text'].startswith('{')]
    terminal = finishes[0]
    assert terminal['exit_code'] == binding['actualExternalNativeExitCode'] == 0
    assert json.loads(terminal['output']) == read(binding['resultPath'])

desktop_binding = historical['defaultDesktop']
assert digest(desktop_binding['invokerSourcePath']) == desktop_binding['invokerSourceSha256']
desktop_launch = read('historical-native-tool/' + desktop_binding['launchEvent'])['payload']['input']
command_match = re.search(r'cmd:("(?:[^"\\]|\\.)*")', desktop_launch)
assert command_match
command = json.loads(command_match.group(1))
source_match = re.search("s=r'''([\\s\\S]*?)'''\\.replace\\('__ROOT__',m\\['windowsRoot'\\]\\)", command)
assert source_match
invoker_bytes = (ROOT / desktop_binding['invokerSourcePath']).read_bytes()
root_match = re.search(rb"\$root='([^']+)'", invoker_bytes)
assert root_match
reconstructed_invoker = source_match.group(1).replace('__ROOT__', root_match.group(1).decode()).encode()
assert reconstructed_invoker == invoker_bytes
assert desktop_binding['sourceSha256'] in invoker_bytes.decode()

managed60 = read(historical['managed60']['resultPath'])
assert managed60['observedSeconds'] == 60.3769552
assert managed60['currentSamples'] == 107
assert managed60['originalRowCount'] == 138
assert managed60['resourceCount'] == 17
assert managed60['currentUnexpectedDescendantObservationEvents'] == 0

gui = read('gui-historical/default-runtime-and-retained-7338-final-safe.json')
assert gui['renderedCanonicalProviderCount'] == 21
assert gui['nativeLoadedThemeMappingCount'] == 42
assert gui['originalSavedRecordApiRetainedCount'] == 213
assert gui['actualNativeGuiExitCode'] == gui['actualNativeUiNodeExitCode'] == gui['actualNativeRetentionNodeExitCode'] == 0
for field, path in {
    'readonlyUiResultSha256': 'gui-historical/default-native-readonly-ui-safe.json',
    'runtimeBeforeRetentionSha256': 'gui-historical/default-runtime-before-retention-safe.json',
    'preflightResultSha256': 'gui-historical/preflight-default-runtime-state-retention-57e77cd9786d37b5-safe.json',
    'retentionResultSha256': 'gui-historical/post-default-runtime-state-retention-a0064efebd258df0-safe.json',
}.items():
    assert gui[field] == digest(path), field

fresh = read('dependencies/fresh-runtime-inputs.json')
expected = {item['path'][4:]: item['sha256'] for item in fresh['remotePackageInventory'] if item['path'].startswith('web/')}
resource_tables = sorted((ROOT / 'managed-stable').glob('*/static-http-resources-safe.json'))
assert len(resource_tables) == 2
for path in resource_tables:
    table = json.loads(path.read_text())
    assert table['resourceCount'] == 17
    assert all(item['httpStatus'] == 200 for item in table['resources'])
    assert {item['path']: item['installedAndHttpSha256'] for item in table['resources']} == expected

latest = read('managed-stable/current-probes/current-stable-probe-c28bfbd2198a407091e801dd8e3e128f-safe.json')
assert latest['observedAtUtc'] == '2026-11-20T01:59:37.4228329Z'
assert latest['remotePid'] == 98280 and latest['managerPid'] == 81620
assert latest['currentUnexpectedDescendantObservationEvents'] == 0
assert latest['remoteSavedCredentialWireVerified'] is False
assert latest['remoteAnonymousInfoStatus'] == 401
backup = read('dependencies/fresh-offline-backup-public-derived-safe.json')
assert 'backupDirectory' not in backup
assert len(backup['roots']) == 6
assert read('scope-safe.json')['currentFinalRevalidationPassed'] is False
print(json.dumps({'status': 'offline-byte-integrity-and-independent-cross-references-verified', 'filesChecked': len(listed), 'historicalDesktopAndManaged60ActualNativeExitCodes': [0, 0], 'nativeExternalRevalidationExitCodes': exit_codes, 'frozenResourceTablesVerified': len(resource_tables), 'currentFinalRevalidationPassed': False, 'nativeProcessStateQueried': False}, indent=2))
