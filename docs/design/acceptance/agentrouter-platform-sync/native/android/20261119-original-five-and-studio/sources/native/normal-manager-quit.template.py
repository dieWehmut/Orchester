# Public review template; native execution is disabled. Original source digest is in provenance-safe.json.
if __name__ == "__main__":
    raise RuntimeError("public-review-template-native-execution-disabled")
import sys,types,json,hashlib,time,subprocess,ctypes
from pathlib import Path
from datetime import datetime,timezone
ROOT=Path(__file__).parent
SOURCE=ROOT/'src/native_wire_common.py';raw=SOURCE.read_bytes();assert hashlib.sha256(raw).hexdigest()=='e5c7c058c9d6644e4a9a11296ac4b1f04ccd8a07f534c6059f814dc6ea1e79b7'
m=types.ModuleType('native_wire_common');m.__file__=str(SOURCE);exec(compile(raw,str(SOURCE),'exec'),m.__dict__)
RUNTIME=Path(r'<USERPROFILE>\AppData\Local\Orchester-build\android5-current-wire-runtime-20261113\runtime-0b12b11bcb5c477aba71c94aab73367b')
result={'schemaVersion':1,'status':'exact-owned-manager-normal-control-not-yet-submitted','processSignals':0,'forcedTerminations':0,'newManagersOrAvdsStarted':0,'rawPrivateValuesIncluded':False}
lease=None
try:
 p=subprocess.run([r'<USERPROFILE>\AppData\Roaming\uv\python\cpython-3.14.9-windows-x86_64-none\python.exe','-I',str(ROOT/'refresh-current-identity.py')],capture_output=True)
 m.require(p.returncode==0,'quit-current-independent-native-identity-failed')
 identity=json.loads((ROOT/'identity-current-safe.json').read_text());lease=m.Lease(identity)
 current=json.loads((RUNTIME/'manager-current-safe.json').read_text());m.fresh_receipt(current,10)
 m.require(current['manager']==identity['manager'] and current['normalQuitRequested'] is False and current['activeCreatorHandleCount']==2,'quit-current-manager-holder-binding-differs')
 lease.guard();m.require(not any(r['port']==19330 for r in lease.tcp_rows()),'quit-own-cdp-port-still-used')
 control={'schemaVersion':1,'action':'normal_quit','namespaceId':identity['namespaceId'],'bindingSha256':identity['bindingSha256'],'managerPid':identity['manager']['pid'],'managerCreatedExact':identity['manager']['createdExact']}
 with (RUNTIME/'control.json').open('x',encoding='utf-8') as f:json.dump(control,f);f.write('\n')
 result.update(control=control,normalControlSubmitted=True,originalManagerHolderProofObservedAtUtc=current['observedAtUtc'],preservedOriginalManagerFailureCode=current['failureCode'])
 deadline=time.monotonic()+180;last_report=time.monotonic()
 while True:
  terminal={r:lease.k.WaitForSingleObject(lease.handles[r],0)==0 for r in ('manager','server','head','qemu')}
  ports=[r for r in lease.tcp_rows() if r['port'] in (5041,5572,5573,19330)]
  if all(terminal.values()) and not ports:break
  if time.monotonic()-last_report>=20:
   print(json.dumps({'status':'waiting-for-original-manager-normal-native-terminal','terminalRoles':terminal,'remainingPrivateTcpRowCount':len(ports),'forcedTerminations':0}),flush=True);last_report=time.monotonic()
  m.require(time.monotonic()<deadline,'normal-manager-terminal-observation-bound-reached-no-forced-exit')
  time.sleep(.25)
 codes={}
 for role in ('manager','server','head','qemu'):
  code=ctypes.c_uint32();m.require(bool(lease.k.GetExitCodeProcess(lease.handles[role],ctypes.byref(code))),'original-manager-native-exit-unreadable');codes[role]=code.value
 original=json.loads((RUNTIME/'terminal-safe.json').read_text());m.require(original['namespaceId']==identity['namespaceId'] and original['bindingSha256']==identity['bindingSha256'] and original['manager']==identity['manager'],'original-manager-terminal-receipt-binding-differs')
 m.require(original['allCreatorHandlesClosedAfterActualTerminal'] is True and original['allCreatedNativeHandlesActuallyTerminal'] is True and original['allFourPrivatePortsReleased'] is True and original['normalQuitRequested'] is True and original['forcedTerminations']==0,'original-manager-normal-release-proof-differs')
 m.require(all(codes[r]==0 for r in ('server','head','qemu')),'original-owned-channel-native-child-nonzero-exit')
 result.update(status='original-owned-manager-normal-control-native-children-and-ports-released',nativeExitCodes=codes,originalManagerTerminalStatus=original['status'],originalManagerFailurePreserved=original['failureCode'],originalManagerTerminalReceiptSha256=hashlib.sha256((RUNTIME/'terminal-safe.json').read_bytes()).hexdigest(),allOriginalManagerCreatorHandlesActuallyClosed=True,allFourPrivatePortsReleased=True,allOriginalChannelChildExitCodesZero=True)
 (ROOT/'original-manager-terminal-safe.json').write_bytes((RUNTIME/'terminal-safe.json').read_bytes())
except Exception as e:
 result.update(status='owned-manager-normal-control-observation-incomplete',errorType=type(e).__name__,failureCode=getattr(e,'code','safe-owned-manager-normal-control-failed'))
finally:
 if lease:lease.close();result['ownNativeQueryHandlesClosed']=True
 result['observedAtUtc']=datetime.now(timezone.utc).isoformat();result['sourceSha256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest();(ROOT/'normal-manager-quit-safe.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
sys.exit(0 if result['status']=='original-owned-manager-normal-control-native-children-and-ports-released' else 1)
