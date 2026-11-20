# Public review template; native execution is disabled. Original source digest is in provenance-safe.json.
if __name__ == "__main__":
    raise RuntimeError("public-review-template-native-execution-disabled")
import sys,types,json,hashlib,time,subprocess
from pathlib import Path
from datetime import datetime,timezone
ROOT=Path(__file__).parent
src=ROOT/'src/native_five_current_wire.py';raw=src.read_bytes()
assert hashlib.sha256(raw).hexdigest()=='52a2f0aa4431f2f3211cd823206182cb085c1bd5c2adb6be33755a5edf0d099e'
m=types.ModuleType('owned_acceptance');m.__file__=str(src);exec(compile(raw,str(src),'exec'),m.__dict__)
result={'schemaVersion':1,'status':'current-full-product-storage-tail-not-yet-observed','credentialGets':0,'remoteHttpCalls':0,'userValuesIncluded':False,'storageWrites':0,'forwardsCreated':0}
lease=cdp=None;forward=False;opened=False;socket_name=None
snapshot_key='__orchesterAcceptanceStorageTail20261119'
try:
 p=subprocess.run([m.Lease.__module__ and r'<USERPROFILE>\AppData\Roaming\uv\python\cpython-3.14.9-windows-x86_64-none\python.exe','-I',str(ROOT/'refresh-current-identity.py')],capture_output=True)
 m.require(p.returncode==0,'independent-native-tail-identity-refresh-failed')
 identity=json.loads((ROOT/'identity-current-safe.json').read_text());lease=m.Lease(identity)
 m.require(not any(row['port']==19330 for row in lease.tcp_rows()),'tail-cdp-port-already-used')
 _,rc=lease.adb(['shell','-T','am','start','-W','-n',m.PACKAGE+'/.MainActivity']);opened=True
 app,ready=m.wait_started_app_identity(lease,10235);result['normalActivityReadiness']=ready
 socket_name='webview_devtools_remote_'+str(app['pid'])
 m.require(not any(row[1]=='tcp:19330' for row in m.forward_rows(lease)),'tail-forward-already-used')
 forward=True;lease.adb(['forward','--no-rebind','tcp:19330','localabstract:'+socket_name]);m.require(m.owned_forward(lease,socket_name),'tail-forward-binding-differs');result['forwardsCreated']=1
 deadline=time.monotonic()+30
 while cdp is None:
  try:cdp=m.CDP(19330)
  except Exception:
   m.require(time.monotonic()<deadline,'tail-cdp-readiness-failed');time.sleep(.25)
 time.sleep(2)
 result['phase']='memory-only-before-storage-digests'
 before=r"""(async()=>{const key='__orchesterAcceptanceStorageTail20261119';if(Object.hasOwn(globalThis,key))throw Error('own-memory-key-used');const entries=Object.keys(localStorage).sort().map(k=>[k,localStorage.getItem(k)]);const serialized=JSON.stringify(entries);const devices=localStorage.getItem('orchester.devices');const compact=JSON.stringify(JSON.parse(devices||'[]'));const theme=localStorage.getItem('orchester.theme');globalThis[key]={entries,serialized,devices,theme};const digest=async v=>[...new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(v)))].map(x=>x.toString(16).padStart(2,'0')).join('');return {localStorageEntryCount:entries.length,fullLocalStorageSortedPairsSha256:await digest(serialized),savedDeviceRecordCount:JSON.parse(compact).length,savedDevicesRawUtf8Sha256:await digest(devices||''),savedDevicesCompactUtf8Sha256:await digest(compact),historicalPinnedFiveSavedDeviceRecordsMatched:await digest(compact)==='4bc316d7b725d29b35d0aec0b794afa155beac989787fd31565d334da2821fe6',themePresent:theme!==null,themeValueSha256:await digest(JSON.stringify(theme)),userValuesReturned:false};})()"""
 lease.guard(require_cdp=True);result['before']=cdp.eval(before)
 result['storageOnlyTailNoProductMutation']=True
 time.sleep(1)
 result['phase']='memory-only-after-storage-digests'
 after=r"""(async()=>{const entries=Object.keys(localStorage).sort().map(k=>[k,localStorage.getItem(k)]),serialized=JSON.stringify(entries),devices=localStorage.getItem('orchester.devices'),theme=localStorage.getItem('orchester.theme');const digest=async v=>[...new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(v)))].map(x=>x.toString(16).padStart(2,'0')).join('');const full=await digest(serialized),device=await digest(devices||''),themeHash=await digest(JSON.stringify(theme));return {fullLocalStorageExactlyUnchanged:full==='__FULL__',savedDeviceStorageRawExactlyUnchanged:device==='__DEVICE__',themeValueExactlyUnchanged:themeHash==='__THEME__',localStorageEntryCount:entries.length,fullLocalStorageSortedPairsSha256:full,savedDevicesRawUtf8Sha256:device,themeValueSha256:themeHash,snapshotsComputedInMemoryOnly:true,userValuesReturned:false};})()"""
 after=after.replace('__FULL__',result['before']['fullLocalStorageSortedPairsSha256']).replace('__DEVICE__',result['before']['savedDevicesRawUtf8Sha256']).replace('__THEME__',result['before']['themeValueSha256'])
 lease.guard(require_cdp=True);result['after']=cdp.eval(after)
 m.require(result['after']['fullLocalStorageExactlyUnchanged'] and result['after']['savedDeviceStorageRawExactlyUnchanged'] and result['after']['themeValueExactlyUnchanged'],'current-complete-product-storage-changed')
 xml,rc=lease.adb(['exec-out','run-as',m.PACKAGE,'cat','shared_prefs/remote_credentials.xml']);m.require(m.sha(xml)==m.BASELINE_XML_SHA,'tail-original-encrypted-digest-differs');result['originalEncryptedXmlSha256']=m.sha(xml)
 result['fullProductStorageStableSincePostMainUiSnapshot']=result['before']['fullLocalStorageSortedPairsSha256']=='aebbb8ab42d7bceea9aff31ef4a2e1ce6a46b301696573188646af9cd51b0855'
 result['status']='current-full-product-storage-and-historical-five-records-tail-passed'
except Exception as e:
 result.update(status='current-full-product-storage-tail-failed',errorType=type(e).__name__,failureCode=getattr(e,'code','safe-storage-tail-observation-failed'))
finally:
 errors=[]
 if cdp:
  try:cdp.eval("(()=>{delete globalThis['__orchesterAcceptanceStorageTail20261119'];return {ownMemorySnapshotCleared:true};})()")
  except Exception:errors.append('own-memory-snapshot-clear-failed')
  cdp.close()
 if lease:
  if opened:
   for _ in range(6):
    top,_=lease.adb(['shell','-T','dumpsys','activity','activities']);resumed=any(m.PACKAGE in line for line in top.decode(errors='replace').splitlines() if 'topResumedActivity' in line or 'mResumedActivity' in line)
    if not resumed:break
    lease.adb(['shell','-T','input','keyevent','4']);time.sleep(.3)
   top,_=lease.adb(['shell','-T','dumpsys','activity','activities']);resumed=any(m.PACKAGE in line for line in top.decode(errors='replace').splitlines() if 'topResumedActivity' in line or 'mResumedActivity' in line)
   result['normalBackExitObserved']=not resumed
  if forward:
   m.require(m.owned_forward(lease,socket_name),'tail-cleanup-forward-binding-differs');lease.adb(['forward','--remove','tcp:19330'],require_cdp=True);m.require(not any(row['port']==19330 for row in lease.tcp_rows()),'tail-cdp-port-not-released');result['ownForwardActuallyRemoved']=True
  lease.guard();lease.close();result['allNativeAdbCreatorHandlesTerminal']=all(p.returncode is not None for p in lease.clients);result['nativeAdbCreatorActualExitCodes']=[p.returncode for p in lease.clients];result['ownNativeQueryHandlesClosed']=True
 result['cleanupErrors']=errors;result['observedAtUtc']=datetime.now(timezone.utc).isoformat();result['sourceSha256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
 (ROOT/'full-product-storage-tail-safe.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
sys.exit(0 if result['status']=='current-full-product-storage-and-historical-five-records-tail-passed' and not errors else 1)
