# ARCHIVAL TEMPLATE: Linux username replaced. Not the exact executed source. Do not run during offline validation.
#!/usr/bin/env python3
import json,hashlib,struct,subprocess,os,time
from pathlib import Path
from datetime import datetime,timezone
from probe_drvfs9p import Client,s9
base=Path(__file__).resolve().parent
c=Client();out={'observedAtUtc':datetime.now(timezone.utc).isoformat(),'operation':'read exact public System32 cmd.exe through new non-elevated9P route, copy to ownedLinux evidence, then run builtin ver via normalWSLinterop','windowsFileWrites':0,'existingMountChanges':0,'processSignals':0,'nativeAttempts':[]}
try:
 c.s.connect((2,50002));v=c.rpc(100,struct.pack('<I',65536)+s9('9P2000.L'),65535)
 if v[6:]!=b'9P2000.L':raise ValueError('protocol changed')
 c.rpc(104,struct.pack('<II',1,0xffffffff)+s9('<linux-user>')+s9('drvfs;path=C:\\;uid=1000;gid=1000;symlinkroot=/mnt/')+struct.pack('<I',1000))
 names=['Windows','System32','cmd.exe'];w=c.rpc(110,struct.pack('<IIH',1,2,3)+b''.join(s9(n) for n in names))
 if struct.unpack_from('<H',w)[0]!=3:raise ValueError('incomplete walk')
 pre=c.rpc(24,struct.pack('<IQ',2,0x7ff));size=struct.unpack_from('<Q',pre,49)[0]
 if not 100000<size<2000000:raise ValueError('unexpected public cmd size')
 c.rpc(12,struct.pack('<II',2,0));buf=bytearray()
 while len(buf)<size:
  v=c.rpc(116,struct.pack('<IQI',2,len(buf),min(32768,size-len(buf))));n=struct.unpack_from('<I',v)[0]
  if n!=len(v)-4 or not n:raise ValueError('short public command read')
  buf.extend(v[4:])
 after=c.rpc(24,struct.pack('<IQ',2,0x7ff))
 if pre[8:21]!=after[8:21] or struct.unpack_from('<Q',after,49)[0]!=size or bytes(buf[:2])!=b'MZ':raise ValueError('public command metadata changed')
 target=base/'owned-native-cmd.exe';target.write_bytes(buf);target.chmod(0o700)
 out.update({'copiedPublicCommandBytes':len(buf),'copiedPublicCommandSha256':hashlib.sha256(buf).hexdigest(),'beforeAfterQidAndSizeSame':True})
 c.rpc(120,struct.pack('<I',2));c.rpc(120,struct.pack('<I',1))
finally:c.s.close()
for name in ['357_interop','628707_interop']:
 env=os.environ.copy();env['WSL_INTEROP']='/run/WSL/'+name
 before=time.monotonic();r=subprocess.Popen(['/init',str(target),'cmd.exe','/d','/c','ver'],executable='/init',cwd=str(base),env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
 try:
  stdout,stderr=r.communicate(timeout=10)
  out['nativeAttempts'].append({'socket':name,'actualExitCode':r.returncode,'seconds':time.monotonic()-before,'stdout':stdout.decode(errors='replace')[:700],'stderr':stderr.decode(errors='replace')[:700]})
 except subprocess.TimeoutExpired:
  out['nativeAttempts'].append({'socket':name,'observationTimedOut':True,'linuxInitPid':r.pid,'seconds':time.monotonic()-before});break
 if r.returncode==0:break
print(json.dumps(out,ensure_ascii=False,indent=2));(base/'public-cmd-native-probe-safe.json').write_text(json.dumps(out,ensure_ascii=False,indent=2)+'\n')
