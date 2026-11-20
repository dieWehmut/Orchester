# ARCHIVAL TEMPLATE: Linux username replaced. Not the exact executed source. Do not run during offline validation.
#!/usr/bin/env python3
import socket,struct,json,time,hashlib
from pathlib import Path
from datetime import datetime,timezone

def s9(t):
 b=t.encode();return struct.pack('<H',len(b))+b
class Client:
 def __init__(self):self.s=socket.socket(socket.AF_VSOCK,socket.SOCK_STREAM);self.s.settimeout(5);self.tag=0;self.frames=[]
 def recv(self,n):
  out=b''
  while len(out)<n:
   b=self.s.recv(n-len(out))
   if not b:raise EOFError('server closed')
   out+=b
  return out
 def rpc(self,typ,b,tag=None):
  if tag is None:self.tag+=1;tag=self.tag
  frame=struct.pack('<IBH',7+len(b),typ,tag)+b;self.s.sendall(frame)
  header=self.recv(7);length,rtagkind,rtag=struct.unpack('<IBH',header)
  if not 7<=length<=65536 or rtag!=tag:raise ValueError('invalid frame header')
  payload=self.recv(length-7);self.frames.append({'requestType':typ,'responseType':rtagkind,'tag':tag,'responseBytes':length,'responseSha256':hashlib.sha256(header+payload).hexdigest()})
  if rtagkind==7:raise OSError(struct.unpack('<I',payload)[0],'9P Rlerror')
  if rtagkind==107:raise OSError('9P Rerror: '+payload[2:].decode(errors='replace')[:100])
  if rtagkind!=typ+1:raise ValueError('unexpected response')
  return payload

def main():
 c=Client();out={'observedAtUtc':datetime.now(timezone.utc).isoformat(),'uid':1000,'port':50002,'hostCid':2,'adminServerUsed':False,'mountWrites':0,'windowsFileWrites':0,'processSignals':0,'status':'attempting'};beg=time.monotonic()
 try:
  c.s.connect((2,50002));out['freshVsockConnected']=True
  version=c.rpc(100,struct.pack('<I',65536)+s9('9P2000.L'),65535);out['version']=version[6:].decode();out['serverMsize']=struct.unpack_from('<I',version)[0]
  if out['version']!='9P2000.L':raise ValueError('unimplemented negotiated version')
  c.rpc(104,struct.pack('<II',1,0xffffffff)+s9('<linux-user>')+s9('drvfs;path=C:\\;uid=1000;gid=1000;symlinkroot=/mnt/')+struct.pack('<I',1000));out['readOnlyRootAttachSucceeded']=True
  names=['Windows','System32','cmd.exe']
  walked=c.rpc(110,struct.pack('<IIH',1,2,len(names))+b''.join(s9(x) for x in names));out['walkedPublicCommandComponents']=struct.unpack_from('<H',walked)[0]
  if out['walkedPublicCommandComponents']!=len(names):raise ValueError('partial public file walk')
  meta=c.rpc(24,struct.pack('<IQ',2,0x7ff));out['publicCmdMode']=struct.unpack_from('<I',meta,21)[0];out['publicCmdBytes']=struct.unpack_from('<Q',meta,49)[0]
  c.rpc(12,struct.pack('<II',2,0));head=c.rpc(116,struct.pack('<IQI',2,0,4096));n=struct.unpack_from('<I',head)[0];out['publicCmdInitialBytesRead']=n;out['publicCmdMZMagic']=head[4:6]==b'MZ';out['publicCmdHeaderSha256']=hashlib.sha256(head[4:]).hexdigest()
  c.rpc(120,struct.pack('<I',2));c.rpc(120,struct.pack('<I',1));out['status']='read-only-public-command-file-route-verified'
 except Exception as e:out['status']='failed';out['errorType']=type(e).__name__;out['errno']=getattr(e,'errno',None);out['error']=str(e)[:240]
 finally:c.s.close()
 out['seconds']=time.monotonic()-beg;out['protocolFrames']=c.frames
 print(json.dumps(out,indent=2));Path(__file__).with_name('fresh-drvfs9p-probe-safe.json').write_text(json.dumps(out,indent=2)+'\n')
if __name__=='__main__':main()
