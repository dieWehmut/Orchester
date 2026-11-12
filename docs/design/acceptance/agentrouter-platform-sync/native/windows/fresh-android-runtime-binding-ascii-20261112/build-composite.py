"""Read-only verification and honest aggregation of previously executed native scopes.

No installer, ADB, emulator, product process, default installation, credential,
environment or raw execution log is read or modified. The only verification
subprocess is official apksigner on the public signed APK. Output files are
exclusive creations in this invocation's own directory.
"""
import hashlib
import json
import pathlib
import re
import subprocess
import zipfile
from datetime import datetime, timezone

ROOT = pathlib.Path(__file__).resolve().parent
REPO = pathlib.Path('/home/han/project/Orchester')
ARCHIVE = REPO / 'docs/design/acceptance/agentrouter-platform-sync'
NATIVE = pathlib.Path('/mnt/c/Users/han/AppData/Local/Orchester-build/android-studio-21-monitors-continuation-20261111')
RUN = pathlib.Path('/mnt/c/Users/han/AppData/Local/Orchester-build/monitor-final-20261110/monitors-final21-71def64-20261111')
REVISION = '71def643cb08b208cc79687f79255c504eb14dc6'
APK_SHA = 'f83fa2a75c712a3d2e1f3f203cf73ce89204fb4ca0bce4d8c2ef0c4fe3596d3c'
CERT = 'df6df435ccf10a685fb109668da8cdf7b022b481ba346f3143190aeda0c4468e'
EXPECTED_IDS = set('codex claude deepseek opencode gemini antigravity cursor workbuddy workbuddy_global qwen openclaw cline roo kilo hermes pi crush claude_app trae zcode craft'.split())
checks = []
read_files = {}

def require(condition, label):
    if not condition:
        raise ValueError(label)
    checks.append(label)

def sha(data):
    return hashlib.sha256(data).hexdigest()

def file_bytes(path):
    path = pathlib.Path(path)
    require(path.is_file() and not path.is_symlink(), 'regular listed file: ' + str(path))
    data = path.read_bytes()
    read_files[str(path)] = sha(data)
    return data

def read_json(path, expected=None):
    data = file_bytes(path)
    if expected:
        require(sha(data) == expected, 'immutable JSON pin: ' + str(path))
    return json.loads(data.decode('utf-8-sig'))

def win_path(path):
    value = str(path)
    require(value.startswith('/mnt/c/'), 'native Windows output path')
    return 'C:\\' + value[len('/mnt/c/'):].replace('/', '\\')

def native_path(value):
    require(value.startswith('C:\\'), 'listed source Windows path')
    return pathlib.Path('/mnt/c/' + value[3:].replace('\\', '/'))

def inventory(root):
    files = sorted(pathlib.Path(root).rglob('*'))
    require(not any(p.is_symlink() for p in files), 'public resource tree contains no redirected entries')
    return [{'path': str(p.relative_to(root)).replace('\\', '/'), 'sha256': sha(file_bytes(p))} for p in files if p.is_file()]

def exact_inventory(items):
    require(len(items) == len({v['path'] for v in items}), 'inventory contains unique paths')
    return sorted([{'path': v['path'], 'sha256': v['sha256']} for v in items], key=lambda v: v['path'])

def save(name, data):
    # ASCII JSON escapes preserve Unicode values and support the unchanged
    # Windows PowerShell5 guard that reads BOMless files using default ANSI.
    body = (json.dumps(data, ensure_ascii=True, indent=2) + '\n').encode('utf-8')
    with (ROOT / name).open('xb') as stream:
        stream.write(body)
    return sha(body)

pins = read_json(ROOT / 'input-pins.json')
components = []
documents = {}
for entry in pins['files']:
    path = pathlib.Path(entry['sourcePath'])
    value = read_json(path, entry['sha256'])
    require(path.stat().st_size == entry['bytes'], 'listed JSON bytes: ' + entry['key'])
    if entry.get('archivePath'):
        require(file_bytes(ARCHIVE / entry['archivePath']) == path.read_bytes(), 'source/archive byte equality: ' + entry['key'])
    documents[entry['key']] = value
    components.append(entry)

plan = read_json(RUN / 'plan.json', '8645dca6d20f2d5ff0d845d7c74522f52ca72c6aec49ec9529dcb86a997911cd')
build = read_json(RUN / 'final-result.json', '7ddcd67633e127a4b664dc69d91cf20bd8751a9ab6a009d8ca73d9b0951fe023')
prep = read_json(RUN / 'portable-package-preparation-safe.json', '208f79f9ec5a2a581fd23792c3b97444dee2ae9a347c2ba21ce0f6c8acc083b2')
require(plan['sourceRevision'] == REVISION, 'immutable native producer source71')
require(build['remoteExeSha256'] == prep['remoteExeSha256'], 'producer remote package binding')
require(prep['apkSha256'] == APK_SHA and prep['certificateSha256'] == CERT, 'prepared APK and signer binding')

source = documents['source_mirror']
provenance = documents['apk_provenance']
install = documents['install']
certificate = documents['certificate']
bridge = documents['bridge']
ui = documents['ui']
studio = documents['studio']
completed = documents['completion']
require(all(d['sourceRevision'] == REVISION for d in [source, provenance, install, bridge, ui, studio, completed]), 'all native completed scopes use same immutable source71')
require(source['allSourceGitExact'] is True and len(source['nativeSourceInputs']) == 41, '41 native source inputs historically Git exact')
for item in source['nativeSourceInputs']:
    require(sha(file_bytes(NATIVE / item['path'])) == item['sha256'], 'current native source mirror hash: ' + item['path'])
    blob = subprocess.run(['git', 'show', REVISION + ':' + item['path']], cwd=REPO, capture_output=True)
    require(blob.returncode == 0 and sha(blob.stdout) == item['sha256'], 'source71 native Git blob hash: ' + item['path'])

resources = [{'path': i['path'], 'sha256': i['sha256']} for i in provenance['publicResources']]
require(len(resources) == 17 and provenance['publicResourceCount'] == 17 and provenance['allPublicResourcesExact'] is True, 'original17 resources provenance')
require(exact_inventory(resources) == exact_inventory(prep['mobileDistInventory']), 'producer native inventory equals original APK resource inventory')
for directory in [native_path(plan['sourceRoot']) / 'apps/mobile/dist', REPO / 'apps/mobile/dist', NATIVE / 'apps/mobile/android/app/src/main/assets/public']:
    require(exact_inventory(inventory(directory)) == exact_inventory(resources), 'current exact17 resource inventory: ' + str(directory))

apk = native_path(prep['apkFile'])
require(sha(file_bytes(apk)) == APK_SHA and apk.stat().st_size == prep['apkBytes'] == 13960333, 'current signed APK exact bytes')
with zipfile.ZipFile(apk) as archive:
    names = sorted(n for n in archive.namelist() if n.startswith('assets/public/') and not n.endswith('/'))
    require(names == sorted('assets/public/' + r['path'] for r in resources), 'APK exact17 public resource names')
    for resource in resources:
        require(sha(archive.read('assets/public/' + resource['path'])) == resource['sha256'], 'APK embedded resource SHA: ' + resource['path'])
apksigner = pathlib.Path('/home/han/Android/Sdk/build-tools/36.0.0/apksigner')
apksigner_sha = sha(file_bytes(apksigner))
signed = subprocess.run([str(apksigner), 'verify', '--print-certs', str(apk)], capture_output=True, text=True)
require(signed.returncode == 0, 'official apksigner actual exit0')
certs = re.findall(r'^Signer #\d+ certificate SHA-256 digest: ([0-9a-f]{64})$', signed.stdout, re.MULTILINE)
require(certs == [CERT], 'current APK signer cryptographic validation')
require(certificate['originalWindowsInstalledCertificateSha256'] == certificate['currentFinal21CertificateSha256'] == CERT and certificate['commandActualExitCode'] == 0 and certificate['sameCertificateInstallRCompatible'] is True, 'correct Windows original installed certificate comparison')
require(install['appInstallActualExitCode'] == install['testInstallActualExitCode'] == 0 and install['installedApkSha256'] == APK_SHA and install['originalFiveAllEncryptedValuesRetained'] is True, 'correct Windows actual install-r and retention')
require(install['actualAvdHost'] == bridge['actualAvdHost'] == ui['actualAvdHost'] == 'Windows' and install['serial'] == bridge['serial'] == 'emulator-5572', 'correct same Windows native target')
require(provenance['nativeGradleActualExitCode'] == 0 and documents['gradle_result']['exitCode'] == 0 and documents['gradle_result']['ok'] is True, 'native qualified Gradle build actual exit0')
require(documents['gradle_started']['pid'] == documents['gradle_result']['pid'] == 27644 and documents['gradle_started']['argsExecutable'] == 'C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe', 'native Windows Gradle original process identity')
require(bridge['status'] == 'passed' and bridge['commandActualExitCode'] == 0 and bridge['tests'] == 4 and bridge['installedApkSha256'] == APK_SHA and bridge['testsReRunAfterCorrectDeviceProvenanceAndInstall'] is True, 'actual correct Windows four Bridge tests')
require(ui['status'] == 'passed' and ui['canonicalProviderCount'] == 21 and ui['canonicalThemeImageCount'] == 42 and ui['allCanonicalThemeImagesLoaded'] is True, 'native UI actual21 providers42loadedimages')
require(len(ui['canonicalProviderIds']) == 21 and set(ui['canonicalProviderIds']) == EXPECTED_IDS and ui['officialSourceMatchedMarkCount'] == 42 and ui['allCanonicalThemeMarksMatchOfficialSource'] is True, 'native UI exactcanonical and official identities')
require(studio['actualGradleResolveProjectSuccess'] is True and studio['actualIdeProjectDataImportFinished'] is True and studio['actualGradleSyncDurationMilliseconds'] == 17400, 'actual Android Studio JDK21 import and sync')

created, unicode, resize, close, remove, wire, channel = [documents[k] for k in ['terminal_created', 'unicode', 'resize', 'close', 'removal', 'wire', 'channel_cleanup']]
require(all(d['status'] == 'passed' for d in [created, unicode, resize, close, remove, wire, channel]), 'original completed native product subscopes')
require(created['beforeOwnedTerminalCount'] == 0 and created['afterOwnedTerminalCount'] == 1 and created['normalProductNewTerminalClicked'] is True, 'actual product create own terminal')
require(unicode['actualNativeAndroidEnterCommandExitCode'] == 0 and unicode['receivedOutputPacketCount'] == unicode['sentInputPacketCount'] == 3 and unicode['actualWindowsCmdCaretParsedUnicodeEchoReturned'] is True, 'actual Unicode and native Enter3input3output')
require(resize['productShowKeyboardTrustedTouch'] is True and resize['emulationMetricsInjected'] is False and resize['beforeViewportHeight'] > resize['afterViewportHeight'] and resize['beforeBackendRows'] == 28 and resize['afterBackendRows'] == 8 and resize['sameOwnTerminalStillRunning'] is True, 'actual trusted keyboard and same terminal resize')
require(close['normalProductCloseClicked'] is True and close['normalProductCloseConfirmed'] is True and close['afterBackendState'] == 'closed' and close['actualBackendExitCode'] == 1 and close['actualTerminalWebSocketExitCount'] == 1 and close['terminalOutputDrainedByWebSocketExit'] is True, 'actual normal product close and drain preserving terminal exit1')
require(close['actualWebSocketExitPacketCode'] is None and close['actualWebSocketExitPacketState'] is None, 'missing WebSocket payload fields remain unproved')
require(remove['beforeSavedDeviceCount'] == 6 and remove['afterSavedDeviceCount'] == 5 and remove['originalFiveEncryptedKeyValuePairsExactlyRetained'] is True and remove['ownBackendRevokeStatus'] == 204 and remove['ownTokenAfterRevokeInfoStatus'] == 401 and remove['serverRevocationSeparateNormalApiRequest'] is True, 'own product forget6to5 and separate revoke204401')
require(wire['nativeInstalledApkSha256'] == APK_SHA and wire['nativeInstalledApkBytes'] == 13960333 and wire['original7337GetStatusCodes'] == [200] and wire['originalSavedDeviceRecordsExactlyRetained'] is True and wire['decryptSucceededCount'] == 5 and wire['decryptFailedCount'] == 0, 'installedAPK/originalfiveexact AES5of5 original7337wire200')
require(completed['status'] == 'native-terminal-retention-revocation-and-owned-cleanup-completed' and completed['ownTaskAbsenceIndependentlyObserved'] is True and completed['ownScheduledTaskDeleted'] is True, 'own task actual absence')
require(completed['taskCleanupInvokerActualExitCode'] == 1 and completed['preDeleteTaskResultReceiptDirectlyRecorded'] is False and completed['preDeleteTaskResultZeroBoundByGuardedExecutedSourceFlow'] is True, 'task writer exit1 and weak predelete result explicitly preserved')
require(completed['ownBackendActualNormalQuitAllOriginalHandlesEnded'] is True and completed['ownBackendActualLauncherExitCode'] == 0 and completed['ownBackendPrivateTransportRemaining'] == 0 and completed['ownTemporary7339PortReleased'] is True, 'original own backend normal quit and transports cleanup')
require(channel['sameOriginalBirthImageAndPrivatePortsVerified'] is True and channel['allFourPrivatePortsReleased'] is True and channel['queryHandlesClaimedAsCreatorHandles'] is False, 'own native channel verified identity and ports cleanup')
for field in ['normalOwnEmulatorConsoleKillExitCode', 'actualQemuQueryHandleExitCode', 'actualHeadQueryHandleExitCode', 'normalOwnAdb5039KillServerCommandExitCode', 'actualServerQueryHandleExitCode', 'originalManagerCreatorEmulatorHandleActualExitCode', 'originalManagerCreatorServerHandleActualExitCode', 'actualManagerQueryHandleExitCode']:
    require(channel[field] == 0, 'native cleanup actual exit0: ' + field)
require(completed['nativeNormalHomeCommandExitCode'] == 0 and completed['appProcessDestroyedClaimed'] is False and completed['processSignals'] == completed['forcedTerminations'] == completed['modelCalls'] == 0, 'normal Home boundary and no force/model signals')

# The native Windows PowerShell inventory uses culture-aware ordering; preserve
# its exact order only after proving complete path/hash set equivalence above.
# The frozen guard deliberately compares serialized order and remains unchanged.
resources = prep['mobileDistInventory']

for path, expected in read_files.items():
    require(sha(pathlib.Path(path).read_bytes()) == expected, 'all listed inputs stable after validation: ' + path)
original = {
    'format': 'orchester-derived-composite-native-android-acceptance-v1', 'status': 'passed',
    'receiptKind': 'derived-composite-of-hash-verified-original-native-receipts',
    'singleNativeExecutionClaimed': False, 'nativeCommandsReexecutedByCompositeVerifier': False,
    'acceptanceMeaning': 'Every required source/build/install/Bridge/UI/terminal/owned-cleanup scope is supported by its listed historical native receipt; current public source/APK/resource/certificate bytes were independently reread. Passed does not convert nonzero product or observer exits to zero.',
    'sourceRevision': REVISION, 'apkSha256': APK_SHA, 'certificateSha256': CERT,
    'resourcesDistMirrorAndApkByteExact': True, 'resourceInventory': resources,
    'build': {'nativeWindows': True, 'exitCode': 0, 'receiptKey': 'gradle_result', 'historicalOriginalPid': 27644, 'firstBuildFailurePreserved': True},
    'install': {'appExitCode': 0, 'testExitCode': 0, 'receiptKey': 'install', 'targetHost': 'Windows', 'serial': 'emulator-5572'},
    'nativeBridge': {'status': 'passed', 'commandActualExitCode': 0, 'tests': 4, 'receiptKey': 'bridge'},
    'ui': dict(ui, receiptKey='ui'),
    'terminal': {'status': 'passed', 'receiptKeys': ['terminal_created','unicode','resize','close'], 'actualClosedCmdTerminalExitCode': 1, 'webSocketExitPacketState': None, 'webSocketExitPacketCode': None, 'normalProductCloseAndDrain': True},
    'cleanup': {'status': 'passed', 'receiptKeys': ['removal','wire','channel_cleanup','completion'], 'ownTaskAbsent': True, 'taskCleanupInvokerActualExitCode': 1, 'directPreDeleteTaskResultZeroRecorded': False, 'preDeleteTaskZeroSourceFlowOnly': True, 'ownBackendOriginalHandlesNormalExitVerified': True, 'normalHomeDoesNotProveAppProcessDestroyed': True, 'serverRevokeWasSeparateNormalApiRequest': True},
    'androidStudio': {'actualResolveAndImport': True, 'syncMilliseconds': 17400, 'receiptKey': 'studio', 'ideRunInstrumentationClaimed': False},
    'originalNativeReceipts': components, 'currentVerificationAssertions': len(checks),
    'verifierSourceSha256': sha(pathlib.Path(__file__).read_bytes()),
    'apksigner': {'actualExitCode': signed.returncode, 'toolSha256': apksigner_sha, 'certificateSha256': CERT},
    'formalWindowsPortable14OrDesktop18Accepted': False, 'defaultWindowsInstallationChanged': False,
    'privateCredentialsEnvironmentRawLogsOrScreenshotsIncluded': False,
}
original_sha = save('original-composite-acceptance-safe.json', original)
normalized = {
    'format': 'orchester-fresh-native-android-acceptance-v1', 'status': 'passed',
    'receiptKind': 'normalized-derived-composite-binding-not-single-native-execution',
    'sourceRevision': REVISION, 'apkSha256': APK_SHA, 'certificateSha256': CERT,
    'sourceAndDistVerified': True, 'originalInstalledCertificateMatched': True,
    'nativeBridgeAccepted': True, 'nativeUiAccepted': True, 'nativeTerminalAccepted': True, 'cleanupAccepted': True,
    'nativeGradleExitCode': 0, 'nativeInstallExitCode': 0,
    'originalReceiptFile': win_path(ROOT / 'original-composite-acceptance-safe.json'),
    'originalReceiptSha256': original_sha, 'mobileDistInventory': resources,
    'limitations': original['acceptanceMeaning'],
    'taskCleanupInvokerActualExitCode': 1, 'actualClosedCmdTerminalExitCode': 1,
    'formalWindows14Or18Accepted': False, 'defaultInstallationChanged': False,
}
normalized_sha = save('normalized-acceptance-safe.json', normalized)
save('composite-verification-safe.json', {
    'status': 'all-listed-original-scopes-and-current-public-bytes-validated',
    'observedAtUtc': datetime.now(timezone.utc).isoformat(), 'sourceRevision': REVISION,
    'assertionsPassed': len(checks), 'originalNativeReceiptComponentCount': len(components),
    'originalReceiptSha256': original_sha, 'normalizedReceiptSha256': normalized_sha,
    'nativeSourceGitBlobChecks': 41, 'resourceFilesComparedPerTree': 17, 'resourceTreesCompared': 3,
    'apkEmbeddedPublicResourceFilesCompared': 17, 'actualApksignerExitCode': signed.returncode,
    'verifierSourceSha256': sha(pathlib.Path(__file__).read_bytes()),
    'modelCalls': 0, 'processOrInputSignals': 0, 'runtime14Executed': False, 'runtime18Executed': False,
    'defaultInstallationChanged': False, 'historicalNativeCommandsRerun': False,
    'privateContentsReadOrIncluded': False,
})
print(json.dumps({'normalizedReceiptFile': win_path(ROOT / 'normalized-acceptance-safe.json'), 'normalizedReceiptSha256': normalized_sha, 'originalReceiptSha256': original_sha, 'assertionsPassed': len(checks)}))
