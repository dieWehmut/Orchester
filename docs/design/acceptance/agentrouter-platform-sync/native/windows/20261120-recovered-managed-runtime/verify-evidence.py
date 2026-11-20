#!/usr/bin/env python3
import hashlib, json, re
from pathlib import Path
ROOT=Path(__file__).resolve().parent
def read(p): return json.loads((ROOT/p).read_text(encoding='utf-8-sig'))
def sha(p): return hashlib.sha256((ROOT/p).read_bytes()).hexdigest()
m=read('manifest-safe.json');listed={x['path'] for x in m['files']};actual={str(p.relative_to(ROOT)) for p in ROOT.rglob('*') if p.is_file()};assert actual-set(m['integrityExcludedPaths'])==listed
for x in m['files']:assert sha(x['path'])==x['sha256'] and (ROOT/x['path']).stat().st_size==x['sizeBytes']
pre=read('preflight/native-preflight-result-safe.json');assert pre['old81620And98280NativeAbsent'] and pre['successfulNativeListenerAndProcessQueries'] and pre['no7338ListenerByTwoNativeMethods'] and pre['wholeInstalledRemoteFrozenPackageExact'];assert pre['offlineStateRetentionResult']['originalRowCount']==138
f=read('observation/attached-observer-final-safe.json');assert f['status']=='recovered-bounded-original-state-managed-7338-resources-retention-and-real-cycles-verified';assert f['observedSeconds']>=60 and f['currentSamples']==88 and f['resourceCount']==17 and f['originalRowCount']==138 and f['currentUnexpectedDescendantObservationEvents']==0 and f['remoteAnonymousInfoStatus']==401
assert f['initialCapturedIdentityCount']==205 and f['maximumCapturedIdentityCount']==226 and f['peakHeldIdentityCount']==6 and f['finalHeldIdentityCount']==2 and f['prunedExitedDescendantCount']==224 and f['originalRootHandleNeverPruned'] and f['recentTerminalRecordLimit']==32
assert sha('sources/06-bounded-managed-7338.ps1')==f['managerSourceSha256'];assert sha('sources/OwnedRuntimeProcessesBounded.cs')==read('scope-safe.json')['helperSha256'];assert f['maximumCapturedIdentityCount']==f['finalHeldIdentityCount']+f['prunedExitedDescendantCount']
post=read('handoff/post-observer-native-probe-safe.json');assert post['managerPid']==49728 and post['remotePid']==39092 and post['anonymousInfoStatus']==401 and post['unexpectedDescendants']==0 and post['nativeListenerOwnerVerified'];assert post['managerSourceSha256']==f['managerSourceSha256'] and post['managerInputSha256']==f['managerInputSha256'];assert post['totalCapturedIdentityCount']==post['heldIdentityCount']+post['prunedExitedDescendantCount']
for name in ['attached-observer','post-observer-probe','safe-receipt-export']:assert read('execution/'+name+'-execution-safe.json')['actualExternalNativeExitCode']==0
ae=read('execution/attached-observer-execution-safe.json');assert ae['nativeScriptSourceSha256']==sha('sources/09-invoke-attached-observer.ps1');assert ae['bridgeSourceSha256']==sha('sources/native_bridge.py');assert ae['stdoutSha256']==sha('execution/attached-observer-stdout-safe.log');assert json.loads((ROOT/'execution/attached-observer-stdout-safe.log').read_text())==f
pe=read('execution/post-observer-probe-execution-safe.json');assert pe['nativeScriptSourceSha256']==sha('sources/06-current-recovered-readonly-probe.ps1');assert pe['stdoutSha256']==sha('execution/post-observer-probe-stdout-safe.log');assert json.loads((ROOT/'execution/post-observer-probe-stdout-safe.log').read_text())==post
assert sha('sources/09-attach-recovered-observer.ps1') in (ROOT/'sources/09-invoke-attached-observer.ps1').read_text()
resources=read('observation/static-http-resources-safe.json');assert sha('observation/static-http-resources-safe.json')==f['resourcesResultSha256'];assert resources['resourceCount']==17 and all(x['httpStatus']==200 for x in resources['resources']);assert resources['resources']==read('dependencies/frozen-17-resources-safe.json')['resources']
for link in read('dependencies/sibling-source-bindings-safe.json')['links']:assert sha(link['path'])==link['sha256']
assert sha('sources/02-offline-original-state-retention.mjs')=='62ca32147156ae79829c266721c2aec960cbad51ff52287982ca75c315eb1488'
assert read('scope-safe.json')['remoteSavedCredentialWireVerified'] is False
print(json.dumps({'status':'offline-recovered-managed-runtime-evidence-verified','filesChecked':len(listed),'nativeExternalExitCodes':[0,0,0],'resources':17,'originalRows':138,'observedSeconds':f['observedSeconds'],'samples':88,'nativeOperations':0},indent=2))
