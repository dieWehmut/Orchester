# Public review template; native execution is disabled. Original source digest is in provenance-safe.json.
if __name__ == "__main__":
    raise RuntimeError("public-review-template-native-execution-disabled")
"""Prepared Windows-only original-five native reads and one original7337 GET.

Nothing starts at import. All private baseline data stays in execution memory.
Root supplies a new pinned binding only after the actual read-only app UID job.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import time
import types

_OWNED_MODULE_PINS = {'native_wire_common': 'e5c7c058c9d6644e4a9a11296ac4b1f04ccd8a07f534c6059f814dc6ea1e79b7', 'read_installed_app_binding': '985e0dddfc8f9c3777320534316ed33c65f82b137a30cf1f023b118112a075ea', 'cdp_transport_owned': 'd4d66a18e453cbaf7e6e71bc036ad40372e54f6d633429254b6961817d07cf05'}


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


for _owned_name in _OWNED_MODULE_PINS:
    _load_owned(_owned_name)
from native_wire_common import APK_SHA, BASELINE_XML_SHA, CERT_SHA, PACKAGE, SOURCE, GuardError, Lease, fresh_receipt, load_binding, pinned_json, require, sha, write_safe
from read_installed_app_binding import encrypted_pairs
from cdp_transport_owned import CDP

BASELINE_DIRECTORY = r'<USERPROFILE>\AppData\Local\Temp\Orchester-final20-android-20261111'
DEX_SHA = 'df15578ba781311a7a00377af05c387731c663906ec4174d7a8f46ee9a5ebd31'
RUNTIME_BASE = r'<USERPROFILE>\AppData\Local\Orchester-build\android5-current-wire-runtime-20261113'


def stamp():
    return datetime.now(timezone.utc).isoformat().replace('+00:00', 'Z')


def private_file(b, field, filename, pin):
    path = Path(b[field])
    require(os.path.normcase(str(path)) == os.path.normcase(str(Path(BASELINE_DIRECTORY) / filename)), 'original-private-reference-path-differs')
    raw = path.read_bytes()
    require(sha(raw) == pin, 'original-private-reference-pin-differs')
    return raw


def app_identity(lease, expected_uid):
    raw, rc = lease.adb(['shell', '-T', 'pidof', PACKAGE])
    pids = raw.decode().strip().split()
    require(len(pids) == 1 and pids[0].isdigit(), 'native-app-primary-pid-not-unique')
    pid = int(pids[0])
    status, _ = lease.adb(['exec-out', 'run-as', PACKAGE, 'cat', '/proc/' + str(pid) + '/status'])
    uids = [line.split()[1:] for line in status.decode().splitlines() if line.startswith('Uid:')]
    require(len(uids) == 1 and len(uids[0]) == 4 and all(value == str(expected_uid) for value in uids[0]), 'native-app-process-uid-differs')
    cmdline, _ = lease.adb(['exec-out', 'run-as', PACKAGE, 'cat', '/proc/' + str(pid) + '/cmdline'])
    require(cmdline.rstrip(b'\0') == PACKAGE.encode(), 'native-app-process-command-differs')
    raw_stat, _ = lease.adb(['exec-out', 'run-as', PACKAGE, 'cat', '/proc/' + str(pid) + '/stat'])
    fields = raw_stat.decode().rsplit(')', 1)[1].split()
    require(len(fields) > 19 and fields[19].isdigit(), 'native-app-starttime-unreadable')
    return {'pid': pid, 'startTicks': fields[19], 'appUid': expected_uid, 'pidofActualExitCode': rc}


def wait_started_app_identity(lease, expected_uid, maximum_seconds=45):
    deadline = time.monotonic() + maximum_seconds
    started = time.monotonic()
    probes = 0
    while True:
        raw, rc = lease.adb(['shell', '-T', 'pidof', PACKAGE], allowed=(0, 1))
        probes += 1
        if rc == 0:
            pids = raw.decode().strip().split()
            require(len(pids) == 1 and pids[0].isdigit(), 'native-started-app-primary-pid-not-unique')
            app = app_identity(lease, expected_uid)
            require(app['pid'] == int(pids[0]), 'native-started-app-pid-changed-during-readiness')
            return app, {'status': 'normal-started-app-native-identity-ready', 'pidofProbeCount': probes,
                         'maximumWaitSeconds': maximum_seconds, 'elapsedSeconds': round(time.monotonic() - started, 3),
                         'onlyPidofExitOneWithEmptyOutputRetried': True, 'identityChecksRetained': True,
                         'forcedTerminations': 0}
        require(not raw.strip(), 'native-started-app-exit-one-with-unexpected-output')
        require(time.monotonic() < deadline, 'normal-started-app-native-identity-readiness-timeout')
        time.sleep(min(.25, max(0, deadline - time.monotonic())))


def installed_uid_and_xml(lease, expected_uid, original_pairs):
    raw, path_rc = lease.adb(['shell', '-T', 'pm', 'path', PACKAGE])
    rows = [line.removeprefix('package:') for line in raw.decode().splitlines() if line.startswith('package:')]
    require(len(rows) == 1 and rows[0].startswith('/data/app/') and rows[0].endswith('/base.apk'), 'installed-apk-path-differs')
    apk, apk_rc = lease.adb(['exec-out', 'cat', rows[0]])
    require(sha(apk) == APK_SHA, 'current-installed-apk-sha-differs')
    raw_uid, uid_rc = lease.adb(['shell', '-T', 'run-as', PACKAGE, 'id', '-u'])
    require(raw_uid.decode().strip() == str(expected_uid), 'current-actual-run-as-uid-differs')
    xml, xml_rc = lease.adb(['exec-out', 'run-as', PACKAGE, 'cat', 'shared_prefs/remote_credentials.xml'])
    pairs = encrypted_pairs(xml)
    require((sha(xml) == BASELINE_XML_SHA and len(pairs) == 5) if original_pairs is None else all(pairs.get(k) == v for k, v in original_pairs.items()), 'current-original-five-encrypted-pairs-differ')
    return xml, {'nativePmPathExitCode': path_rc, 'nativeInstalledApkReadExitCode': apk_rc,
                 'nativeUidReadExitCode': uid_rc, 'nativeEncryptedXmlReadExitCode': xml_rc,
                 'nativeInstalledApkSha256': APK_SHA, 'nativeInstalledApkBytes': len(apk),
                 'actualRunAsAppUid': expected_uid, 'currentEncryptedRecordCount': len(pairs)}


def forward_rows(lease):
    raw, _ = lease.adb(['forward', '--list'])
    rows = [line.split() for line in raw.decode().splitlines() if line.strip()]
    require(all(len(row) == 3 for row in rows), 'native-forward-list-shape')
    return rows


def owned_forward(lease, socket_name):
    rows = [row for row in forward_rows(lease) if row[1] == 'tcp:19330']
    return rows == [['emulator-5572', 'tcp:19330', 'localabstract:' + socket_name]]


def validate(b, identity):
    require(b.get('purpose') == 'windows-original-five-native-current-wire', 'wire-binding-purpose')
    expected_uid = b.get('expectedActualRunAsAppUid')
    require(type(expected_uid) is int and expected_uid >= 10000, 'fresh-actual-native-app-uid-required')
    require(re.fullmatch('[a-f0-9]{64}', b.get('baselineMetadataSha256', '')) is not None, 'original-metadata-pin-required')
    require(re.fullmatch('[a-f0-9]{64}', b.get('expressionSha256', '')) is not None
            and sha(Path(__file__).with_name('native_five_wire_expression.js').read_bytes()) == b['expressionSha256'],
            'native-expression-preflight-pin-differs')
    gate = pinned_json(b['installedAppBindingFile'], b['installedAppBindingSha256'])
    require(gate.get('schemaVersion') == 1 and gate.get('status') == 'current-owned-Windows-installed-app-binding-readonly-passed'
            and gate.get('sourceRevision') == SOURCE and gate.get('namespaceId') == b['namespaceId'], 'fresh-installed-app-binding-required')
    fresh_receipt(gate)
    require(gate.get('identityReceiptSha256') == b['identitySha256'] and gate.get('actualRunAsAppUid') == expected_uid
            and gate.get('nativeInstalledApkSha256') == APK_SHA and gate.get('originalFiveEncryptedKeyValuesExactlyRetained') is True,
            'actual-installed-app-binding-differs')
    linux = pinned_json(b['currentManagedLinux7337File'], b['currentManagedLinux7337Sha256'])
    require(linux.get('schemaVersion') == 1 and linux.get('status') == 'current-managed-linux7337-owner-verified'
            and linux.get('sourceRevision') == SOURCE and linux.get('targetHost') == 'Linux' and linux.get('port') == 7337
            and linux.get('currentListeningSocketOwned') is True and linux.get('managedUnitActive') is True,
            'fresh-root-linux7337-owner-binding-required')
    require(type(linux.get('listenerPid')) is int and linux['listenerPid'] > 0
            and type(linux.get('listenerStartTicks')) is int and linux['listenerStartTicks'] > 0
            and re.fullmatch('[a-f0-9]{64}', linux.get('listenerElfSha256', '')) is not None,
            'root-linux7337-owner-identity-incomplete')
    fresh_receipt(linux)
    require(Path(b['aliasDexFile']).resolve() == (Path(__file__).resolve().parent / 'dex' / 'classes.dex')
            and b.get('aliasDexSha256') == DEX_SHA and sha(Path(b['aliasDexFile']).read_bytes()) == DEX_SHA,
            'diagnostic-dex-pin-differs')
    if Path(b['baselineEncryptedXml']).exists():
        original = private_file(b, 'baselineEncryptedXml', 'credentials-before-private-stdout.log', BASELINE_XML_SHA)
        original_pairs = encrypted_pairs(original)
        require(len(original_pairs) == 5, 'original-encrypted-five-required')
    else:
        require(gate.get('currentXmlByteExactKnownOriginalSha256Matched') is True, 'known-original-encrypted-sha-required')
        original_pairs = None
    devices = None
    if Path(b['baselineMetadataJson']).exists():
        metadata = private_file(b, 'baselineMetadataJson', 'baseline-private.json', b['baselineMetadataSha256'])
        raw_devices = json.loads(metadata.decode('utf-8-sig'))['localStorage']['orchester.devices']
        devices = json.loads(raw_devices)
        require(type(devices) is list and len(devices) > 0 and all(type(d) is dict and isinstance(d.get('address'), str)
                and isinstance(d.get('name'), str) for d in devices), 'original-saved-device-metadata-shape')
    output = Path(b['outputFile'])
    require(os.path.normcase(str(output.parent.parent)) == os.path.normcase(RUNTIME_BASE)
            and output.parent.name == 'runtime-' + b['namespaceId']
            and re.fullmatch('native-five-wire-[a-f0-9]{32}-safe\\.json', output.name) is not None,
            'new-owned-safe-output-required')
    require(not output.exists(), 'safe-output-already-exists')
    return expected_uid, gate, linux, original_pairs, devices


def product_ui_acceptance(lease, cdp, devices, result):
    targets = [d for d in (devices or []) if d.get('address') and str(d['address']).split(':')[-1].rstrip('/') == '7337']
    require(devices is None or len(targets) == 1, 'normal-ui-original7337-target-not-unique')
    private_address = json.dumps(targets[0]['address']) if devices is not None else None
    result.update(schemaVersion=1, userValuesReturned=False, terminalCreatedByThisTask=False)
    def evaluate(expression):
        lease.guard(require_cdp=True)
        if private_address is None:
            expression = expression.replace("h.querySelector('.row-copy small')?.textContent===__PRIVATE_ADDRESS__", "(()=>{try{const a=h.querySelector('.row-copy small')?.textContent||'';return new URL(a.includes('://')?a:'http://'+a).port==='7337'}catch{return false}})()")
            expression = expression.replace("h.querySelector(':scope > .project-path')?.textContent===__PRIVATE_ADDRESS__", "(()=>{try{const a=h.querySelector(':scope > .project-path')?.textContent||'';return new URL(a.includes('://')?a:'http://'+a).port==='7337'}catch{return false}})()")
        else:
            expression = expression.replace('__PRIVATE_ADDRESS__', private_address)
        return cdp.eval(expression)
    def wait(expression, seconds=30):
        deadline = time.monotonic() + seconds
        while True:
            value = evaluate(expression)
            if value.get('passed') is True:
                return value
            require(time.monotonic() < deadline, 'normal-product-ui-bounded-wait-failed')
            time.sleep(.25)
    result['dashboardNavigation'] = evaluate("""(() => {const b=[...document.querySelectorAll('nav[aria-label="主导航"] button')].filter(b=>b.textContent.trim()==='控制台');if(b.length!==1||b[0].disabled)throw Error('dashboard-navigation');b[0].click();return {normalProductDashboardClicked:true};})()""")
    monitor_expression = """(() => {const hosts=[...document.querySelectorAll('.device-dashboard')].filter(h=>h.querySelector('.row-copy small')?.textContent===__PRIVATE_ADDRESS__);if(hosts.length!==1)return {passed:false,targetHostCount:hosts.length};const h=hosts[0],s=h.querySelector('[data-mobile-agent-monitors]');if(!s)return {passed:false,targetMonitorCount:0};const rows=[...s.querySelectorAll('[data-mobile-agent-id]')],images=[...s.querySelectorAll('.agent-icon img')],ids=[...s.querySelectorAll('[data-agent-icon]')].map(i=>i.dataset.agentIcon);const connected=h.querySelector('.connection-badge')?.dataset.status==='connected',notices=s.querySelectorAll(':scope > p').length,loaded=images.every(i=>i.complete&&i.naturalWidth>0),visible=document.querySelector('h1')?.textContent==='控制台';return {passed:visible&&connected&&notices===0&&rows.length===21&&new Set(ids).size===21&&!ids.includes('generic')&&images.length===42&&loaded,nativeProductDashboardVisible:visible,connected,snapshotNoticeCount:notices,providerRowCount:rows.length,uniqueCanonicalProviderCount:new Set(ids).size,themeImageCount:images.length,darkImageCount:s.querySelectorAll('.agent-icon__dark').length,lightImageCount:s.querySelectorAll('.agent-icon__light').length,allThemeImagesLoaded:loaded,userValuesReturned:false};})()"""
    result['actualProductMonitor'] = wait(monitor_expression, 40)
    result['projectNavigation'] = evaluate("""(() => {const h=[...document.querySelectorAll('.device-dashboard')].filter(h=>h.querySelector('.row-copy small')?.textContent===__PRIVATE_ADDRESS__);const b=h.length===1?[...h[0].querySelectorAll('.output-actions button')].filter(b=>b.textContent.trim()==='项目'):[];if(b.length!==1||b[0].disabled)throw Error('project-navigation');b[0].click();return {normalTargetProjectButtonClicked:true};})()""")
    project_expression = """(() => {const h=[...document.querySelectorAll('.project-device')].filter(h=>h.querySelector(':scope > .project-path')?.textContent===__PRIVATE_ADDRESS__);if(h.length!==1)return {passed:false,targetHostCount:h.length};const projects=[...h[0].querySelectorAll('.project-grid article')].filter(p=>p.querySelector('.project-path')?.textContent==='<AUTHORIZED_WORKSPACE_PATH>');const b=projects.length===1?projects[0].querySelector('.project-terminal'):null;return {passed:!!b&&!b.disabled,currentWorkspaceProjectCount:projects.length,enterTerminalButtonEnabled:!!b&&!b.disabled,userValuesReturned:false};})()"""
    result['currentWorkspaceProject'] = wait(project_expression, 30)
    result['terminalNavigation'] = evaluate("""(() => {const h=[...document.querySelectorAll('.project-device')].filter(h=>h.querySelector(':scope > .project-path')?.textContent===__PRIVATE_ADDRESS__);const p=h.length===1?[...h[0].querySelectorAll('.project-grid article')].filter(p=>p.querySelector('.project-path')?.textContent==='<AUTHORIZED_WORKSPACE_PATH>'):[];const b=p.length===1?p[0].querySelector('.project-terminal'):null;if(!b||b.disabled)throw Error('terminal-navigation');b.click();return {normalProductEnterTerminalClicked:true};})()""")
    ready = wait("""(() => {const p=document.querySelector('.terminal-page'),b=p?.querySelector('.terminal-new');return {passed:!!p&&!!b&&!b.disabled&&!!p.querySelector('.xterm-helper-textarea'),nativeTerminalPageVisible:!!p,newTerminalButtonEnabled:!!b&&!b.disabled,xtermInputRendered:!!p?.querySelector('.xterm-helper-textarea'),userValuesReturned:false};})()""", 30)
    result['actualTerminalPage'] = ready
    # Only the session short ID hash leaves the page; no user output is returned.
    session_hash = """(async()=>{const d=document.querySelector('.terminal-toolbar .picker-detail')?.textContent||'';const bytes=await crypto.subtle.digest('SHA-256',new TextEncoder().encode(d));return {sessionShortIdHash:[...new Uint8Array(bytes)].map(x=>x.toString(16).padStart(2,'0')).join(''),sessionSelected:!!d};})()"""
    old = evaluate(session_hash)
    result['newTerminalClick'] = evaluate("""(() => {const b=document.querySelector('.terminal-new');if(!b||b.disabled)throw Error('new-terminal');b.click();return {normalNewTerminalButtonClicked:true};})()""")
    deadline = time.monotonic() + 30
    own_hash = None
    while True:
        selected = evaluate(session_hash)
        if selected['sessionSelected'] and selected['sessionShortIdHash'] != old['sessionShortIdHash']:
            own_hash = selected['sessionShortIdHash']
            break
        require(time.monotonic() < deadline, 'own-new-terminal-selection-not-observed')
        time.sleep(.25)
    result.update(terminalCreatedByThisTask=True,ownSelectedTerminalShortIdSha256=own_hash,existingSessionInputCommandsSent=0)
    try:
        result['actualOwnTerminalConnection'] = wait("""(() => {const p=document.querySelector('.terminal-page'),c=p?.querySelector('.terminal-connection'),t=p?.querySelector('.xterm-helper-textarea');return {passed:c?.dataset.connected==='true'&&!!t,connected:c?.dataset.connected==='true',xtermInputRendered:!!t,userValuesReturned:false};})()""", 30)
        require(evaluate(session_hash)['sessionShortIdHash']==own_hash, 'own-terminal-selection-changed-before-input')
        evaluate("""(() => {const t=document.querySelector('.xterm-helper-textarea');if(!t)throw Error('terminal-input');t.focus();return {ownTerminalInputFocused:document.activeElement===t};})()""")
        cdp.call('Input.insertText', {'text': "printf 'ORCHESTER_ANDROID_20261119_ACCEPTANCE_OK\\n'"})
        cdp.call('Input.dispatchKeyEvent', {'type':'keyDown','key':'Enter','code':'Enter','windowsVirtualKeyCode':13,'nativeVirtualKeyCode':13})
        cdp.call('Input.dispatchKeyEvent', {'type':'keyUp','key':'Enter','code':'Enter','windowsVirtualKeyCode':13,'nativeVirtualKeyCode':13})
        result['actualOwnTerminalOutput'] = wait("""(() => {const s=document.querySelector('.terminal-surface')?.textContent||'';const count=s.split('ORCHESTER_ANDROID_20261119_ACCEPTANCE_OK').length-1;return {passed:count>=2,staticAcceptanceMarkerOccurrences:count,userOutputReturned:false};})()""", 15)
    finally:
        require(evaluate(session_hash)['sessionShortIdHash']==own_hash, 'own-terminal-selection-changed-before-normal-exit')
        evaluate("""(() => {const t=document.querySelector('.xterm-helper-textarea');if(!t)throw Error('terminal-input');t.focus();return {ownTerminalInputFocused:document.activeElement===t};})()""")
        cdp.call('Input.insertText', {'text':'exit'})
        cdp.call('Input.dispatchKeyEvent', {'type':'keyDown','key':'Enter','code':'Enter','windowsVirtualKeyCode':13,'nativeVirtualKeyCode':13})
        cdp.call('Input.dispatchKeyEvent', {'type':'keyUp','key':'Enter','code':'Enter','windowsVirtualKeyCode':13,'nativeVirtualKeyCode':13})
        result['actualOwnTerminalNormalExit'] = wait("""(() => {const p=document.querySelector('.terminal-page'),e=p?.querySelector('.terminal-exit'),c=p?.querySelector('.terminal-connection');return {passed:!!e&&c?.dataset.connected==='false',terminalExitVisible:!!e,ownTerminalNoLongerConnected:c?.dataset.connected==='false',normalShellExitCommandSent:true,forcedCloseInvoked:false,userValuesReturned:false};})()""", 30)
    result['status'] = 'actual-native-product-monitor-and-own-terminal-passed'
    return result


def run(b, identity):
    result = {'schemaVersion': 1, 'status': 'current-Windows-original-five-wire-not-yet-observed',
              'sourceRevision': SOURCE, 'namespaceId': b['namespaceId'], 'actualAvdHost': 'Windows',
              'actualAvdName': 'Medium_Phone_API_37.0', 'serial': 'emulator-5572',
              'nativeAdbServerPort': 5041, 'cdpPort': 19330, 'originalEncryptedBaselineSha256': BASELINE_XML_SHA,
              'appInstallOrResetOrWipe': False, 'newPairingUsed': False, 'credentialSetOrRemoveInvoked': False,
              'currentWindowsFiveWireAccepted': False, 'rawXmlKeysValuesAddressesNamesOrTokensIncluded': False,
              'automaticStartupReconnectRequestsMeasured': False, 'forcedTerminations': 0,
              'diagnosticDexUploaded': False, 'ownForwardCreated': False, 'cleanupComplete': False}
    lease, cdp, dex_owned, forward_created, app_opened = None, None, False, False, False
    failure, phase, remote, socket_name, app = None, 'binding-preflight', None, None, None
    try:
        uid, app_gate, linux, original_pairs, devices = validate(b, identity)
        result.update(actualRunAsAppUid=uid, installedAppBindingSha256=b['installedAppBindingSha256'],
                      managerBindingSha256=b['managerBindingSha256'], identityReceiptSha256=b['identitySha256'],
                      rootCurrentLinux7337OwnerReceiptSha256=b['currentManagedLinux7337Sha256'],
                      rootLinux7337ListenerPid=linux['listenerPid'], rootLinux7337ListenerStartTicks=linux['listenerStartTicks'],
                      rootLinuxOwnerProofIndependentlyReverifiedByWindows=False)
        lease = Lease(identity)
        result['currentNativeObserver'] = lease.identity['observer']
        phase = 'current-installed-app-and-original-encrypted-five'
        before_xml, reads = installed_uid_and_xml(lease, uid, original_pairs)
        require(sha(before_xml) == app_gate['currentEncryptedXmlSha256'], 'encrypted-xml-changed-since-actual-app-binding')
        if original_pairs is None:
            original_pairs = encrypted_pairs(before_xml)
        result.update(reads, beforeEncryptedXmlSha256=sha(before_xml), originalFiveEncryptedKeyValuesExactlyRetainedBefore=True,
            originalBaselinePrivateBytesAvailable=app_gate.get('originalBaselinePrivateBytesAvailable', True),
            currentXmlByteExactKnownOriginalSha256Matched=sha(before_xml) == BASELINE_XML_SHA,
            originalSavedMetadataBaselineFileAvailable=devices is not None,
            originalSavedMetadataBaselineIndependentlyReverified=devices is not None,
            savedMetadataObservationScope='original-baseline-and-current' if devices is not None else 'current-product-memory-before-and-after-no-value-rewrite')
        phase = 'existing-native-aes-alias-before-product-open'
        remote = '/data/local/tmp/orchester-five-alias-' + b['namespaceId'] + '.dex'
        lease.adb(['shell', '-T', '/system/bin/test', '!', '-e', remote])
        dex_owned = True
        result['diagnosticDexUploadAttempted'] = True
        lease.adb(['push', b['aliasDexFile'], remote])
        uploaded, _ = lease.adb(['exec-out', 'cat', remote])
        require(sha(uploaded) == DEX_SHA, 'own-diagnostic-dex-upload-sha-differs')
        result['diagnosticDexUploaded'] = True
        lease.adb(['shell', '-T', 'chmod', '444', remote])
        checked_dex, _ = lease.adb(['exec-out', 'cat', remote])
        require(sha(checked_dex) == DEX_SHA, 'owned-diagnostic-readonly-dex-hash-differs')
        _, dex_writable_rc = lease.adb(['shell', '-T', 'run-as', PACKAGE, '/system/bin/test', '-w', remote], allowed=(0, 1))
        require(dex_writable_rc == 1, 'owned-diagnostic-dex-still-app-writable')
        result['diagnosticDexReadonlyBeforeClassLoading'] = True
        result['diagnosticDexWritableProbeActualExitCode'] = dex_writable_rc
        alias_raw, alias_rc = lease.adb(['shell', '-T', 'run-as', PACKAGE, '/system/bin/env', 'CLASSPATH=' + remote,
                                       '/system/bin/app_process', '/system/bin', 'ExistingWindowsAliasGuard', str(uid)], allowed=(0, 1, 2))
        result['existingNativeAliasCheckActualExitCode'] = alias_rc
        alias = json.loads(alias_raw.decode().strip())
        result.update(existingNativeKeystoreAliasPresentBeforeCredentialGet=alias.get('existingAliasPresent'),
                      existingNativeAliasCheckActualExitCode=alias_rc, aliasGuardDexSha256=DEX_SHA,
                      keyGenerationCalls=alias.get('keyGenerationCalls'), aliasGuardCredentialReads=alias.get('credentialReads'))
        require(alias_rc == 0 and alias.get('existingAliasPresent') is True and alias.get('appUid') == uid
                and alias.get('expectedNativeAppUidMatched') is True and alias.get('keyGenerationCalls') == 0
                and alias.get('credentialReads') == 0, 'existing-native-aes-alias-not-proved')
        phase = 'normal-owned-product-open-and-own-forward'
        result['activityOpenRequestAttempted'] = True
        _, activity_rc = lease.adb(['shell', '-T', 'am', 'start', '-n', PACKAGE + '/.MainActivity'], allowed=(0, 1))
        result['normalActivityOpenActualExitCode'] = activity_rc
        require(activity_rc == 0, 'normal-owned-activity-open-failed')
        app_opened = True
        app, readiness = wait_started_app_identity(lease, uid)
        result['normalActivityNativeReadiness'] = readiness
        result['nativeAppIdentity'] = app
        require(not any(row['port'] == 19330 for row in lease.tcp_rows()), 'own-cdp-port-no-longer-free')
        require(not any(row[1] == 'tcp:19330' for row in forward_rows(lease)), 'own-cdp-forward-already-present')
        socket_name = 'webview_devtools_remote_' + str(app['pid'])
        forward_created = True
        result['ownForwardCreateAttempted'] = True
        lease.adb(['forward', '--no-rebind', 'tcp:19330', 'localabstract:' + socket_name])
        require(owned_forward(lease, socket_name), 'own-cdp-forward-binding-differs')
        lease.guard(require_cdp=True)
        result.update(ownForwardCreated=True, ownForwardTargetPid=app['pid'])
        phase = 'native-original-five-and-one-original7337-get'
        deadline = time.monotonic() + 45
        while cdp is None:
            lease.guard(require_cdp=True)
            require(app_identity(lease, uid) == app, 'native-app-identity-changed-before-observer')
            try:
                cdp = CDP(19330)
            except Exception:
                if time.monotonic() >= deadline:
                    raise GuardError('owned-cdp-page-not-ready')
                time.sleep(.25)
        expression_path = Path(__file__).with_name('native_five_wire_expression.js')
        expression = expression_path.read_text(encoding='utf-8')
        require(sha(expression_path.read_bytes()) == b['expressionSha256'], 'native-expression-pin-differs')
        require(expression.count('__ORIGINAL_KEYS__') == 1 and expression.count('__ORIGINAL_DEVICES__') == 1, 'native-expression-template-differs')
        expression = expression.replace('__ORIGINAL_KEYS__', json.dumps(list(original_pairs)))
        expression = expression.replace('__ORIGINAL_DEVICES__', json.dumps(devices) if devices is not None else "JSON.parse(localStorage.getItem('orchester.devices') || '[]')")
        lease.guard(require_cdp=True)
        fresh_receipt(linux)
        require(owned_forward(lease, socket_name), 'own-cdp-forward-changed-before-native-observer')
        observed = cdp.eval(expression)
        require(owned_forward(lease, socket_name), 'own-cdp-forward-changed-during-native-observer')
        require(type(observed) is dict and observed.get('addressesNamesCiphertextOrTokensReturned') is False, 'native-observation-safe-contract')
        # Accept only the static safe field names from this pinned expression.
        allowed = {'status', 'nativePlatform', 'platformAndroid', 'originalEncryptedRecordCount', 'nativeCredentialGetCount',
                   'decryptSucceededCount', 'decryptFailedCount', 'originalSaved7337CandidateCount', 'nativeHttpGetAttemptCount',
                   'original7337GetStatusCodes', 'original7337InfoNameMatched', 'originalSavedOriginUsedWithoutRewrite',
                   'requestsOnlyOriginal7337Info', 'originalSavedDeviceRecordsExactlyRetained', 'currentSavedDeviceStorageRawUnchanged',
                   'newPairingUsed', 'credentialSetOrRemoveInvoked', 'addressesNamesCiphertextOrTokensReturned',
                   'automaticStartupReconnectRequestsMeasured', 'observerExplicitRequestScopeOnly'}
        require(set(observed) <= allowed, 'native-observation-unexpected-field')
        observed['originalSavedMetadataBaselineIndependentlyReverified'] = devices is not None
        result['nativeObservation'] = observed
        require(observed.get('nativePlatform') is True and observed.get('platformAndroid') is True
                and observed.get('nativeCredentialGetCount') == 5 and observed.get('decryptSucceededCount') == 5
                and observed.get('decryptFailedCount') == 0 and observed.get('nativeHttpGetAttemptCount') == 1
                and observed.get('original7337GetStatusCodes') == [200] and observed.get('original7337InfoNameMatched') is True
                and observed.get('originalSavedDeviceRecordsExactlyRetained') is True
                and observed.get('currentSavedDeviceStorageRawUnchanged') is True, 'native-original-five-or-original7337-observation-failed')
        lease.guard(require_cdp=True)
        require(app_identity(lease, uid) == app, 'native-app-identity-changed-during-observer')
        phase = 'actual-native-product-monitor-and-own-terminal'
        result['actualNativeProductUi'] = {}
        product_ui_acceptance(lease, cdp, devices, result['actualNativeProductUi'])
        after_xml, after_reads = installed_uid_and_xml(lease, uid, original_pairs)
        require(after_xml == before_xml, 'original-encrypted-xml-mutated-during-observer')
        lease.guard(require_cdp=True)
        result.update(afterEncryptedXmlSha256=sha(after_xml), originalFiveEncryptedKeyValuesExactlyRetainedAfter=True,
                      currentEncryptedXmlBytesExactlyUnchanged=True, nativeHostQueryHandlesHeldThroughReads=True,
                      certificateSha256BoundByByteExactExpectedApk=CERT_SHA, certificateIndependentlyReparsedThisJob=False,
                      explicitObserverNativeFiveAndOriginal7337Passed=True)
    except Exception as error:
        failure = {'phase': phase, 'errorType': type(error).__name__,
                   'failureCode': error.code if isinstance(error, GuardError) else 'native-wire-or-input-observation-failed'}
    finally:
        cleanup_errors = []
        if cdp is not None:
            try:
                cdp.close()
            except Exception as error:
                cleanup_errors.append({'operation': 'own-cdp-socket-close', 'errorType': type(error).__name__,
                                       'failureCode': 'own-cdp-socket-close-failed'})
        if lease is not None:
            if app_opened:
                try:
                    attempts = 0
                    while attempts < 6:
                        top, _ = lease.adb(['shell', '-T', 'dumpsys', 'activity', 'activities'])
                        resumed = any(PACKAGE in line for line in top.decode(errors='replace').splitlines()
                                      if 'topResumedActivity' in line or 'mResumedActivity' in line)
                        if not resumed:
                            break
                        lease.adb(['shell', '-T', 'input', 'keyevent', '4'])
                        attempts += 1
                        time.sleep(.3)
                    top, _ = lease.adb(['shell', '-T', 'dumpsys', 'activity', 'activities'])
                    resumed = any(PACKAGE in line for line in top.decode(errors='replace').splitlines()
                                  if 'topResumedActivity' in line or 'mResumedActivity' in line)
                    require(not resumed, 'normal-product-back-exit-not-observed')
                    result['normalProductBackExit'] = {'backKeyEventCount': attempts,
                        'productNoLongerResumed': True, 'forcedStopOrTerminationInvoked': False}
                except Exception as error:
                    cleanup_errors.append({'operation': 'normal-product-back-exit', 'errorType': type(error).__name__,
                        'failureCode': error.code if isinstance(error, GuardError) else 'normal-product-back-exit-failed'})
            if forward_created:
                try:
                    require(owned_forward(lease, socket_name), 'cleanup-own-forward-binding-differs')
                    lease.adb(['forward', '--remove', 'tcp:19330'], require_cdp=True)
                    require(not any(row[1] == 'tcp:19330' for row in forward_rows(lease)), 'cleanup-own-forward-still-present')
                    require(not any(row['port'] == 19330 for row in lease.tcp_rows()), 'cleanup-own-cdp-port-still-present')
                    result['ownForwardActuallyRemoved'] = True
                except Exception as error:
                    cleanup_errors.append({'operation': 'own-forward-remove', 'errorType': type(error).__name__,
                                           'failureCode': error.code if isinstance(error, GuardError) else 'own-forward-cleanup-failed'})
            if dex_owned:
                try:
                    raw, _ = lease.adb(['exec-out', 'cat', remote])
                    require(sha(raw) == DEX_SHA, 'cleanup-own-diagnostic-dex-sha-differs')
                    lease.adb(['shell', '-T', '/system/bin/rm', remote])
                    lease.adb(['shell', '-T', '/system/bin/test', '!', '-e', remote])
                    result['ownDiagnosticDexActuallyRemoved'] = True
                except Exception as error:
                    cleanup_errors.append({'operation': 'own-diagnostic-dex-remove', 'errorType': type(error).__name__,
                                           'failureCode': error.code if isinstance(error, GuardError) else 'own-diagnostic-dex-cleanup-failed'})
            try:
                lease.guard()
                result['currentNativeHostIdentityVerifiedAfterCleanup'] = True
            except Exception as error:
                cleanup_errors.append({'operation': 'final-current-host-identity', 'errorType': type(error).__name__,
                                       'failureCode': error.code if isinstance(error, GuardError) else 'final-native-host-identity-failed'})
            try:
                lease.close()
                result['nativeHostQueryHandlesClosed'] = True
            except Exception as error:
                # Lease raises a close error only after all original native
                # client HANDLEs have actually reached their terminal state.
                cleanup_errors.append({'operation': 'held-native-query-handle-close', 'errorType': type(error).__name__,
                                       'failureCode': error.code if isinstance(error, GuardError) else 'native-query-handle-close-failed'})
                result['nativeHostQueryHandlesClosed'] = False
            result['allNativeAdbCreatorHandlesActualTerminal'] = all(client.returncode is not None for client in lease.clients)
            result['nativeAdbCreatorActualExitCodes'] = [client.returncode for client in lease.clients]
        result['activityOpenedNormally'] = app_opened
        result['appNormalExitLeftToRootOwnedAvdManager'] = False
        result['cleanupErrors'] = cleanup_errors
        result['cleanupComplete'] = not cleanup_errors and (not forward_created or result.get('ownForwardActuallyRemoved') is True)
        result['originalXmlComparedAcrossExplicitObserverOnly'] = result.get('currentEncryptedXmlBytesExactlyUnchanged') is True
        if failure is not None:
            result['failure'] = failure
        accepted = failure is None and not cleanup_errors and result.get('explicitObserverNativeFiveAndOriginal7337Passed') is True
        result['currentWindowsFiveWireAccepted'] = accepted
        result['status'] = 'current-Windows-original-five-native-and-original7337-wire-passed' if accepted else 'current-Windows-original-five-wire-failed-with-evidence-preserved'
        result['observedAtUtc'] = stamp()
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--bindings', required=True)
    parser.add_argument('--bindings-sha256', required=True)
    args = parser.parse_args()
    try:
        b, identity = load_binding(args.bindings, args.bindings_sha256)
        result = run(b, identity)
        result['bindingSha256'] = args.bindings_sha256
        result['sourceSha256'] = sha(Path(__file__).read_bytes())
        write_safe(b['outputFile'], result)
        print(json.dumps(result))
        return 0 if result['currentWindowsFiveWireAccepted'] else 1
    except Exception as error:
        print(json.dumps({'schemaVersion': 1, 'status': 'current-Windows-five-wire-before-safe-publication-failed',
                          'errorType': type(error).__name__, 'failureCode': error.code if isinstance(error, GuardError) else 'native-wire-publication-or-binding-failed',
                          'currentWindowsFiveWireAccepted': False, 'rawErrorOrPrivateValuesIncluded': False}))
        return 1


if __name__ == '__main__':
    sys.exit(main())
