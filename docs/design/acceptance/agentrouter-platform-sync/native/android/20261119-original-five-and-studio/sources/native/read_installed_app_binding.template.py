# Public review template; native execution is disabled. Original source digest is in provenance-safe.json.
if __name__ == "__main__":
    raise RuntimeError("public-review-template-native-execution-disabled")
"""Prepared only: native Windows app reads, no activity open or credential Get."""
import argparse
import hashlib
import json
import sys
import types
import xml.etree.ElementTree as ET
from pathlib import Path
from datetime import datetime, timezone
_OWNED_MODULE_PINS = {'native_wire_common': 'e5c7c058c9d6644e4a9a11296ac4b1f04ccd8a07f534c6059f814dc6ea1e79b7'}


def _load_owned(name):
    if not sys.flags.isolated:
        raise RuntimeError('isolated-native-python-required')
    path = Path(__file__).resolve().parent / (name + '.py')
    raw = path.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    if digest != _OWNED_MODULE_PINS[name]:
        raise RuntimeError('owned-module-source-pin-differs')
    existing = sys.modules.get(name)
    if existing is not None:
        if getattr(existing, '__source_sha256__', None) != digest or getattr(existing, '__source_pinned_from_bytes__', False) is not True:
            raise RuntimeError('unexpected-owned-module-cache')
        return existing
    module = types.ModuleType(name)
    module.__file__, module.__source_sha256__, module.__source_pinned_from_bytes__ = str(path), digest, True
    sys.modules[name] = module
    exec(compile(raw, str(path), 'exec'), module.__dict__)
    return module


_load_owned('native_wire_common')
from native_wire_common import APK_SHA, BASELINE_XML, BASELINE_XML_SHA, CERT_SHA, PACKAGE, SOURCE, GuardError, Lease, load_binding, require, sha, write_safe


def encrypted_pairs(raw):
    entries = ET.fromstring(raw).findall('string')
    pairs = {e.attrib['name']: e.text or '' for e in entries}
    require(len(pairs) == len(entries), 'duplicate-credential-key')
    return pairs


def run(b, identity):
    require(b.get('purpose') == 'windows-original-five-readonly-app-binding', 'readonly-app-binding-purpose')
    lease = Lease(identity)
    try:
        paths, path_rc = lease.adb(['shell', '-T', 'pm', 'path', PACKAGE])
        rows = [line.removeprefix('package:') for line in paths.decode().splitlines() if line.startswith('package:')]
        require(len(rows) == 1 and rows[0].startswith('/data/app/') and rows[0].endswith('/base.apk'), 'installed-apk-path')
        apk, apk_rc = lease.adb(['exec-out', 'cat', rows[0]])
        apk_sha = sha(apk)
        require(apk_sha == APK_SHA, 'installed-apk-pin')
        uid_raw, uid_rc = lease.adb(['shell', '-T', 'run-as', PACKAGE, 'id', '-u'])
        require(uid_raw.decode().strip().isdigit(), 'actual-app-uid-shape')
        uid = int(uid_raw.decode().strip())
        require(uid >= 10000, 'actual-app-uid-range')
        require(str(Path(b['baselineEncryptedXml'])).lower() == str(Path(BASELINE_XML)).lower(), 'original-encrypted-baseline-path')
        baseline_available = Path(b['baselineEncryptedXml']).exists()
        if baseline_available:
            original = Path(b['baselineEncryptedXml']).read_bytes()
        else:
            original, _ = lease.adb(['exec-out', 'run-as', PACKAGE, 'cat', 'shared_prefs/remote_credentials.xml'])
        require(sha(original) == BASELINE_XML_SHA, 'original-encrypted-baseline-pin')
        original_pairs = encrypted_pairs(original)
        require(len(original_pairs) == 5, 'original-five-count')
        current, xml_rc = lease.adb(['exec-out', 'run-as', PACKAGE, 'cat', 'shared_prefs/remote_credentials.xml'])
        current_pairs = encrypted_pairs(current)
        retained = all(current_pairs.get(key) == value for key, value in original_pairs.items())
        require(retained, 'original-five-encrypted-pairs-differ')
        lease.guard()
        return {
            'schemaVersion': 1, 'status': 'current-owned-Windows-installed-app-binding-readonly-passed',
            'observedAtUtc': datetime.now(timezone.utc).isoformat(), 'sourceRevision': SOURCE,
            'namespaceId': b['namespaceId'], 'identityReceiptSha256': b['identitySha256'],
            'currentNativeObserver': lease.identity['observer'],
            'actualAvdHost': 'Windows', 'actualAvdName': 'Medium_Phone_API_37.0',
            'nativeAdbServerPort': 5041, 'serial': 'emulator-5572', 'actualRunAsAppUid': uid,
            'nativePmPathExitCode': path_rc, 'nativeInstalledApkReadExitCode': apk_rc,
            'nativeInstalledApkBytes': len(apk), 'nativeInstalledApkSha256': apk_sha,
            'certificateSha256BoundByByteExactExpectedApk': CERT_SHA,
            'certificateIndependentlyReparsedThisJob': False, 'nativeUidReadExitCode': uid_rc,
            'nativeEncryptedXmlReadExitCode': xml_rc, 'currentEncryptedXmlSha256': sha(current),
            'currentEncryptedRecordCount': len(current_pairs), 'originalEncryptedRecordCount': 5,
            'originalFiveEncryptedKeyValuesExactlyRetained': retained,
            'byteExactOriginalXmlClaimedByReadonlyBinding': False,
            'originalEncryptedBaselineSha256': BASELINE_XML_SHA,
            'originalBaselinePrivateBytesAvailable': baseline_available,
            'currentXmlByteExactKnownOriginalSha256Matched': sha(current) == BASELINE_XML_SHA,
            'currentXmlNeverArchivedAsNewBaseline': True,
            'freshCurrentXmlStoredInMemoryOnly': True, 'nativeCredentialGets': 0,
            'nativeHttpRequests': 0, 'activityOpenRequests': 0, 'appInstallOrResetOrWipe': False,
            'nativeHostQueryHandlesHeldThroughReads': True, 'allNativeAdbCreatorHandlesActualTerminal': True,
            'currentWindowsFiveWireAccepted': False, 'privateXmlKeysValuesAddressesOrTokensPrinted': False,
        }
    finally:
        lease.close()


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--bindings', required=True)
    p.add_argument('--bindings-sha256', required=True)
    p.add_argument('--output', required=True)
    args = p.parse_args()
    code = 1
    try:
        b, identity = load_binding(args.bindings, args.bindings_sha256)
        result = run(b, identity)
        code = 0
    except Exception as error:
        result = {'schemaVersion': 1, 'status': 'current-Windows-installed-app-binding-readonly-failed',
                  'errorType': type(error).__name__, 'failureCode': error.code if isinstance(error, GuardError) else 'readonly-native-observation-failed',
                  'rawErrorOrPrivateValuesIncluded': False,
                  'nativeCredentialGets': 0, 'nativeHttpRequests': 0, 'activityOpenRequests': 0,
                  'currentWindowsFiveWireAccepted': False}
    result['sourceSha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    write_safe(args.output, result)
    print(json.dumps(result))
    return code


if __name__ == '__main__':
    sys.exit(main())
