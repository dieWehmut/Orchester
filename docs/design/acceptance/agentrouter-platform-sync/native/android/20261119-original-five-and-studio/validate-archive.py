#!/usr/bin/env python3
"""Read-only archive validation. Never execute archived native sources."""
import sys
sys.dont_write_bytecode = True
import ast
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path, PurePosixPath
import re


class ArchiveError(ValueError):
    pass


def require(condition, label):
    if not condition:
        raise ArchiveError(label)


def read_json(path):
    def pairs(items):
        result = {}
        for key, value in items:
            require(key not in result, 'duplicate JSON field')
            result[key] = value
        return result
    try:
        return json.loads(path.read_text(encoding='utf-8-sig'), object_pairs_hook=pairs)
    except (OSError, UnicodeError, json.JSONDecodeError):
        raise ArchiveError('JSON read or parse failed') from None


def walk(value, key=''):
    if isinstance(value, dict):
        for name, child in value.items():
            yield from walk(child, name)
    elif isinstance(value, list):
        for child in value:
            yield from walk(child, key)
    else:
        yield key, value


def field(data, dotted):
    value = data
    for key in dotted.split('.'):
        require(isinstance(value, dict) and key in value, 'required field missing: ' + dotted)
        value = value[key]
    return value


def expect(data, dotted, expected):
    actual = field(data, dotted)
    require(type(actual) is type(expected) and actual == expected, 'assertion failed: ' + dotted)


def utc_day(data, day='2026-11-19'):
    value = field(data, 'observedAtUtc')
    require(isinstance(value, str), 'UTC timestamp is not a string')
    # Python 3.10 accepts microseconds but rejects Windows' seven fractional
    # digits. Normalize a temporary calendar-comparison copy only. The original
    # receipt string and exact birth/FileTime identity fields stay untouched.
    comparison_value = re.sub(
        r'\.(\d+)(?=Z$|[+-]\d{2}:\d{2}$)',
        lambda match: '.' + (match.group(1) + '000000')[:6],
        value,
    )
    if comparison_value.endswith('Z'):
        comparison_value = comparison_value[:-1] + '+00:00'
    try:
        parsed = datetime.fromisoformat(comparison_value)
    except ValueError:
        raise ArchiveError('UTC timestamp format invalid') from None
    require(parsed.tzinfo is not None and parsed.utcoffset() == timezone.utc.utcoffset(parsed), 'timestamp is not UTC')
    require(parsed.date().isoformat() == day, 'actual UTC date changed')


def digest(value):
    return isinstance(value, str) and re.fullmatch(r'[a-f0-9]{64}', value) is not None


# Exact names avoid rejecting safe counters such as nativeCredentialGetCount.
BLOCKED_KEYS = {
    'token', 'tokens', 'accessToken', 'refreshToken', 'access_token', 'refresh_token',
    'authorization', 'Authorization', 'hmac', 'HMAC', 'ciphertext', 'rawXml',
    'encryptedXml', 'rawCredentials', 'localStorage', 'localStorageValues',
    'deviceId', 'deviceIds', 'deviceName', 'deviceNames', 'deviceAddress',
    'deviceAddresses', 'address', 'addresses', 'name', 'names', 'devices',
    'capKey', 'capKeys', 'capabilityKey', 'capabilityKeys', 'privateCapKey',
    'privateCapKeys', 'privateCapabilityKey', 'privateCapabilityKeys',
}
PRIVATE_PATH = re.compile(r'(?:[A-Za-z]:[\\/]+Users[\\/]|/home/[A-Za-z0-9_.-]+/|/mnt/[a-z]/Users/)', re.I)
RAW_SID = re.compile(r'\bS-1-5-21-(?:\d+-){2,}\d+\b')
JWT = re.compile(r'\beyJ[A-Za-z0-9_-]{12,}\.[A-Za-z0-9_-]{12,}\.[A-Za-z0-9_-]{12,}\b')
SAVED_TOKEN_KEY = re.compile(r'orchester\.token:(?:https?://|[A-Za-z0-9][A-Za-z0-9_.-]+:\d)')


def scan_json(data):
    if isinstance(data, dict):
        for key, value in data.items():
            require(key not in BLOCKED_KEYS, 'sensitive exact JSON field present')
            if key.lower().endswith('sha256'):
                require(digest(value), 'SHA-256 field format invalid')
            scan_json(value)
    elif isinstance(data, list):
        for value in data:
            scan_json(value)
    elif isinstance(data, str):
        require(PRIVATE_PATH.search(data) is None, 'private absolute path present')
        require(RAW_SID.search(data) is None, 'raw user SID present')
        require(JWT.search(data) is None, 'JWT-shaped private value present')
        require(SAVED_TOKEN_KEY.search(data) is None, 'saved credential origin key present')


def validate(root):
    manifest = read_json(root / 'manifest.json')
    scan_json(manifest)
    entries = field(manifest, 'files')
    require(isinstance(entries, list) and entries, 'manifest files must be a nonempty list')
    listed = {}
    for entry in entries:
        require(isinstance(entry, dict), 'manifest entry must be an object')
        name = field(entry, 'path')
        require(isinstance(name, str) and '\\' not in name, 'manifest path must be POSIX relative')
        path = PurePosixPath(name)
        require(not path.is_absolute() and name == str(path) and not any(p in ('', '.', '..') for p in path.parts), 'manifest path is not canonical')
        require(name != 'manifest.json' and name not in listed, 'manifest self-reference or duplicate path')
        require(digest(field(entry, 'sha256')), 'manifest SHA-256 invalid')
        size = field(entry, 'bytes')
        require(type(size) is int and size >= 0, 'manifest byte size invalid')
        target = root / name
        require(target.is_file() and not target.is_symlink(), 'manifest file missing or symbolic link')
        raw = target.read_bytes()
        require(len(raw) == size and hashlib.sha256(raw).hexdigest() == entry['sha256'], 'public file bytes or hash differ')
        listed[name] = entry
    actual = set()
    for path in root.rglob('*'):
        require(not path.is_symlink(), 'archive symbolic link present')
        if path.is_file() and path.relative_to(root).as_posix() != 'manifest.json':
            actual.add(path.relative_to(root).as_posix())
    require(actual == set(listed), 'manifest does not cover complete archive inventory')
    expect(manifest, 'fileCountExcludingManifest', len(listed))
    expect(manifest, 'fileBytesExcludingManifest', sum(e['bytes'] for e in entries))
    require({'README.md', 'validate-archive.py'} <= actual, 'README or validator not inventoried')
    require(not any('__pycache__' in n or n.endswith(('.pyc', '.png', '.jpg', '.jpeg', '.webp', '.apk')) for n in actual), 'unreviewed cache, screenshot or native binary present')
    dex_files = {n for n in actual if n.endswith('.dex')}
    require(dex_files == {'sources/native/alias-guard.dex'}, 'diagnostic Dex inventory differs')
    require(listed['sources/native/alias-guard.dex']['sha256'] == 'df15578ba781311a7a00377af05c387731c663906ec4174d7a8f46ee9a5ebd31', 'diagnostic Dex differs from accepted static helper')
    for name in sorted(actual):
        path = root / name
        if path.suffix == '.json':
            scan_json(read_json(path))
        elif path.suffix in ('.py', '.js', '.java', '.ps1', '.md', '.txt'):
            text = path.read_text(encoding='utf-8-sig')
            require(PRIVATE_PATH.search(text) is None, 'private path literal present in public source')
            require(RAW_SID.search(text) is None and JWT.search(text) is None, 'private identity or authentication value in public source')
            require(SAVED_TOKEN_KEY.search(text) is None, 'saved credential origin literal in public source')
            if path.suffix == '.py':
                try:
                    ast.parse(text, filename=name)
                except SyntaxError:
                    raise ArchiveError('public Python source or template syntax invalid') from None
            if '.template.' in name:
                guard = ('if __name__ == "__main__":\n    raise RuntimeError("public-review-template-native-execution-disabled")'
                         if path.suffix == '.py' else "throw 'public-review-template-native-execution-disabled'")
                require(guard in '\n'.join(text.splitlines()[:3]), 'public review template execution guard missing')
    def load(name):
        require(name in listed, 'required evidence file missing: ' + name)
        return read_json(root / name)
    summary = load('summary-safe.json')
    wire = load('wire/native-five-wire-safe.json')
    storage = load('storage/full-product-storage-tail-safe.json')
    xml = load('storage/historical-xml-digest-safe.json')
    saved = load('storage/historical-saved-fields-safe.json')
    quit_proof = load('cleanup/normal-manager-quit-safe.json')
    terminal = load('cleanup/original-manager-terminal-safe.json')
    release = load('cleanup/post-release-current-safe.json')
    studio = load('studio/actual-credentials-analysis-safe.json')
    studio_identity = load('studio/current-studio-identity-safe.json')
    identity = load('ownership/identity-safe.json')
    linux = load('ownership/linux-owner-safe.json')
    failure = load('ownership/manager-failure-safe.json')
    app = load('wire/installed-app-read-safe.json')
    boot = load('wire/boot-readiness-safe.json')
    binding = load('wire/binding-safe.json')
    app_binding = load('wire/app-read-binding-safe.json')
    self_binding = load('wire/self-binding-safe.json')
    manager = load('ownership/manager-bindings-safe.json')
    provenance = load('provenance-safe.json')
    source_mapping = load('source-mappings-original-derived-safe.json')
    studio_mapping = load('studio/source-receipt-mapping-derived-safe.json')
    for data in (summary, wire, storage, quit_proof, terminal, release, studio, identity, linux):
        utc_day(data)
    expect(failure, 'errorType', 'MemoryError')
    expect(failure, 'observationStep', 'qemu-identity')
    expect(failure, 'diagnostic.nativeOperation', 'image-sha-read')
    utc_day(failure, '2026-11-17')
    revision = '71def643cb08b208cc79687f79255c504eb14dc6'
    expect(manifest, 'sourceRevision', revision)
    expect(provenance, 'archiveSourceRevision', revision)
    for data in (summary, wire, app, boot, binding, app_binding, identity, linux, source_mapping):
        expect(data, 'sourceRevision', revision)
    for data in (summary, wire, app, boot, identity):
        expect(data, 'actualAvdHost', 'Windows')
        expect(data, 'actualAvdName', 'Medium_Phone_API_37.0')
    for data in (wire, app, boot, identity):
        expect(data, 'serial', 'emulator-5572')
    for data in (wire, app, boot):
        expect(data, 'nativeAdbServerPort', 5041)
    expect(identity, 'serverPort', 5041)
    for data in (wire, app, boot, binding, app_binding, self_binding):
        expect(data, 'namespaceId', identity['namespaceId'])
    for key in ('originalCreatorHandlesHeld', 'qemuQueryAndSynchronizeHandleHeld', 'transportIdentityCommandsCompleted',
                'independentlyVerifiedCurrentNativeProcesses', 'independentObserverOwnQueryHandlesClosed'):
        expect(identity, key, True)
    for key in ('qemuParentMatches', 'headParentMatches', 'headUsesOriginalAvd', 'workerOriginalPathMatches'):
        expect(identity, 'independentParentAndCommandComparisons.' + key, True)
    for key in ('pid', 'createdExact', 'sessionId'):
        expect(identity, 'worker.' + key, field(manager, 'nativeWorker.' + key))
    expect(identity, 'worker.sha256', field(manager, 'nativeWorker.imageSha256'))
    for key in ('serverPort', 'consolePort', 'guestAdbPort', 'cdpPort'):
        expect(identity, key, field(manager, 'channel.' + key))
    expect(linux, 'targetHost', 'Linux')
    expect(linux, 'port', 7337)
    for key in ('currentListeningSocketOwned', 'managedUnitActive', 'pidBirthCgroupAndBootStable'):
        expect(linux, key, True)
    for key in ('serviceActions', 'processSignals'):
        expect(linux, key, 0)
    expect(wire, 'rootLinux7337ListenerPid', linux['listenerPid'])
    expect(wire, 'rootLinux7337ListenerStartTicks', linux['listenerStartTicks'])
    expect(wire, 'rootLinuxOwnerProofIndependentlyReverifiedByWindows', False)
    expect(self_binding, 'linuxOwnerProofIndependentlyReverifiedByWindows', False)
    apk = 'f83fa2a75c712a3d2e1f3f203cf73ce89204fb4ca0bce4d8c2ef0c4fe3596d3c'
    for data in (wire, app):
        expect(data, 'nativeInstalledApkSha256', apk)
        expect(data, 'nativeInstalledApkBytes', 13960333)
        expect(data, 'actualRunAsAppUid', 10235)
        expect(data, 'certificateIndependentlyReparsedThisJob', False)
        for key in ('nativePmPathExitCode', 'nativeInstalledApkReadExitCode', 'nativeUidReadExitCode', 'nativeEncryptedXmlReadExitCode'):
            expect(data, key, 0)
    expect(boot, 'sysBootCompletedOneObserved', True)
    expect(boot, 'allNativeAdbCreatorHandlesActualTerminal', True)
    expect(boot, 'nativeAdbCreatorActualExitCodes', [0])
    expect(self_binding, 'sourceInputsAndRuntimeProofsReadLocked', True)
    expect(self_binding, 'actualNativeBootExitCode', 0)
    expect(self_binding, 'actualNativeAppBindingExitCode', 0)
    expect(wire, 'currentWindowsFiveWireAccepted', True)
    for key in ('appInstallOrResetOrWipe', 'newPairingUsed', 'credentialSetOrRemoveInvoked', 'rawXmlKeysValuesAddressesNamesOrTokensIncluded'):
        expect(wire, key, False)
    for key in ('keyGenerationCalls', 'forcedTerminations'):
        expect(wire, key, 0)
    expect(wire, 'existingNativeKeystoreAliasPresentBeforeCredentialGet', True)
    observation = field(wire, 'nativeObservation')
    for key, value in {'nativePlatform': True, 'platformAndroid': True, 'nativeCredentialGetCount': 5, 'decryptSucceededCount': 5, 'decryptFailedCount': 0, 'nativeHttpGetAttemptCount': 1, 'original7337GetStatusCodes': [200], 'original7337InfoNameMatched': True, 'originalSavedOriginUsedWithoutRewrite': True, 'currentSavedDeviceStorageRawUnchanged': True}.items():
        expect(observation, key, value)
    original = 'f708ff5441e06dbe1c542d778ba70a270409bdef14a1fec508d88eeeb63abdbd'
    for key in ('beforeEncryptedXmlSha256', 'afterEncryptedXmlSha256'):
        expect(wire, key, original)
    expect(xml, 'derivedOriginalSha256', original)
    expect(xml, 'derivedOriginalRecordCount', 5)
    expect(xml, 'rawKeysCiphertextOrDerivedXmlArchived', False)
    monitor = field(wire, 'actualNativeProductUi.actualProductMonitor')
    for key, value in {'passed': True, 'connected': True, 'providerRowCount': 21, 'uniqueCanonicalProviderCount': 21, 'themeImageCount': 42, 'darkImageCount': 21, 'lightImageCount': 21, 'allThemeImagesLoaded': True, 'snapshotNoticeCount': 0}.items():
        expect(monitor, key, value)
    own = field(wire, 'actualNativeProductUi')
    expect(own, 'terminalCreatedByThisTask', True)
    expect(own, 'existingSessionInputCommandsSent', 0)
    expect(own, 'actualOwnTerminalOutput.staticAcceptanceMarkerOccurrences', 2)
    expect(own, 'actualOwnTerminalNormalExit.passed', True)
    expect(own, 'actualOwnTerminalNormalExit.forcedCloseInvoked', False)
    for key in ('ownForwardActuallyRemoved', 'ownDiagnosticDexActuallyRemoved', 'cleanupComplete', 'allNativeAdbCreatorHandlesActualTerminal', 'nativeHostQueryHandlesClosed', 'normalProductBackExit.productNoLongerResumed'):
        expect(wire, key, True)
    codes = field(wire, 'nativeAdbCreatorActualExitCodes')
    require(isinstance(codes, list) and len(codes) == 48 and all(type(c) is int for c in codes) and codes.count(1) == 1 and codes.count(0) == 47, 'wire native creator outcomes differ')
    expect(wire, 'diagnosticDexWritableProbeActualExitCode', 1)
    expect(storage, 'status', 'current-full-product-storage-and-historical-five-records-tail-passed')
    for key in ('credentialGets', 'remoteHttpCalls', 'storageWrites'):
        expect(storage, key, 0)
    for key in ('after.fullLocalStorageExactlyUnchanged', 'after.savedDeviceStorageRawExactlyUnchanged', 'after.themeValueExactlyUnchanged', 'normalBackExitObserved', 'ownForwardActuallyRemoved', 'allNativeAdbCreatorHandlesTerminal', 'ownNativeQueryHandlesClosed'):
        expect(storage, key, True)
    expect(storage, 'before.localStorageEntryCount', 4)
    expect(storage, 'after.localStorageEntryCount', 4)
    expect(storage, 'nativeAdbCreatorActualExitCodes', [0] * 18)
    for key in ('fullLocalStorageSortedPairsSha256', 'savedDevicesRawUtf8Sha256', 'themeValueSha256'):
        require(field(storage, 'before.' + key) == field(storage, 'after.' + key), 'storage before/after digest differs')
    require(field(storage, 'before.fullLocalStorageSortedPairsSha256') == field(saved, 'historicalQaFullSortedStorageWithOriginalFiveSha256'), 'complete persisted fields historical digest differs')
    require(field(storage, 'before.savedDevicesCompactUtf8Sha256') == field(saved, 'derivedOriginalRecordsCompactUtf8Sha256'), 'saved five historical digest differs')
    require(field(storage, 'before.themeValueSha256') == field(saved, 'historicalQaThemeJsonValueSha256'), 'theme historical digest differs')
    expect(saved, 'derivedOriginalRecordCount', 5)
    expect(saved, 'historicalQaFullStorageDerivedOriginalFiveEntryCount', 4)
    expect(saved, 'originalEarliestWholeMetadataSha256Recovered', False)
    expect(summary, 'sourceLimits.earliestWholeMetadataPrivateFileSha256Reverified', False)
    expect(summary, 'sourceLimits.automaticStartupReconnectRequestsMeasured', False)
    expect(summary, 'sourceLimits.certificateIndependentlyReparsedThisJob', False)
    expect(quit_proof, 'nativeExitCodes', {'manager': 1, 'server': 0, 'head': 0, 'qemu': 0})
    expect(quit_proof, 'originalManagerFailurePreserved', 'native-observation-or-control-failed')
    for key in ('normalControlSubmitted', 'allOriginalManagerCreatorHandlesActuallyClosed', 'allFourPrivatePortsReleased', 'ownNativeQueryHandlesClosed'):
        expect(quit_proof, key, True)
    expect(quit_proof, 'forcedTerminations', 0)
    expect(terminal, 'status', 'owned-channel-terminal-with-preserved-failure')
    expect(terminal, 'qemuQueryHandleExitCode', 0)
    expect(terminal, 'failureCode', 'native-observation-or-control-failed')
    for key in ('allCreatorHandlesClosedAfterActualTerminal', 'allCreatedNativeHandlesActuallyTerminal', 'allFourPrivatePortsReleased', 'normalQuitRequested'):
        expect(terminal, key, True)
    require(all(type(c) is int and c == 0 for c in field(terminal, 'originalCreatorExitCodes').values()), 'original manager creator child nonzero')
    expect(release, 'allFiveOriginalNativeProcessesAbsent', True)
    expect(release, 'privateTcpRowCount', 0)
    expect(release, 'queryHandlesClosed', True)
    expect(release, 'privatePorts', [5041, 5572, 5573, 19330])
    rows = field(release, 'processes')
    require(isinstance(rows, list) and len(rows) == 5 and {r['role'] for r in rows} == {'manager', 'worker', 'server', 'head', 'qemu'}, 'post-release original process roles differ')
    for row in rows:
        role = row['role']
        expect(row['observation'], 'pid', identity[role]['pid'])
        expect(row['observation'], 'expectedCreatedExact', identity[role]['createdExact'])
        expect(row['observation'], 'originalProcessAbsent', True)
        expect(row['observation'], 'birthMatched', True)
        expect(row['observation'], 'currentNativeHandleTerminal', True)
        expect(row['observation'], 'queryHandleClosed', True)
    for key in ('processSignals', 'adbOperations', 'avdOperations', 'newJobsSubmitted'):
        expect(release, key, 0)
    expect(studio, 'actualNativeExitCode', 0)
    expect(studio, 'helpNativeExitCode', 0)
    expect(studio, 'analysis.errors', 0)
    expect(studio, 'analysis.warnings', 0)
    expect(studio, 'analysis.infos', 3)
    expect(studio, 'analysis.stdoutReportsStudioPid', False)
    expect(studio, 'nativeCreatorExitCodes', [0, 0])
    for key in ('originalStudioHandleHeldThroughAnalysis', 'studioIdentityVerifiedAfterAnalysis', 'allNativeChildrenOriginalHandlesActuallyTerminal', 'studioQueryHandleClosed'):
        expect(studio, key, True)
    for key in ('rawStdoutArchived', 'rawStderrArchived', 'privateValuesIncluded'):
        expect(studio, key, False)
    for key in ('studioPid', 'studioCreatedExact', 'studioImageSha256'):
        expect(studio, key, studio_identity[key])
    expect(studio, 'sourceReadLocksHeldThroughAnalysis', True)
    expect(studio, 'analysis.fixedStudioPidExplicitArgumentBound', True)
    expect(studio, 'sourceSha256', listed['sources/studio/CredentialsPlugin.java']['sha256'])
    # Each reference binds the original bytes of its required role. Public
    # redaction hashes are separate; another known digest is not a substitute.
    proofs = {}
    for entry in field(provenance, 'provenance'):
        name = field(entry, 'publicPath')
        require(name in listed and name not in proofs, 'provenance target missing or duplicate')
        expect(entry, 'publicSha256', listed[name]['sha256'])
        expect(entry, 'publicBytes', listed[name]['bytes'])
        if 'sourceSha256' in entry:
            require(digest(entry['sourceSha256']), 'provenance original SHA-256 invalid')
            require(type(entry.get('sourceBytes')) is int and entry['sourceBytes'] >= 0, 'provenance original bytes invalid')
        if entry.get('representation') == 'original-bytes':
            expect(entry, 'sourceSha256', entry['publicSha256'])
            expect(entry, 'sourceBytes', entry['publicBytes'])
        proofs[name] = entry
    require(set(proofs) == set(listed) - {'provenance-safe.json'}, 'provenance inventory incomplete')
    def bound(data, key, target):
        require(target in proofs and 'sourceSha256' in proofs[target], 'original reference target missing')
        expect(data, key, proofs[target]['sourceSha256'])
    for data, key, target in (
        (wire, 'identityReceiptSha256', 'ownership/identity-safe.json'),
        (wire, 'installedAppBindingSha256', 'wire/installed-app-read-safe.json'),
        (wire, 'rootCurrentLinux7337OwnerReceiptSha256', 'ownership/linux-owner-safe.json'),
        (wire, 'bindingSha256', 'wire/binding-safe.json'),
        (wire, 'managerBindingSha256', 'ownership/manager-bindings-safe.json'),
        (wire, 'sourceSha256', 'sources/native/native_five_current_wire.template.py'),
        (app, 'sourceSha256', 'sources/native/read_installed_app_binding.template.py'),
        (app, 'identityReceiptSha256', 'ownership/identity-safe.json'),
        (identity, 'bindingSha256', 'ownership/manager-bindings-safe.json'),
        (boot, 'sourceSha256', 'sources/native/wait_owned_android_boot.template.py'),
        (boot, 'bindingSha256', 'wire/app-read-binding-safe.json'),
        (binding, 'identitySha256', 'ownership/identity-safe.json'),
        (binding, 'managerBindingSha256', 'ownership/manager-bindings-safe.json'),
        (binding, 'installedAppBindingSha256', 'wire/installed-app-read-safe.json'),
        (binding, 'currentManagedLinux7337Sha256', 'ownership/linux-owner-safe.json'),
        (binding, 'aliasDexSha256', 'sources/native/alias-guard.dex'),
        (binding, 'expressionSha256', 'sources/native/native_five_wire_expression.js'),
        (app_binding, 'identitySha256', 'ownership/identity-safe.json'),
        (app_binding, 'managerBindingSha256', 'ownership/manager-bindings-safe.json'),
        (self_binding, 'identitySha256', 'ownership/identity-safe.json'),
        (self_binding, 'managerBindingSha256', 'ownership/manager-bindings-safe.json'),
        (self_binding, 'bootProofSha256', 'wire/boot-readiness-safe.json'),
        (self_binding, 'appProofSha256', 'wire/installed-app-read-safe.json'),
        (self_binding, 'readBindingSha256', 'wire/app-read-binding-safe.json'),
        (self_binding, 'wireBindingSha256', 'wire/binding-safe.json'),
        (quit_proof, 'originalManagerTerminalReceiptSha256', 'cleanup/original-manager-terminal-safe.json'),
    ):
        bound(data, key, target)
    for group in ('sourceMappings', 'receiptMappings'):
        mapped = field(source_mapping, group)
        names = [item['path'] for item in mapped]
        require(len(names) == len(set(names)), 'original source mapping duplicate path')
        for item in mapped:
            matching = [p for p in proofs.values() if p.get('sourceFilename') == PurePosixPath(item['path']).name
                        and p.get('sourceSha256') == item['sha256'] and p.get('sourceBytes') == item['bytes']]
            require(bool(matching), 'original source mapping not bound to matching provenance role')
    for item in field(studio_mapping, 'sourcesAndReceipts'):
        name = PurePosixPath(item['linuxFile']).name
        matching = [p for p in proofs.values() if p.get('sourceFilename') == name
                    and p.get('sourceSha256') == item['sha256'] and p.get('sourceBytes') == item['bytes']]
        require(bool(matching), 'Studio original source mapping differs')
        expect(item, 'linuxAndWindowsBytesEqual', True)
    # These are dated historical checks, distinct from the 2026-11-19 wire run.
    sync = load('studio/project-sync-20261116-safe.json')
    project = load('studio/project-validation-20261116-safe.json')
    bridge = load('history/native-bridge-tests-20261111-safe.json')
    resize = load('history/keyboard-resize-20261112-safe.json')
    completion = load('history/product-completion-20261112-safe.json')
    reboot = load('history/linux-reboot-lifecycle-20261116-safe.json')
    coverage = load('requirements-coverage-safe.json')
    utc_day(project, '2026-11-16')
    utc_day(reboot, '2026-11-16')
    for data in (sync, project, bridge, completion, coverage):
        expect(data, 'sourceRevision', revision)
    expect(sync, 'studioPid', studio['studioPid'])
    expect(project, 'studioPid', studio['studioPid'])
    expect(project, 'studioImageSha256', studio['studioImageSha256'])
    expect(project, 'studioExactBirthUtc', sync['studioExactBirthUtc'])
    # FileTime is the immutable process birth; convert via integer arithmetic.
    birth = datetime.fromisoformat(sync['studioExactBirthUtc'][:26] + '+00:00')
    epoch = datetime(1601, 1, 1, tzinfo=timezone.utc)
    delta = birth - epoch
    birth_filetime = (delta.days * 86400 + delta.seconds) * 10000000 + delta.microseconds * 10
    fraction = re.search(r'\.(\d+)Z$', sync['studioExactBirthUtc']).group(1)
    birth_filetime += int((fraction + '0000000')[6])
    expect(studio, 'studioCreatedExact', str(birth_filetime))
    for key in ('actualIdeProjectSyncSuccess', 'actualIdeProjectImportFinished'):
        expect(sync, key, True)
    expect(sync, 'actualIdeTestRunInvoked', False)
    expect(project, 'actualIdeSyncDurationMilliseconds', 44193)
    expect(project, 'actualIdeInstrumentationRunInvoked', False)
    expect(project, 'separateGradleUnitTask.actualNativeExitCode', 0)
    expect(project, 'separateGradleUnitTask.status', 'NO-SOURCE')
    expect(project, 'separateGradleUnitTask.testReportXmlCount', 0)
    expect(project, 'separateGradleUnitTask.unitTestsPassedClaim', False)
    expect(project, 'sourceAndPackagedWebInputsAfterChecks.count', 58)
    expect(project, 'sourceAndPackagedWebInputsAfterChecks.allHashesEqual', True)
    expect(bridge, 'commandActualExitCode', 0)
    expect(bridge, 'tests', 4)
    expect(bridge, 'instrumentationCode', -1)
    expect(bridge, 'actualAvdHost', 'Windows')
    expect(bridge, 'actualAvdName', 'Medium_Phone_API_37.0')
    expect(bridge, 'serial', 'emulator-5572')
    expect(bridge, 'installedApkSha256', apk)
    expect(bridge, 'originalCertificateSha256', wire['certificateSha256BoundByByteExactExpectedApk'])
    for key in ('productShowKeyboardTrustedTouch', 'nativeViewportActuallyShrunk', 'backendDimensionsActuallyChanged', 'sameOwnTerminalStillRunning'):
        expect(resize, key, True)
    expect(resize, 'emulationMetricsInjected', False)
    expect(resize, 'beforeBackendRows', 28)
    expect(resize, 'afterBackendRows', 8)
    expect(completion, 'nativeInstalledFinal21ApkSha256', apk)
    expect(completion, 'actualClosedCmdTerminalExitCode', 1)
    expect(completion, 'observerFailuresPreserved', True)
    expect(coverage, 'currentProductMonitorAndOwnTerminal.resizeMeasuredInThisRun', False)
    expect(coverage, 'historicalNativeKeyboardResize.temporaryBackendPort', 7339)
    expect(coverage, 'historicalNativeKeyboardResize.current7337ResizeClaimed', False)
    expect(coverage, 'currentStudioInstance.unitTestsPassedClaim', False)
    for key in ('actualKernelBootIdentityChanged', 'typedBootStartupCausalChainProved', 'unitEnabledActiveRunning'):
        expect(reboot, key, True)
    for key in ('kernelBootId', 'listenerPid', 'listenerStartTicks', 'controllerPid', 'systemdManagerPid'):
        expect(linux, key, reboot[key])
    expect(linux, 'unitName', reboot['unit'])
    return {'status': 'public-android-archive-validated-readonly', 'fileCount': len(listed), 'nativeAcceptanceRerun': False, 'archiveWrites': 0, 'networkOrDeviceOperations': 0}


def main():
    require(len(sys.argv) <= 2, 'usage: validate-archive.py [archive-directory]')
    root = Path(sys.argv[1]).resolve() if len(sys.argv) == 2 else Path(__file__).resolve().parent
    try:
        result = validate(root)
    except (ValueError, OSError, UnicodeError, KeyError, TypeError, AttributeError, IndexError) as error:
        label = str(error) if isinstance(error, ArchiveError) else 'archive structure or file read failed'
        print(json.dumps({'status': 'public-android-archive-validation-failed', 'reason': label, 'archiveWrites': 0, 'nativeAcceptanceRerun': False}))
        return 1
    print(json.dumps(result))
    return 0


if __name__ == '__main__':
    sys.exit(main())
