# Public review template; native execution is disabled. Original source digest is in provenance-safe.json.
if __name__ == "__main__":
    raise RuntimeError("public-review-template-native-execution-disabled")
import sys,types,json,hashlib,subprocess,os
from pathlib import Path
from datetime import datetime,timezone
SRC=Path(__file__).parent/'src'/'native_wire_common.py'
raw=SRC.read_bytes()
assert hashlib.sha256(raw).hexdigest()=='e5c7c058c9d6644e4a9a11296ac4b1f04ccd8a07f534c6059f814dc6ea1e79b7'
m=types.ModuleType('native_wire_common');m.__file__=str(SRC)
exec(compile(raw,str(SRC),'exec'),m.__dict__)
BASE=Path(r'<USERPROFILE>\AppData\Local\Orchester-build')
RUNTIME=BASE/'android5-current-wire-runtime-20261113'/'runtime-0b12b11bcb5c477aba71c94aab73367b'
OLD=BASE/'a5w8-fa67a5cf'/'identity-self-fa67a5cfd9844bea819ba87667a18d17.json'
BIND=BASE/'a5di-0b12b11b'/'manager-bindings.json'
OUT=Path(__file__).parent/'identity-current-safe.json'
lease=None
result={'schemaVersion':1,'status':'independent-current-native-identity-not-yet-verified','privateValuesIncluded':False,'processSignals':0,'adbCommands':0}
try:
 expected=m.pinned_json(OLD,'8fa179c06d4fbdea51eea9e333caa37ea8974166eee919abfa6fda17f9ad1c4b')
 binding=m.pinned_json(BIND,'a0994a5e9fe37be0f14dc590ce592c851c6d1203aa095541884848cb9f878787')
 current=json.loads((RUNTIME/'manager-current-safe.json').read_text(encoding='utf-8-sig'))
 m.fresh_receipt(current,10)
 m.require(current['manager']==expected['manager'] and current['namespaceId']==expected['namespaceId'] and current['bindingSha256']==expected['bindingSha256'],'current-manager-binding-differs')
 m.require(current['status']=='owned-handles-retained-awaiting-normal-terminal' and current['activeCreatorHandleCount']==2 and current['originalCreatorExitCodes']['server'] is None and current['originalCreatorExitCodes']['head'] is None and current['normalQuitRequested'] is False,'current-manager-holder-proof-differs')
 lease=m.Lease(expected)
 # Only compare expected command/path and parent; never serialize command lines.
 command=r"$ErrorActionPreference='Stop';$man=Get-CimInstance Win32_Process -Filter 'ProcessId=25436';$q=Get-CimInstance Win32_Process -Filter 'ProcessId=33048';$h=Get-CimInstance Win32_Process -Filter 'ProcessId=4080';$w=Get-CimInstance Win32_Process -Filter 'ProcessId=5340';[ordered]@{managerBindingInCommand=($man.CommandLine.Contains('a5di-0b12b11b') -and $man.CommandLine.Contains('a0994a5e9fe37be0f14dc590ce592c851c6d1203aa095541884848cb9f878787'));qemuParentMatches=($q.ParentProcessId -eq 4080);headParentMatches=($h.ParentProcessId -eq 25436);headUsesOriginalAvd=($h.CommandLine.Contains('Medium_Phone_API_37.0') -and $h.CommandLine.Contains('5572'));managerParentPresent=($man.ParentProcessId -gt 0);workerOriginalPathMatches=($w.ExecutablePath -ceq 'C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe')}|ConvertTo-Json -Compress"
 p=subprocess.Popen([expected['worker']['image'],'-NoProfile','-Command',command],stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
 lease.clients.append(p)
 out,err=p.communicate();m.require(p.returncode==0,'native-parent-read-failed')
 parents=json.loads(out.decode('utf-8-sig'));result['independentParentAndCommandComparisons']=parents;m.require(all(v is True for v in parents.values()),'current-native-parent-or-command-differs')
 lease.guard()
 current_after=json.loads((RUNTIME/'manager-current-safe.json').read_text(encoding='utf-8-sig'));m.fresh_receipt(current_after,10)
 m.require(current_after['manager']==current['manager'] and current_after['activeCreatorHandleCount']==2 and current_after['normalQuitRequested'] is False,'manager-holder-changed-during-independent-identity')
 result=dict(expected)
 result.update(observedAtUtc=datetime.now(timezone.utc).isoformat().replace('+00:00','Z'),independentlyVerifiedCurrentNativeProcesses=True,independentObserver=lease.identity['observer'],independentParentAndCommandComparisons=parents,managerCurrentHolderProofObservedAtUtc=current_after['observedAtUtc'],managerPreservedFailureCode=current_after['failureCode'],originalCreatorHandleClaimsSource='current-original-manager-retained-loop-receipt',independentObserverOwnQueryHandlesHeldThroughObservation=True,independentObserverSignals=0,independentObserverAdbCommands=0)
except Exception as e:
 result.update(status='independent-current-native-identity-failed',errorType=type(e).__name__,failureCode=getattr(e,'code','safe-native-observation-failed'))
finally:
 if lease:lease.close()
 result['independentObserverOwnQueryHandlesClosed']=bool(lease)
 OUT.write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
 print(json.dumps({k:result.get(k) for k in ['status','observedAtUtc','managerPreservedFailureCode','failureCode','independentObserverOwnQueryHandlesClosed','independentParentAndCommandComparisons']}))
sys.exit(0 if result.get('independentlyVerifiedCurrentNativeProcesses') else 1)
