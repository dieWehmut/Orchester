import hashlib
import errno
import fcntl
import struct
import stat
import secrets
import json
import os
from pathlib import Path
import queue
import select
import shutil
import signal
import socket
import sqlite3
import subprocess
import sys
import threading
import time
from urllib.error import HTTPError, URLError
from urllib.request import build_opener, ProxyHandler, Request

ROOT = Path('/home/han/project/Orchester')
STATE = Path('/home/han/.local/state/orchester-remote')
OUT = Path('/home/han/.local/state/orchester-delivery/20261113/linux-service-recovery/persistent-controller')
BINARY = ROOT / 'target/debug/orchester'
REVISION = '71def643cb08b208cc79687f79255c504eb14dc6'
EXPECTED_SHA = 'd983bceea7c0797c995c7ffb038eaa984323ee1106293da18433f2a3f22d6649'
GATE = Path('/home/han/.local/state/orchester-delivery/20261111/root-rust-gates/summary.json')
HTTP = build_opener(ProxyHandler({}))
REPORT = OUT / 'final-safe.json'

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def write_report(data):
    temporary = REPORT.with_suffix('.tmp')
    temporary.write_text(json.dumps(data, indent=2) + '\n')
    temporary.chmod(0o600)
    temporary.replace(REPORT)

def identity(pid):
    proc = Path('/proc') / str(pid)
    fields = (proc / 'stat').read_text().split(') ', 1)[1].split()
    return int(fields[19]), os.readlink(proc / 'exe'), digest(proc / 'exe')

def open_port():
    with socket.socket() as connection:
        connection.settimeout(0.2)
        return connection.connect_ex(('127.0.0.1', 7337)) == 0

def snapshot():
    # Retain all original private rows only in this process for exact comparison.
    with sqlite3.connect((STATE / 'state/remote.db').as_uri() + '?mode=ro', uri=True) as db:
        runs = db.execute('SELECT workspace,id,value FROM remote_runs ORDER BY workspace,id').fetchall()
        state = db.execute('SELECT key,value FROM remote_state ORDER BY key').fetchall()
    parsed = {key: json.loads(value) for key, value in state}
    counts = {}
    for _, _, value in runs:
        name = json.loads(value)['summary']['state']
        counts[name] = counts.get(name, 0) + 1
    return runs, state, parsed, counts

def backup_state():
    directory = OUT / ('private-backup-' + time.strftime('%Y%m%dT%H%M%S', time.gmtime()))
    assert not directory.exists()
    directory.mkdir(mode=0o700)
    records = []
    for source in sorted(STATE.rglob('*')):
        if source.is_symlink():
            raise RuntimeError('Unexpected private state symlink')
        if not source.is_file():
            continue
        relative = source.relative_to(STATE)
        target = directory / relative
        target.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        before = digest(source)
        if source.suffix == '.db':
            with sqlite3.connect(source.as_uri() + '?mode=ro', uri=True) as src:
                with sqlite3.connect(target) as dst:
                    src.backup(dst)
        else:
            shutil.copy2(source, target)
        target.chmod(0o600)
        assert before == digest(source)
        records.append({'path': relative.as_posix(), 'sourceBeforeAfterExact': True,
                        'backupSha256': digest(target), 'backupBytes': target.stat().st_size})
    with sqlite3.connect((directory / 'state/remote.db').as_uri() + '?mode=ro', uri=True) as db:
        runs = db.execute('SELECT workspace,id,value FROM remote_runs ORDER BY workspace,id').fetchall()
        state = db.execute('SELECT key,value FROM remote_state ORDER BY key').fetchall()
    return directory, records, runs, state

def stop_owned(child, pidfd):
    if child.poll() is None:
        child.stdin.write(b'quit\n')
        child.stdin.flush()
    while child.poll() is None:
        try:
            child.wait(timeout=15)
        except subprocess.TimeoutExpired:
            # Keep the same ownership and normal quit pending; never kill another process.
            continue


def main():
    os.umask(0o077)
    OUT.mkdir(parents=True, exist_ok=True, mode=0o700)
    OUT.chmod(0o700)
    gates = json.loads(GATE.read_text())
    assert gates['status'] == 'passed' and gates['sourceInputsStable'] is True
    assert gates['revision'] == gates['headAfter'] == REVISION
    assert [g['phase'] for g in gates['gates']] == ['rust-tests', 'rust-fmt', 'rust-clippy', 'linux-native-build']
    assert all(type(g['exitCode']) is int and g['exitCode'] == 0 and g['processExited'] is True for g in gates['gates'])
    assert gates['binary']['sha256'] == EXPECTED_SHA == digest(BINARY)
    current_head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    assert not subprocess.check_output(['git', 'diff', '--', 'Cargo.toml', 'Cargo.lock', '.cargo', 'kisten'], cwd=ROOT)
    frozen = json.loads((GATE.parent / 'source-after.json').read_text())
    for item in frozen['files']:
        assert digest(ROOT / item['path']) == item['sha256']
    frontend_path = ROOT / 'docs/design/acceptance/agentrouter-platform-sync/tests/frontend/final21-builds-independent-review.json'
    frontend = json.loads(frontend_path.read_text())
    assert frontend['sourceRevision'] == REVISION and frontend['sourceStable'] is True
    assert frontend['sourceInputsEqualFinal71'] is True
    mobile = next(build for build in frontend['builds'] if build['name'] == 'mobile')
    assert mobile['actualExitCode'] == 0 and mobile['processExited'] is True
    expected_files = mobile['artifactFiles']
    assert len(expected_files) == 17
    actual_files = {p.relative_to(ROOT / 'apps/mobile/dist').as_posix() for p in (ROOT / 'apps/mobile/dist').rglob('*') if p.is_file()}
    assert actual_files == set(expected_files)
    for relative, record in expected_files.items():
        assert digest(ROOT / 'apps/mobile/dist' / relative) == record['sha256']
    assert not open_port(), 'Another current listener already owns 7337'
    before_runs, before_state, before_parsed, before_counts = snapshot()
    assert len(before_runs) >= 5
    old_receipt = json.loads(Path('/home/han/.local/state/orchester-delivery/20261111/linux-restore/final-safe.json').read_text())
    with sqlite3.connect((Path(old_receipt['privateBackupDirectory']) / 'state/remote.db').as_uri() + '?mode=ro', uri=True) as old_db:
        old_runs = old_db.execute('SELECT workspace,id,value FROM remote_runs ORDER BY workspace,id').fetchall()
        old_state = old_db.execute('SELECT key,value FROM remote_state ORDER BY key').fetchall()
    current_runs_map = {(w,i):v for w,i,v in before_runs}
    old_runs_retained = all(current_runs_map.get((w,i)) == v for w,i,v in old_runs)
    old_parsed = {k:json.loads(v) for k,v in old_state}
    old_semantic_equal = old_parsed == before_parsed
    assert old_runs_retained
    host_tokens = before_parsed['auth:host']['tokens']
    legacy_tokens = before_parsed['auth:' + str(ROOT)]['tokens']
    assert len(host_tokens) >= 3 and len(legacy_tokens) >= 2
    assert set(legacy_tokens).issubset(host_tokens)
    assert all(value['info']['expires_at'] > time.time() for value in host_tokens.values())
    expected_devices = sorted((value['info'] for value in host_tokens.values()), key=lambda d: d['device_id'])
    projects = before_parsed['projects']
    assert all(Path(p['path']).is_dir() and Path(p['path']).resolve() == Path(p['path']) and Path(p['path']).is_relative_to(ROOT) for p in projects.values())
    backup_dir, backup_records, backup_runs, backup_rows = backup_state()
    assert backup_runs == before_runs and backup_rows == before_state
    report = {'status': 'prepared', 'sourceRevision': REVISION, 'binarySha256': EXPECTED_SHA,
              'gateSummarySha256': digest(GATE), 'rustSourceInputsChecked': len(frozen['files']),
              'mobileReceiptSha256': digest(frontend_path), 'privateBackupDirectory': str(backup_dir),
              'backupFiles': backup_records, 'consistentBackupRowsExactlyEqual': True,
              'originalPort7337ObservedClosed': True, 'priorProcessHandlesInherited': False,
              'currentEnvironmentInheritedExceptExplicitStateHome': True,
              'currentEnvironmentTransfer': 'current caller snapshot in task-owned regular 0600 private JSON; loaded at service startup; not prior-process environment',
              'currentEnvironmentInHeapOnly': False,
              'privateCurrentEnvironmentSnapshotUsed': True,
              'environmentSnapshotPubliclyArchived': False,
              'persistentUserUnitConfigured': True,
              'rebootAutostartActuallyTested': False,
              'staleOwnControlSocketRemoved': STALE_SOCKET_REMOVED,
              'oldSnapshotOriginalRunRowsRetained': old_runs_retained,
              'oldSnapshotStateJsonEqualCurrent': old_semantic_equal,
              'currentGitHead': current_head,
              'oldProcessExitCause': 'unknown',
              'namedUserService': UNIT,
              'controllerPid': os.getpid(),
              'controllerStartTicks': identity(os.getpid())[0],
              'controllerExecutableSha256': identity(os.getpid())[2],
              'controllerScriptSha256': digest(Path(__file__)),
              'controllerCgroup': Path('/proc/self/cgroup').read_text().strip(),
              'fixedPrivateControlSocket': str(CONTROL_PATH),
              'stdinControllerMode': 'named user service, StandardInput=null; same Popen pipe writer retained',
              'priorLiveArgvOrEnvironmentEqualityClaimed': False, 'originalRunCount': len(before_runs),
              'originalRunStates': before_counts, 'originalLiveHostDeviceCount': len(expected_devices),
              'originalPersistedAuthEntryCountAcrossScopes': len(host_tokens) + len(legacy_tokens),
              'originalDuplicateLegacyAuthEntryCount': len(set(host_tokens) & set(legacy_tokens)),
              'modelCalls': 0, 'existingCredentialValuesCopiedToSafeReport': False}
    write_report(report)
    environment = dict(BOOTSTRAP_ENV)
    environment['ORCHESTER_HOME'] = str(STATE)
    command = [str(BINARY), '--json', 'remote', '--bind', '0.0.0.0', '--port', '7337',
               '--workspace', str(ROOT), '--assets', str(ROOT / 'apps/mobile/dist'), '--no-pairing']
    child = None
    signal.signal(signal.SIGCHLD, signal.SIG_DFL)
    pidfd = None
    token = None
    own_device_id = None
    pairing_closed = False
    messages = queue.Queue(maxsize=64)
    management_messages_enabled = [True]
    own_device_name = "Orchester acceptance " + secrets.token_hex(12)

    def await_message(label):
        while True:
            try:
                return messages.get(timeout=15)
            except queue.Empty:
                if child.poll() is not None:
                    raise RuntimeError('Owned listener exited before management response')
                report.update(status='waiting-for-management-observation',
                              pendingObservation=label, ownedListenerVerifiedRunning=True)
                write_report(report)

    def admin(command):
        child.stdin.write((command + '\n').encode())
        child.stdin.flush()
        return await_message('local-admin-response')

    def request(path, method='GET', body=None):
        headers = {'Content-Type': 'application/json'}
        if token is not None:
            headers['Authorization'] = 'Bearer ' + token
        req = Request('http://127.0.0.1:7337/remote/v1' + path,
                      data=None if body is None else json.dumps(body).encode(),
                      method=method, headers=headers)
        while True:
            try:
                with HTTP.open(req, timeout=20) as response:
                    return response.status, response.read()
            except (TimeoutError, URLError) as error:
                timed_out = isinstance(error, TimeoutError) or isinstance(getattr(error, 'reason', None), TimeoutError)
                if not timed_out or method != 'GET' or child.poll() is not None:
                    raise
                # A read observation timeout does not stop or restart this listener.
                report.update(status='waiting-for-http-observation', pendingObservation=path,
                              ownedListenerVerifiedRunning=True)
                write_report(report)

    try:
        child = subprocess.Popen(command, cwd=ROOT, env=environment, stdin=subprocess.PIPE,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE, close_fds=True)
        try:
            def drain_stderr():
                while child.stderr.read(65536):
                    pass
            threading.Thread(target=drain_stderr, daemon=True).start()
            def drain():
                while True:
                    line=child.stdout.readline(1024*1024)
                    if not line:break
                    if not management_messages_enabled[0]:continue
                    try:
                        message=json.loads(line)
                        if message.get('event') not in {'listening','devices','pairing','pairing_closed','device_revoked','error'}:continue
                        messages.put_nowait(message)
                    except (ValueError,TypeError,queue.Full):
                        pass
            threading.Thread(target=drain, daemon=True).start()
            pidfd = os.pidfd_open(child.pid)
        except Exception:
            stop_owned(child, pidfd)
            raise
        startup = await_message('startup-listening')
        assert startup['event'] == 'listening' and startup['address'].endswith(':7337')
        assert startup.get('pairing_code') is None and open_port()
        ticks, executable, sha = identity(child.pid)
        assert executable == str(BINARY) and sha == EXPECTED_SHA
        report.update(pid=child.pid, startTicks=ticks, startupPairingDisabled=True)
        checks = []
        for relative, record in sorted(expected_files.items()):
            with HTTP.open('http://127.0.0.1:7337/' + relative, timeout=5) as response:
                payload = response.read()
                assert response.status == 200 and hashlib.sha256(payload).hexdigest() == record['sha256']
            checks.append({'path': relative, 'status': 200, 'sha256': record['sha256']})
        report['publicResources'] = checks
        for protected in ['/info', '/agents/status']:
            try:
                request(protected)
                raise RuntimeError('Unauthenticated route accepted')
            except HTTPError as error:
                assert error.code == 401
        report['unauthenticatedProtectedRoutesStatus'] = 401
        baseline_devices = admin('devices')
        assert baseline_devices['event'] == 'devices'
        assert sorted(baseline_devices['devices'], key=lambda d: d['device_id']) == expected_devices
        pairing = admin('pair')
        assert pairing['event'] == 'pairing'
        pairing_closed = False
        code, raw = request('/pair', 'POST', {'code': pairing['pairing_code'], 'device_name': own_device_name})
        assert code == 200
        paired = json.loads(raw)
        token = paired['token']
        own_device_id = paired['device_id']
        assert paired['host_id'] == before_parsed['host_id']
        authorized = []
        for path in ['/info', '/projects', '/agents', '/agents/status']:
            code, raw = request(path)
            assert code == 200
            value = json.loads(raw)
            if path == '/info':
                assert value['schema_version'] == 2 and value['host_id'] == before_parsed['host_id']
                assert value['workspace'] == str(ROOT)
            elif path == '/projects':
                assert sorted(value['projects'], key=lambda p: p['id']) == sorted(projects.values(), key=lambda p: p['id'])
            elif path == '/agents':
                assert isinstance(value['agents'], list)
                assert all(type(a['available']) is bool and type(a['supports_resume']) is bool for a in value['agents'])
            else:
                assert value['schema_version'] == 3
                expected = {'codex', 'claude', 'deepseek', 'opencode', 'gemini', 'antigravity', 'cursor', 'workbuddy', 'workbuddy_global', 'qwen', 'openclaw', 'cline', 'roo', 'kilo', 'hermes', 'pi', 'crush', 'claude_app', 'trae', 'zcode', 'craft'}
                catalog = {agent['provider']: agent for agent in value['agents'] if agent['provider'] in expected}
                assert set(catalog) == expected
                assert catalog['workbuddy']['display_name'] == 'WorkBuddy 国内版'
                assert catalog['workbuddy_global']['display_name'] == 'WorkBuddy AI 国际版'
                services = {'workbuddy', 'workbuddy_global', 'openclaw', 'claude_app', 'trae', 'zcode', 'craft'}
                for provider in services:
                    assert 'service_presence' in catalog[provider]['capabilities']
                for provider in ['cline', 'roo', 'kilo']:
                    assert 'shared_editor_presence_unknown' in catalog[provider]['capabilities']
                assert all(all(field not in agent for field in ['pid', 'args', 'exe']) for agent in value['agents'])
                report.update(canonicalProviderCount=len(catalog), canonicalProviderIds=sorted(expected),
                              serviceProviderCount=len(services), sharedEditorUnknownProviderCount=3,
                              workBuddyEditionNamesDistinct=True, snapshotSchemaVersion=3)
            authorized.append({'path': path, 'status': code, 'jsonContractVerified': True})
        code, _ = request('/device', 'DELETE')
        assert code == 204
        try:
            request('/info')
            raise RuntimeError('Revoked temporary token accepted')
        except HTTPError as error:
            assert error.code == 401
        token = None
        own_device_id = None
        assert admin('close')['event'] == 'pairing_closed'
        pairing_closed = True
        devices_after = admin('devices')
        actual_devices_map={d['device_id']:d for d in devices_after['devices']}
        assert all(actual_devices_map.get(d['device_id']) == d for d in expected_devices)
        after_runs, after_state, after_parsed, after_counts = snapshot()
        original_runs = {(workspace, run_id): data for workspace, run_id, data in before_runs}
        current_runs = {(workspace, run_id): data for workspace, run_id, data in after_runs}
        assert all(current_runs.get(key) == data for key, data in original_runs.items())
        def contains_original(current,original):
            if isinstance(original,dict):
                return isinstance(current,dict) and all(k in current and contains_original(current[k],v) for k,v in original.items())
            return current==original
        assert all(contains_original(after_parsed.get(key),value) for key,value in before_parsed.items())
        ticks_after, exe_after, sha_after = identity(child.pid)
        assert (ticks_after, exe_after, sha_after) == (ticks, executable, sha)
        report.update(status='deployed-and-verified', authorizedReadChecks=authorized,
                      temporaryOwnDeviceRemoved=True, revokedTemporaryTokenStatus=401,
                      pairingClosedAfterChecks=True, originalLiveHostDevicesExactlyRetained=True,
                      originalHostAndLegacyAuthRowsExactlyRetained=True,
                      originalProjectAndHostStateExactlyRetained=True, originalRunRowsExactlyRetained=True,
                      allRemoteStateJsonExactlyRetained=before_parsed == after_parsed,
                      allRemoteStateRawRowsExactlyRetained=before_state == after_state,
                      currentRunCount=len(after_runs), currentRunStates=after_counts,
                      controlledStdinRetained=True, listenerPort=7337, userProcessSignals=0,
                      verificationCompletedAt=time.time())
        write_report(report)
        print(json.dumps(report), flush=True)
    except Exception as error:
        if child is not None and child.poll() is None:
            # Independent own-device and pairing cleanup; a failed HTTP delete cannot skip local revoke.
            if token is not None:
                try:
                    code,_=request('/device','DELETE')
                    if code==204:token=None;own_device_id=None
                except Exception as cleanup_error:
                    report['ownHttpDeleteCleanupErrorType']=type(cleanup_error).__name__
            if own_device_id is None and token is None:
                try:
                    listed=admin('devices')
                    own_matches=[d for d in listed.get('devices',[]) if d.get('device_name')==own_device_name and d.get('device_id') not in {d['device_id'] for d in expected_devices}]
                    if len(own_matches)==1:own_device_id=own_matches[0]['device_id']
                except Exception as cleanup_error:
                    report['ownDeviceDiscoveryCleanupErrorType']=type(cleanup_error).__name__
            if own_device_id is not None:
                try:
                    revoked=admin('revoke '+own_device_id)
                    report['temporaryOwnDeviceRevokedLocally']=revoked.get('event')=='device_revoked' and revoked.get('revoked') is True
                except Exception as cleanup_error:
                    report['ownLocalRevokeCleanupErrorType']=type(cleanup_error).__name__
            if not pairing_closed:
                try:
                    report['pairingClosedDuringCleanup']=admin('close').get('event')=='pairing_closed'
                    pairing_closed=report['pairingClosedDuringCleanup']
                except Exception as cleanup_error:
                    report['pairingCloseCleanupErrorType']=type(cleanup_error).__name__
            stop_owned(child, pidfd)
        report.update(status='verification-failed-owned-listener-stopped', errorType=type(error).__name__,
                      ownedExitCode=None if child is None else child.poll(), listenerPortReleased=not open_port())
        write_report(report)
        print(json.dumps(report), flush=True)
        if pidfd is not None:
            os.close(pidfd)
        raise SystemExit(1)

    try:
        management_messages_enabled[0] = False
        while not messages.empty():
            try: messages.get_nowait()
            except queue.Empty:break
        global BOOTSTRAP_CONNECTION
        if BOOTSTRAP_CONNECTION is not None:
            try:
                send_control(BOOTSTRAP_CONNECTION, report)
            except (OSError, TimeoutError):
                pass
            BOOTSTRAP_CONNECTION.close()
            BOOTSTRAP_CONNECTION = None
        while child.poll() is None:
            if QUIT_REQUESTED:
                stop_owned(child, pidfd)
                break
            try:
                connection, _ = CONTROL_SERVER.accept()
            except socket.timeout:
                continue
            try:
                request = read_control(connection)
                action = request.get('action')
                if action == 'status':
                    birth, exe, sha = identity(child.pid)
                    assert (birth, exe, sha) == (ticks, executable, EXPECTED_SHA)
                    status = {'status':'live-owned', 'controllerPid':os.getpid(),
                              'controllerStartTicks':identity(os.getpid())[0],
                              'controllerExecutableSha256':identity(os.getpid())[2],
                              'controllerScriptSha256':digest(Path(__file__)),
                              'listenerPid':child.pid, 'listenerStartTicks':birth,
                              'listenerExecutableSha256':sha, 'listening7337':open_port(),
                              'retainedWriterFd':child.stdin.fileno(), 'pidfdHeld':pidfd is not None,
                              'namedUserService':UNIT, 'stateRoot':str(STATE),
                              'fixedPrivateControlSocket':str(CONTROL_PATH)}
                    send_control(connection,status)
                elif action == 'quit':
                    stop_owned(child,pidfd)
                    send_control(connection,{'status':'owned-normal-quit-complete',
                                             'listenerExitCode':child.returncode,
                                             'samePopenWriterUsed':True, 'userPidSignals':0})
                else:
                    send_control(connection,{'status':'unsupported-control-action'})
            except (OSError, TimeoutError, ValueError, AssertionError, RecursionError):
                # A client observation failure does not stop the default producer.
                pass
            finally:
                connection.close()
    finally:
        if child.poll() is None:
            stop_owned(child,pidfd)
    exit_report = {'status':'owned-listener-exited', 'listenerPid':child.pid,
                   'exitCode':child.returncode, 'causeOfOldInstanceExit':'unknown'}
    path = OUT/'owned-exit-safe.json'
    path.write_text(json.dumps(exit_report,indent=2)); path.chmod(0o600)
    os.close(pidfd)



UNIT = 'orchester-default-remote-7337-20261112-post-reboot.service'
CONTROL_PATH = OUT/'control.sock'
CONTROL_SERVER = None
BOOTSTRAP_ENV = None
BOOTSTRAP_CONNECTION = None
QUIT_REQUESTED = False
STALE_SOCKET_REMOVED = False
CURRENT_ENVIRONMENT_PATH = OUT/'environment-current-private.json'
MAX_REQUEST = 2*1024*1024

def recv_exact(connection,size):
    chunks=[]
    while size:
        part=connection.recv(size)
        if not part: raise ValueError('incomplete private control frame')
        chunks.append(part);size-=len(part)
    return b''.join(chunks)

def read_control(connection):
    connection.settimeout(3)
    pid,uid,gid=struct.unpack('3i',connection.getsockopt(socket.SOL_SOCKET,socket.SO_PEERCRED,12))
    if uid != os.getuid():raise ValueError('different control uid')
    length=struct.unpack('!I',recv_exact(connection,4))[0]
    if length<2 or length>MAX_REQUEST:raise ValueError('invalid private control size')
    value=json.loads(recv_exact(connection,length))
    if not isinstance(value,dict):raise ValueError('invalid private control request')
    return value

def send_control(connection,value):
    connection.settimeout(10)
    data=json.dumps(value).encode();connection.sendall(struct.pack('!I',len(data))+data)

def unit_state():
    p=subprocess.run(['systemctl','--user','show',UNIT,'--property=MainPID','--property=ControlGroup','--property=ActiveState','--property=SubState'],stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=10)
    return p.returncode,{line.split('=',1)[0]:line.split('=',1)[1] for line in p.stdout.decode().splitlines() if '=' in line}

def private_client(action,environment=None):
    code,unit=unit_state();pid=int(unit.get('MainPID','0'))
    if code or pid<=0:
        receipt=json.loads(REPORT.read_text())
        pid=receipt['controllerPid']
        unit={'controlMode':'independent-owned-controller-user-manager-unavailable','MainPID':str(pid)}
    birth,exe,sha=identity(pid)
    connection=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM);connection.settimeout(60)
    connection.connect(str(CONTROL_PATH))
    peer,uid,gid=struct.unpack('3i',connection.getsockopt(socket.SOL_SOCKET,socket.SO_PEERCRED,12))
    if peer!=pid or uid!=os.getuid():raise RuntimeError('unexpected private controller peer')
    if environment is None:
        receipt=json.loads(REPORT.read_text())
        if (birth,sha)!=(receipt['controllerStartTicks'],receipt['controllerExecutableSha256']):raise RuntimeError('controller birth or executable changed')
        if digest(Path(__file__))!=receipt['controllerScriptSha256']:raise RuntimeError('controller source changed')
    request={'action':action}
    if environment is not None:request['environment']=environment
    send_control(connection,request)
    connection.settimeout(180)
    length=struct.unpack('!I',recv_exact(connection,4))[0]
    if length>1024*1024:raise ValueError('invalid private response size')
    response=json.loads(recv_exact(connection,length));connection.close();return response,unit

def serve():
    global CONTROL_SERVER,BOOTSTRAP_CONNECTION,BOOTSTRAP_ENV,QUIT_REQUESTED,STALE_SOCKET_REMOVED
    os.umask(0o077);OUT.mkdir(parents=True,exist_ok=True,mode=0o700);OUT.chmod(0o700)
    lock=open(OUT/'supervisor.lock','a+b');os.chmod(OUT/'supervisor.lock',0o600)
    fcntl.flock(lock.fileno(),fcntl.LOCK_EX|fcntl.LOCK_NB)
    if CONTROL_PATH.exists() or CONTROL_PATH.is_symlink():
        existing = CONTROL_PATH.lstat()
        if not stat.S_ISSOCK(existing.st_mode) or existing.st_uid != os.getuid():
            raise RuntimeError('unrecognized existing private control path preserved')
        probe = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        probe.settimeout(1)
        try:
            result = probe.connect_ex(str(CONTROL_PATH))
        finally:
            probe.close()
        if result != errno.ECONNREFUSED or open_port():
            raise RuntimeError('existing live or uncertain control/listener preserved')
        current = CONTROL_PATH.lstat()
        if (current.st_dev, current.st_ino) != (existing.st_dev, existing.st_ino):
            raise RuntimeError('private control path changed during stale check')
        CONTROL_PATH.unlink()
        STALE_SOCKET_REMOVED = True
    server=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM);server.bind(str(CONTROL_PATH));CONTROL_PATH.chmod(0o600);inode=CONTROL_PATH.stat().st_ino
    CONTROL_SERVER=server;server.listen(8);server.settimeout(0.5)
    def on_quit(signum,frame):
        global QUIT_REQUESTED
        QUIT_REQUESTED=True
    signal.signal(signal.SIGTERM,on_quit)
    try:
        env_fd = os.open(CURRENT_ENVIRONMENT_PATH, os.O_RDONLY | os.O_NOFOLLOW)
        with os.fdopen(env_fd, 'r') as environment_file:
            metadata = os.fstat(environment_file.fileno())
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != os.getuid() or stat.S_IMODE(metadata.st_mode) != 0o600:
                raise RuntimeError('private current-environment snapshot metadata invalid')
            environment = json.load(environment_file)
        if not isinstance(environment,dict) or not all(isinstance(k,str) and isinstance(v,str) for k,v in environment.items()):
            raise ValueError('invalid private current-environment snapshot')
        BOOTSTRAP_ENV = environment
        main()
    except Exception as error:
        safe={'status':'controller-setup-failed','errorType':type(error).__name__}
        (OUT/'controller-failure-safe.json').write_text(json.dumps(safe));os.chmod(OUT/'controller-failure-safe.json',0o600)
        if BOOTSTRAP_CONNECTION is not None:
            try:send_control(BOOTSTRAP_CONNECTION,safe)
            except (OSError,TimeoutError):pass
        raise
    finally:
        server.close()
        try:
            if CONTROL_PATH.stat().st_ino==inode:CONTROL_PATH.unlink()
        except FileNotFoundError:pass
        lock.close()

def launch():
    os.umask(0o077)
    code,unit=unit_state()
    if unit.get('ActiveState') in ['active','activating','deactivating']:raise RuntimeError('named service already active, preserved')
    if open_port():raise RuntimeError('another current listener owns 7337, preserved')
    command=['systemctl','--user','start',UNIT]
    r=subprocess.run(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=15)
    launcher_child=None
    if r.returncode:
        # Preserve the unavailable manager; start only this new, guarded controller.
        launcher_child=subprocess.Popen([sys.executable,str(Path(__file__)),'serve'],
            stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,
            start_new_session=True,close_fds=True)
    def current_unit():
        if launcher_child is not None:
            return 0,{'MainPID':str(launcher_child.pid),
                      'ActiveState':'active' if launcher_child.poll() is None else 'failed',
                      'controlMode':'independent-owned-controller-user-manager-unavailable'}
        return unit_state()
    deadline=time.monotonic()+180
    while not REPORT.exists():
        code, unit = current_unit()
        if code or unit.get('ActiveState') not in ['active','activating']:
            raise RuntimeError('persistent service became terminal before verification')
        if time.monotonic()>deadline:raise RuntimeError('controller readiness observation timeout')
        time.sleep(0.1)
    while True:
        report=json.loads(REPORT.read_text())
        code, unit=current_unit()
        if code or unit.get('ActiveState') not in ['active','activating']:
            raise RuntimeError('persistent service became terminal before verification')
        if report.get('status')=='deployed-and-verified' and report.get('controllerPid')==int(unit.get('MainPID','0')):
            break
        if time.monotonic()>deadline:raise RuntimeError('verification observation timeout')
        time.sleep(0.2)
    response,unit=private_client('status')
    receipt={'bootstrapResponseStatus':report.get('status'),'unitMetadata':unit,
             'launcherExitedAfterVerification':True,'currentEnvironmentInHeapOnly':False,
             'privateCurrentEnvironmentSnapshotUsed':True,'persistentUserUnitConfigured':True,
             'rebootAutostartActuallyTested':False,
             'noGlobalUserManagerEnvironmentImport':True,'producerOwnedByNamedIndependentUserUnit':launcher_child is None,
             'userManagerStartFailedFallbackUsed':launcher_child is not None,
             'independentControllerDetachedAfterVerifiedStartup':launcher_child is not None,
             'runnerCgroup':Path('/proc/self/cgroup').read_text().strip(),
             'controllerCgroup':report.get('controllerCgroup')}
    p=OUT/'unit-launch-safe.json';p.write_text(json.dumps(receipt,indent=2));p.chmod(0o600)
    print(json.dumps({'status':report.get('status'),'listenerPid':response.get('listenerPid'),
                      'listenerStartTicks':response.get('listenerStartTicks'),'controllerPid':response.get('controllerPid'),
                      'controllerStartTicks':response.get('controllerStartTicks'),
                      'namedUserService':UNIT,'fixedPrivateControlSocket':str(CONTROL_PATH)}))
    if response.get('status')!='live-owned':raise SystemExit(1)

if __name__=='__main__':
    if len(sys.argv)!=2:raise SystemExit(2)
    if sys.argv[1]=='serve':serve()
    elif sys.argv[1]=='start':launch()
    elif sys.argv[1] in ['status','quit']:
        response,unit=private_client(sys.argv[1]);print(json.dumps({'control':response,'unitMetadata':unit}))
    else:raise SystemExit(2)
