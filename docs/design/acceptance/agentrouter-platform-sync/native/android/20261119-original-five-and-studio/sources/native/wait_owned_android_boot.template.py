# Public review template; native execution is disabled. Original source digest is in provenance-safe.json.
if __name__ == "__main__":
    raise RuntimeError("public-review-template-native-execution-disabled")
"""Prepared Windows-only boot readiness. No APK/metadata/XML/token reads."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import types

COMMON_SHA = 'e5c7c058c9d6644e4a9a11296ac4b1f04ccd8a07f534c6059f814dc6ea1e79b7'
if not sys.flags.isolated:
    raise RuntimeError('isolated-native-python-required')
module_path = Path(__file__).resolve().parent / 'native_wire_common.py'
module_raw = module_path.read_bytes()
if hashlib.sha256(module_raw).hexdigest() != COMMON_SHA:
    raise RuntimeError('owned-common-source-pin-differs')
module = types.ModuleType('native_wire_common')
module.__file__, module.__source_sha256__, module.__source_pinned_from_bytes__ = str(module_path), COMMON_SHA, True
sys.modules['native_wire_common'] = module
exec(compile(module_raw, str(module_path), 'exec'), module.__dict__)
from native_wire_common import GuardError, Lease, SOURCE, load_binding, require, sha, write_safe


def stamp():
    return datetime.now(timezone.utc).isoformat().replace('+00:00', 'Z')


def probe(lease, deadline):
    lease.guard()
    env = os.environ.copy()
    for name in ('ADB_SERVER_SOCKET', 'ANDROID_SERIAL', 'ADB_SERVER_PORT'):
        env.pop(name, None)
    env.update(ANDROID_ADB_SERVER_PORT='5041', ADB_LOCAL_TRANSPORT_MAX_PORT='0')
    command = [lease.identity['server']['image'], '-H', '127.0.0.1', '-P', '5041', '-s', 'emulator-5572',
               'shell', '-T', 'getprop', 'sys.boot_completed']
    require(time.monotonic() < deadline, 'owned-android-boot-readiness-deadline')
    client = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
    lease.clients.append(client)  # Original native creator HANDLE retained through actual exit.
    while True:
        remaining = deadline - time.monotonic()
        require(remaining > 0, 'owned-android-boot-readiness-deadline')
        try:
            raw, _private_stderr = client.communicate(timeout=min(10, remaining))
            break
        except subprocess.TimeoutExpired:
            lease.guard()
    lease.guard()
    require(client.returncode == 0, 'owned-android-boot-probe-native-exit-failed')
    require(time.monotonic() <= deadline, 'owned-android-boot-readiness-deadline')
    value = raw.decode('ascii').strip()
    require(value in ('', '0', '1'), 'owned-android-boot-property-unrecognized')
    return value == '1', client.returncode


def run(b, identity, output):
    result = {'schemaVersion': 1, 'sourceRevision': SOURCE, 'namespaceId': b['namespaceId'],
              'status': 'owned-Windows-original-AVD-boot-not-yet-proved',
              'actualAvdHost': 'Windows', 'actualAvdName': 'Medium_Phone_API_37.0',
              'serial': 'emulator-5572', 'nativeAdbServerPort': 5041, 'maximumReadinessSeconds': 180,
              'apkMetadataXmlOrTokenReads': 0, 'nativeCredentialGets': 0, 'nativeHttpRequests': 0,
              'activityOpenRequests': 0, 'appInstallResetOrWipe': False,
              'sysBootCompletedOneObserved': False, 'forcedTerminations': 0,
              'rawCommandOutputPrivateValuesOrErrorsIncluded': False}
    lease, failed, probes = None, False, 0
    try:
        require(b.get('purpose') == 'windows-original-five-readonly-app-binding', 'readonly-app-binding-purpose')
        lease = Lease(identity)
        result['currentNativeObserver'] = lease.identity['observer']
        deadline = time.monotonic() + 180
        while time.monotonic() < deadline:
            probes += 1
            ready, code = probe(lease, deadline)
            result.update(probeCount=probes, lastNativeGetpropExitCode=code)
            if ready:
                lease.guard()
                result['sysBootCompletedOneObserved'] = True
                result['nativeHostIdentityVerifiedAfterBootObservation'] = True
                break
            time.sleep(min(1, max(0, deadline - time.monotonic())))
        require(result['sysBootCompletedOneObserved'], 'owned-android-boot-readiness-deadline')
    except Exception as error:
        failed = True
        result['failure'] = {'errorType': type(error).__name__,
                             'failureCode': error.code if isinstance(error, GuardError) else 'owned-android-boot-observation-failed'}
    finally:
        if lease is not None:
            if failed:
                # Safe interim evidence is published before a timed-out native
                # client may need to remain held beyond the readiness deadline.
                interim = dict(result)
                interim.update(status='boot-readiness-failed-native-client-handles-held-until-actual-terminal',
                               observedAtUtc=stamp(), allNativeAdbCreatorHandlesActualTerminal=False)
                try:
                    write_safe(output + '.held-progress.json', interim)
                except Exception:
                    result['heldProgressPublicationFailed'] = True
            try:
                lease.close()
                result['nativeHostQueryHandlesClosed'] = True
            except Exception as error:
                failed = True
                result['closeFailure'] = {'errorType': type(error).__name__,
                    'failureCode': error.code if isinstance(error, GuardError) else 'native-query-handle-close-failed'}
            result['allNativeAdbCreatorHandlesActualTerminal'] = all(p.returncode is not None for p in lease.clients)
            result['nativeAdbCreatorActualExitCodes'] = [p.returncode for p in lease.clients]
        result['observedAtUtc'] = stamp()
        passed = not failed and result['sysBootCompletedOneObserved'] is True and result.get('allNativeAdbCreatorHandlesActualTerminal') is True
        result['status'] = 'owned-Windows-original-AVD-boot-completed-readonly-passed' if passed else 'owned-Windows-original-AVD-boot-readiness-failed'
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--bindings', required=True)
    parser.add_argument('--bindings-sha256', required=True)
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    try:
        b, identity = load_binding(args.bindings, args.bindings_sha256)
        result = run(b, identity, args.output)
        result['bindingSha256'] = args.bindings_sha256
        result['sourceSha256'] = sha(Path(__file__).read_bytes())
        write_safe(args.output, result)
        print(json.dumps(result))
        return 0 if result['status'] == 'owned-Windows-original-AVD-boot-completed-readonly-passed' else 1
    except Exception as error:
        print(json.dumps({'status': 'owned-boot-readiness-before-safe-publication-failed',
                          'errorType': type(error).__name__,
                          'failureCode': error.code if isinstance(error, GuardError) else 'owned-boot-binding-or-publication-failed',
                          'apkMetadataXmlOrTokenReads': 0, 'rawErrorOrPrivateValuesIncluded': False}))
        return 1


if __name__ == '__main__':
    sys.exit(main())
