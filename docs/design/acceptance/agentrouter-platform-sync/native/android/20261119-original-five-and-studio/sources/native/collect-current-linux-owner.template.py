# Public review template; native execution is disabled. Original source digest is in provenance-safe.json.
if __name__ == "__main__":
    raise RuntimeError("public-review-template-native-execution-disabled")
from pathlib import Path
import hashlib,json,datetime,subprocess,os
root=Path(__file__).resolve().parent;receipt=json.loads((root/'preparation-safe.json').read_text());unit='orchester-default-remote-7337-20261112-post-reboot.service'
def proc(pid):
 s=Path('/proc',str(pid),'stat').read_text().rsplit(')',1)[1].split();return {'pid':pid,'ppid':int(s[1]),'startTicks':int(s[19]),'cgroup':Path('/proc',str(pid),'cgroup').read_text().strip()}
def hashfile(p):
 h=hashlib.sha256()
 with open(p,'rb') as f:
  while b:=f.read(1024*1024):h.update(b)
 return h.hexdigest()
boot=Path('/proc/sys/kernel/random/boot_id').read_text().strip();assert boot=='04a4390b-1b73-42f2-b3ae-906308618b42'
a=proc(266);b=proc(293);c=proc(427);assert a['startTicks']==277 and b['ppid']==266 and b['startTicks']==283 and c['ppid']==293 and c['startTicks']==342 and c['cgroup'].endswith(unit)
elf=hashfile('/proc/427/exe');assert elf=='d983bceea7c0797c995c7ffb038eaa984323ee1106293da18433f2a3f22d6649'
u=subprocess.run(['systemctl','--user','show',unit,'-p','MainPID','-p','ActiveState','-p','SubState','-p','UnitFileState'],check=True,capture_output=True,text=True);props=dict(x.split('=',1) for x in u.stdout.splitlines());assert props=={'MainPID':'293','ActiveState':'active','SubState':'running','UnitFileState':'enabled'}
inodes=set()
for f in Path('/proc/427/fd').iterdir():
 try:s=os.readlink(f)
 except FileNotFoundError:continue
 if s.startswith('socket:['):inodes.add(s[8:-1])
listeners=[]
for table in ['tcp','tcp6']:
 for line in Path('/proc/net',table).read_text().splitlines()[1:]:
  parts=line.split()
  if parts[3]=='0A' and int(parts[1].split(':')[1],16)==7337:listeners.append((table,parts[9],parts[7]))
assert len(listeners)==1 and listeners[0][1] in inodes and listeners[0][2]=='1000'
assert proc(427)==c and Path('/proc/sys/kernel/random/boot_id').read_text().strip()==boot
result={'schemaVersion':1,'status':'current-managed-linux7337-owner-verified','sourceRevision':'71def643cb08b208cc79687f79255c504eb14dc6','observedAtUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'targetHost':'Linux','port':7337,'currentListeningSocketOwned':True,'managedUnitActive':True,'listenerPid':427,'listenerStartTicks':342,'listenerElfSha256':elf,'controllerPid':293,'controllerStartTicks':283,'systemdManagerPid':266,'kernelBootId':boot,'unitName':unit,'listenerSocketInode':int(listeners[0][1]),'pidBirthCgroupAndBootStable':True,'serviceActions':0,'processSignals':0,'environmentDatabaseOrAuthenticationValuesRead':False}
name='linux-owner-'+receipt['attemptId']+'-safe.json';out=root/name
with out.open('x') as f:json.dump(result,f,indent=2);f.write('\n')
out.chmod(0o600);winroot=Path('/mnt/c')/receipt['windowsRoot'][3:].replace('\\','/');(winroot/name).write_bytes(out.read_bytes())
print(json.dumps({'freshLinuxOwnerFileSha256':hashfile(out),'listenerPid':427,'listenerStartTicks':342,'kernelBootId':boot,'rootCollectorHttpOrCredentialCalls':0}))
