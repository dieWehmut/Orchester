"""Prepared Windows-only manager; no import performs a native operation.

Invocation: python.exe -I manage-owned-windows-original-avd.py
  --bindings NEW_BINDINGS.json --bindings-sha256 LOWERCASE_SHA256
The future binding must name the current worker4 and a fresh actual preflight.
Once any child starts, ownership is retained until every actual HANDLE exits.
"""
import argparse
import ctypes
from ctypes import wintypes as w
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import sys
import time
import uuid

SOURCE_REVISION = '71def643cb08b208cc79687f79255c504eb14dc6'
SDK = r'C:\Users\han\AppData\Local\Android\Sdk'
AVD_HOME = r'C:\Users\han\.android\avd'
AVD_NAME = 'Medium_Phone_API_37.0'
OWN_BASE = r'C:\Users\han\AppData\Local\Orchester-build\android5-current-wire-runtime-20261113'
POWERSHELL = r'C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe'
PINS = {
 'adb': (SDK + r'\platform-tools\adb.exe', 'b4a6b455702684652cccf7b46258b29e653538904359a58fd4931cf3ef286b3f'),
 'head': (SDK + r'\emulator\emulator.exe', '06af3c5da3c2cef7266822fe653f54371fc746d78db6f4282e1b2338a70970c8'),
 'qemu': (SDK + r'\emulator\qemu\windows-x86_64\qemu-system-x86_64-headless.exe', '161d3bcef3d5a93698a77f292c899d131d3e92e335c7b5b2360bbc5ce364cf4c'),
 'python': (r'C:\Users\han\AppData\Roaming\uv\python\cpython-3.14.9-windows-x86_64-none\python.exe', '204d51f19707da17766bccecce2ceececadab350663a8190420d1377bac1150d'),
 'avdIni': (AVD_HOME + r'\Medium_Phone_API_37.0.ini', '47fe0b97b105655cb8c2c56af3432038820090ea307e0f564b68601ffc5d0846'),
 'avdConfig': (AVD_HOME + r'\Medium_Phone.avd\config.ini', '2c09c45e0c587d206264ed83ea246215fdd728ce30c035b0804f3b6f1899f4aa'),
 'powershell': (POWERSHELL, '60c9b29843624dd8af6b6a7b26147753be6aeef65098b9a503cc50850afe410f'),
}
OLD = (('manager',30436,'134389406416098295'),('adb',14648,'134389406419545296'),
       ('emulator',29496,'134389406419932540'),('qemu',11140,'134389406420584962'))
PORTS = (5039,5572,5573,19329)

class GuardError(Exception):
 def __init__(self, code, native_error=None):
  self.code, self.native_error = code, native_error
  super().__init__(code)

def require(condition, code):
 if not condition: raise GuardError(code)

def sha_file(path):
 h=hashlib.sha256()
 with open(path,'rb') as f:
  for block in iter(lambda:f.read(1048576),b''): h.update(block)
 return h.hexdigest()

def strict_json(raw):
 def pairs(items):
  result={}
  for key,value in items:
   require(key not in result,'duplicate-json-field');result[key]=value
  return result
 return json.loads(raw.decode('utf-8-sig'),object_pairs_hook=pairs)

def utc(): return datetime.now(timezone.utc).isoformat().replace('+00:00','Z')

class FT(ctypes.Structure): _fields_=[('low',w.DWORD),('high',w.DWORD)]
class SID_ATTR(ctypes.Structure): _fields_=[('sid',w.LPVOID),('attributes',w.DWORD)]
class PROCESS_ENTRY(ctypes.Structure):
 _fields_=[('size',w.DWORD),('usage',w.DWORD),('pid',w.DWORD),('heap',ctypes.c_size_t),
 ('module',w.DWORD),('threads',w.DWORD),('parent',w.DWORD),('priority',w.LONG),('flags',w.DWORD),('name',w.WCHAR*260)]
class TCP4(ctypes.Structure):
 _fields_=[('state',w.DWORD),('local_address',w.DWORD),('local_port',w.DWORD),
 ('remote_address',w.DWORD),('remote_port',w.DWORD),('pid',w.DWORD)]
class TCP6(ctypes.Structure):
 _fields_=[('local_address',ctypes.c_ubyte*16),('local_scope',w.DWORD),('local_port',w.DWORD),
 ('remote_address',ctypes.c_ubyte*16),('remote_scope',w.DWORD),('remote_port',w.DWORD),('state',w.DWORD),('pid',w.DWORD)]

class Native:
 def __init__(self):
  self.k=ctypes.WinDLL('kernel32',use_last_error=True)
  self.a=ctypes.WinDLL('advapi32',use_last_error=True)
  self.i=ctypes.WinDLL('iphlpapi',use_last_error=True)
  H=w.HANDLE;D=w.DWORD;B=w.BOOL;P=ctypes.POINTER
  specs=[(self.k,'OpenProcess',H,[D,B,D]),(self.k,'CloseHandle',B,[H]),
   (self.k,'CreateFileW',H,[w.LPCWSTR,D,D,w.LPVOID,D,D,H]),
   (self.k,'GetCurrentProcess',H,[]),(self.k,'WaitForSingleObject',D,[H,D]),
   (self.k,'GetProcessTimes',B,[H]+[P(FT)]*4),(self.k,'GetExitCodeProcess',B,[H,P(D)]),
   (self.k,'QueryFullProcessImageNameW',B,[H,D,w.LPWSTR,P(D)]),
   (self.k,'CreateToolhelp32Snapshot',H,[D,D]),(self.k,'Process32FirstW',B,[H,P(PROCESS_ENTRY)]),
   (self.k,'Process32NextW',B,[H,P(PROCESS_ENTRY)]),
   (self.a,'OpenProcessToken',B,[H,D,P(H)]),(self.a,'GetTokenInformation',B,[H,D,w.LPVOID,D,P(D)]),
   (self.a,'EqualSid',B,[w.LPVOID,w.LPVOID]),
   (self.i,'GetExtendedTcpTable',D,[w.LPVOID,P(D),B,D,D,D])]
  for lib,name,result,args in specs:
   fn=getattr(lib,name);fn.restype=result;fn.argtypes=args
  self.queries=[];self.pinned_files=[]
 def pin_file(self,path):
  h=self.k.CreateFileW(str(path),0x80000000,1,None,3,0x80,None)
  if h in (None,ctypes.c_void_p(-1).value):raise GuardError('input-source-read-lock-failed',ctypes.get_last_error())
  self.pinned_files.append(h)
 def check(self, ok, code):
  if not ok: raise GuardError(code,ctypes.get_last_error())
 def open(self,pid,allow_absent=False):
  h=self.k.OpenProcess(0x00101000,False,pid)
  if not h:
   error=ctypes.get_last_error()
   if allow_absent and error==87:return None
   raise GuardError('process-identity-unreadable',error)
  self.queries.append(h);return h
 def birth(self,h):
  fields=[FT() for _ in range(4)]
  self.check(self.k.GetProcessTimes(h,*map(ctypes.byref,fields)),'process-times-unreadable')
  return str((fields[0].high<<32)|fields[0].low)
 def live(self,h):
  value=self.k.WaitForSingleObject(h,0)
  require(value in (0,258),'process-wait-unreadable');return value==258
 def exit_code(self,h):
  if self.live(h):return None
  value=w.DWORD();self.check(self.k.GetExitCodeProcess(h,ctypes.byref(value)),'process-exit-unreadable')
  return value.value
 def image(self,h):
  size=w.DWORD(32768);buf=ctypes.create_unicode_buffer(size.value)
  self.check(self.k.QueryFullProcessImageNameW(h,0,buf,ctypes.byref(size)),'process-image-unreadable')
  return buf.value
 def token(self,h):
  t=w.HANDLE();self.check(self.a.OpenProcessToken(h,8,ctypes.byref(t)),'process-token-unreadable')
  try:
   needed=w.DWORD();self.a.GetTokenInformation(t,1,None,0,ctypes.byref(needed))
   require(needed.value>0,'process-token-size-unreadable')
   data=ctypes.create_string_buffer(needed.value)
   self.check(self.a.GetTokenInformation(t,1,data,needed.value,ctypes.byref(needed)),'process-user-unreadable')
   sid=ctypes.cast(data,ctypes.POINTER(SID_ATTR)).contents.sid
   session=w.DWORD();length=w.DWORD()
   self.check(self.a.GetTokenInformation(t,12,ctypes.byref(session),ctypes.sizeof(session),ctypes.byref(length)),'process-session-unreadable')
   return data,sid,session.value
  finally:self.check(self.k.CloseHandle(t),'token-handle-close-failed')
 def identity(self,h,pid,expected_image,expected_sha,session,manager_sid):
  require(self.live(h),'owned-process-not-live')
  image=self.image(h)
  require(os.path.normcase(image)==os.path.normcase(expected_image),'owned-image-path-differs')
  require(sha_file(image)==expected_sha,'owned-image-sha-differs')
  data,sid,actual_session=self.token(h)
  require(actual_session==session and bool(self.a.EqualSid(sid,manager_sid)),'owned-session-or-user-differs')
  return {'pid':pid,'createdExact':self.birth(h),'image':image,'sha256':expected_sha,
          'sessionId':actual_session,'sameUserAsManager':True}
 def processes(self):
  h=self.k.CreateToolhelp32Snapshot(2,0)
  require(h not in (None,ctypes.c_void_p(-1).value),'native-process-table-unreadable')
  try:
   row=PROCESS_ENTRY();row.size=ctypes.sizeof(row);out=[]
   ok=self.k.Process32FirstW(h,ctypes.byref(row))
   if not ok and ctypes.get_last_error()!=18:raise GuardError('native-process-table-unreadable',ctypes.get_last_error())
   while ok:
    out.append((row.pid,row.parent,row.name))
    ok=self.k.Process32NextW(h,ctypes.byref(row))
   require(ctypes.get_last_error()==18,'native-process-table-iteration-failed')
   return out
  finally:self.check(self.k.CloseHandle(h),'snapshot-handle-close-failed')
 def tcp(self):
  result=[]
  for family,row_type in ((socket.AF_INET,TCP4),(socket.AF_INET6,TCP6)):
   size=w.DWORD();error=self.i.GetExtendedTcpTable(None,ctypes.byref(size),False,family,5,0)
   require(error in (0,122) and size.value>=4,'native-tcp-size-unreadable')
   for _ in range(4):
    buf=ctypes.create_string_buffer(size.value)
    error=self.i.GetExtendedTcpTable(buf,ctypes.byref(size),False,family,5,0)
    if error!=122:break
   require(error==0,'native-tcp-table-unreadable')
   count=w.DWORD.from_buffer_copy(buf.raw[:4]).value
   require(4+count*ctypes.sizeof(row_type)<=len(buf),'native-tcp-table-invalid')
   for index in range(count):
    row=row_type.from_buffer_copy(buf.raw[4+index*ctypes.sizeof(row_type):4+(index+1)*ctypes.sizeof(row_type)])
    port=socket.ntohs(row.local_port&0xffff)
    if port not in PORTS:continue
    if family==socket.AF_INET:loopback=(int(row.local_address).to_bytes(4,'little')[0]==127)
    else:loopback=bytes(row.local_address)==b'\0'*15+b'\1'
    result.append({'port':port,'ownerPid':row.pid,'loopback':loopback,'state':row.state})
  return result
 def close_queries(self):
  for h in reversed(self.queries):self.check(self.k.CloseHandle(h),'query-handle-close-failed')
  self.queries.clear()
  for h in reversed(self.pinned_files):self.check(self.k.CloseHandle(h),'input-read-handle-close-failed')
  self.pinned_files.clear()

class Manager:
 def __init__(self,args):
  self.args=args;self.n=Native();self.children=[];self.server=None;self.head=None;self.qemu=None
  self.phase='binding-preflight';self.failure=None;self.quit_requested=False;self.quit_commands=set()
  self.binding=None;self.work=None;self.worker=None;self.identity=None;self.serial_verified=False
 def save(self,name,value):
  value={'schemaVersion':1,'observedAtUtc':utc(),'sourceRevision':SOURCE_REVISION,**value}
  path=self.work/name;temporary=self.work/(name+'.'+uuid.uuid4().hex+'.new')
  try:
   with open(temporary,'x',encoding='utf8') as f:json.dump(value,f,indent=2);f.write('\n')
   os.replace(temporary,path);return True
  except OSError:
   # Disk/publication errors cannot end a manager holding live native children.
   return False
 def pinned_json(self,path,expected_sha):
  require(re.fullmatch('[a-f0-9]{64}',expected_sha or '') is not None,'invalid-input-pin')
  self.n.pin_file(path)
  with open(path,'rb') as stream:raw=stream.read()
  require(hashlib.sha256(raw).hexdigest()==expected_sha,'input-pin-differs')
  return strict_json(raw)
 def verify_worker(self):
  expected=self.binding['nativeWorker']
  actual=self.n.identity(self.worker,expected['pid'],POWERSHELL,expected['imageSha256'],self.session,self.sid)
  require(actual['createdExact']==expected['createdExact'],'current-worker-birth-differs')
  return actual
 def preflight(self):
  require(sys.platform=='win32' and __debug__,'windows-native-nonoptimized-runtime-required')
  self.binding=self.pinned_json(self.args.bindings,self.args.bindings_sha256)
  b=self.binding
  require(b.get('schemaVersion')==1 and b.get('purpose')=='windows-original-five-owned-manager'
          and b.get('sourceRevision')==SOURCE_REVISION,'runtime-binding-contract-differs')
  self.n.pin_file(__file__)
  require(b.get('managerSourceSha256')==sha_file(__file__),'manager-source-pin-differs')
  require(re.fullmatch('[a-f0-9]{32}',b.get('namespaceId','')) is not None,'new-namespace-required')
  output=Path(b['outputDirectory'])
  require(os.path.normcase(str(output.parent))==os.path.normcase(OWN_BASE)
          and output.name=='runtime-'+b['namespaceId'],'new-output-namespace-differs')
  output.mkdir(exist_ok=False);self.work=output
  self.manager_handle=self.n.k.GetCurrentProcess();data,self.sid,self.session=self.n.token(self.manager_handle)
  self.sid_buffer=data
  self.manager_identity=self.n.identity(self.manager_handle,os.getpid(),*PINS['python'],self.session,self.sid)
  expected=b['nativeWorker']
  require(expected.get('imageSha256')==PINS['powershell'][1] and expected.get('sessionId')==self.session,'current-worker-binding-differs')
  self.worker=self.n.open(expected['pid']);self.verify_worker()
  gate=self.pinned_json(b['currentPreflightFile'],b['currentPreflightSha256'])
  stamp=datetime.fromisoformat(gate['observedAtUtc'].replace('Z','+00:00'))
  age=(datetime.now(timezone.utc)-stamp).total_seconds()
  require(0<=age<=300 and gate.get('status')=='readonly-windows-sdk-existing-avd-and-old-channel-preflight-successor2-passed'
          and gate.get('sourceRevision')==SOURCE_REVISION,'fresh-native-preflight-required')
  gw=gate.get('currentWorker',{})
  require(gw.get('pid')==expected['pid'] and gw.get('creationFiletimeExact')==expected['createdExact']
          and gw.get('imageSha256')==expected['imageSha256'] and gw.get('queryAndSynchronizeHandleHeldThroughObservation') is True,'native-preflight-worker-binding-differs')
  for _,(path,sha) in PINS.items():require(sha_file(path)==sha,'sdk-or-avd-pin-differs')
  self.n.pin_file(Path(__file__).with_name('read-current-same-avd-processes.ps1'))
  require(sha_file(Path(__file__).with_name('read-current-same-avd-processes.ps1'))==b['sameAvdGuardSha256'],'same-avd-guard-pin-differs')
  self.old_proof=[]
  for role,pid,created in OLD:
   h=self.n.open(pid,True);same=h is not None and self.n.birth(h)==created
   require(not(same and self.n.live(h)),'old-original-process-still-live')
   self.old_proof.append({'role':role,'pid':pid,'originalBirth':created,'absent':h is None,'reused':h is not None and not same,'matchingTerminal':same and not self.n.live(h)})
  require(not self.n.tcp(),'private-ports-not-currently-free')
  safe=self.same_avd_guard('same-avd-preflight')
  self.verify_worker();require(not self.n.tcp(),'private-ports-no-longer-free')
  require(self.save('preflight-safe.json',{'status':'fresh-current-worker-sdk-avd-ports-and-same-avd-gates-passed','namespaceId':b['namespaceId'],'bindingSha256':self.args.bindings_sha256,'oldExactProcesses':self.old_proof,'sameAvdProcessGuard':safe,'currentWorker':self.verify_worker(),'currentAvailabilityIsNotFutureReservation':True}),'preflight-receipt-publication-failed')
 def same_avd_guard(self,label):
  check=self.command(label,[POWERSHELL,'-NoProfile','-NonInteractive','-File',str(Path(__file__).with_name('read-current-same-avd-processes.ps1'))],60,False)
  require(check['exitCode']==0,'same-avd-current-process-gate-failed')
  safe=strict_json(check['stdoutPath'].read_bytes())
  require(safe.get('status')=='no-current-process-uses-original-medium-avd' and safe.get('candidateReadUnknown') is False
          and safe.get('currentSameAvdProcessCount')==0 and safe.get('sourceSha256')==self.binding['sameAvdGuardSha256'],'same-avd-current-process-gate-failed')
  return safe
 def environment(self):
  env=os.environ.copy()
  for key in ('ADB_SERVER_SOCKET','ANDROID_SERIAL','ADB_SERVER_PORT','ANDROID_SDK_HOME'):env.pop(key,None)
  env.update(ANDROID_ADB_SERVER_PORT='5039',ADB_LOCAL_TRANSPORT_MAX_PORT='0',ANDROID_SDK_ROOT=SDK,ANDROID_HOME=SDK,
             ANDROID_AVD_HOME=AVD_HOME,ANDROID_USER_HOME=r'C:\Users\han\.android',ANDROID_EMULATOR_HOME=r'C:\Users\han\.android')
  return env
 def spawn(self,label,args):
  out_path=self.work/(label+'-stdout-private.log');err_path=self.work/(label+'-stderr-private.log')
  with open(out_path,'xb') as out,open(err_path,'xb') as err:
   p=subprocess.Popen(args,cwd=str(self.work),env=self.environment(),stdin=subprocess.DEVNULL,stdout=out,stderr=err,creationflags=0x08000000)
   child={'label':label,'process':p,'handle':None,'stdoutPath':out_path,'exitCode':None}
   self.children.append(child) # Register ownership before stream close or identity work.
   child['handle']=int(p._handle)
  child['createdExact']=self.n.birth(child['handle']);return child
 def command(self,label,args,seconds,require_transport=True):
  if require_transport:self.refresh_owned()
  require(not any(c['label']==label for c in self.children),'command-already-started')
  child=self.spawn(label,args);deadline=time.monotonic()+seconds
  while self.n.live(child['handle']) and time.monotonic()<deadline:time.sleep(.1)
  child['exitCode']=self.n.exit_code(child['handle'])
  if child['exitCode'] is None:raise GuardError('owned-command-timeout-handles-retained')
  return child
 def server_guard(self):
  require(self.server is not None,'server-not-created')
  identity=self.n.identity(self.server['handle'],self.server['process'].pid,*PINS['adb'],self.session,self.sid)
  require(identity['createdExact']==self.server['createdExact'],'server-creator-birth-differs')
  rows=[r for r in self.n.tcp() if r['port']==5039 and r['state']==2]
  require(rows and all(r['ownerPid']==identity['pid'] and r['loopback'] for r in rows),'private-server-listener-owner-differs')
  return identity
 def refresh_owned(self):
  self.verify_worker();server=self.server_guard()
  require(self.head is not None and self.qemu is not None,'owned-head-and-qemu-not-bound')
  head=self.n.identity(self.head['handle'],self.head['process'].pid,*PINS['head'],self.session,self.sid)
  qemu=self.n.identity(self.qemu['handle'],self.qemu['pid'],*PINS['qemu'],self.session,self.sid)
  require(head['createdExact']==self.head['createdExact'] and qemu['createdExact']==self.qemu['createdExact'],'owned-creator-birth-differs')
  rows=self.n.processes()
  candidates=[r for r in rows if r[1]==head['pid'] and r[2].lower()=='qemu-system-x86_64-headless.exe']
  require(len(candidates)==1 and candidates[0][0]==qemu['pid'],'owned-qemu-parent-or-uniqueness-differs')
  require(int(qemu['createdExact'])>=int(head['createdExact']),'owned-qemu-birth-precedes-head')
  ports=self.n.tcp();expected={5039:server['pid'],5572:qemu['pid'],5573:qemu['pid'],19329:server['pid']}
  for port in (5039,5572,5573):
   listeners=[r for r in ports if r['port']==port and r['state']==2]
   require(listeners and all(r['ownerPid']==expected[port] and r['loopback'] for r in listeners),'private-listener-owner-differs')
  require(all(r['state']!=2 or(r['ownerPid']==expected[r['port']] and r['loopback']) for r in ports),'private-channel-has-foreign-listener')
  return {'schemaVersion':1,'status':'current-owned-windows-medium-target-verified','namespaceId':self.binding['namespaceId'],
   'bindingSha256':self.args.bindings_sha256,'manager':self.manager_identity,'worker':self.verify_worker(),
   'server':server,'head':head,'qemu':qemu,'actualAvdName':AVD_NAME,'actualAvdHost':'Windows','serial':'emulator-5572',
   'serverPort':5039,'consolePort':5572,'guestAdbPort':5573,'cdpPort':19329,'nativePortOwners':ports,
   'originalCreatorHandlesHeld':True,'qemuQueryAndSynchronizeHandleHeld':True,'transportIdentityCommandsCompleted':self.serial_verified,
   'appOrCredentialOrHttpOperations':0,'rawCredentialsOrCommandLinesIncluded':False}
 def wait_private_registration(self,adb,maximum_seconds=240):
  deadline=time.monotonic()+maximum_seconds;probe=0
  while time.monotonic()<deadline:
   self.refresh_owned();probe+=1
   child=self.command('devices-registration-probe-'+str(probe).zfill(4),adb+['devices'],min(30,max(.1,deadline-time.monotonic())))
   require(child['exitCode']==0,'private-adb-registration-probe-exit-failed')
   lines=child['stdoutPath'].read_text(encoding='utf8',errors='strict').splitlines()
   nonempty=[line.strip() for line in lines if line.strip()]
   require(nonempty and nonempty[0]=='List of devices attached','private-adb-registration-output-unrecognized')
   targets=[line.split() for line in nonempty[1:]]
   require(all(len(row)==2 for row in targets),'private-adb-registration-output-unrecognized')
   require(all(row[0]=='emulator-5572' for row in targets),'private-adb-registration-unexpected-extra-target')
   require(len(targets)<=1,'private-adb-registration-target-not-unique')
   require(all(row[1] in ('device','offline') for row in targets),'private-adb-registration-state-unrecognized')
   self.refresh_owned()
   ready=bool(targets) and targets[0][1]=='device'
   require(self.save('devices-registration-safe.json',{'status':'original-private-adb-target-registered' if ready else 'owned-original-target-registration-wait',
    'namespaceId':self.binding['namespaceId'],'bindingSha256':self.args.bindings_sha256,'probeNumber':probe,
    'nativeProbeExitCode':child['exitCode'],'targetCount':len(targets),'unexpectedTargetCount':0,
    'originalTargetState':'device' if ready else 'offline' if targets else 'not-yet-registered',
    'currentNativeOwnershipVerifiedBeforeAndAfterProbe':True,'rawCommandOutputOrPrivateValuesIncluded':False,
    'maximumWaitSeconds':maximum_seconds,'timeoutNeverForcesNativeExit':True}),'registration-receipt-publication-failed')
   if ready:return
   time.sleep(min(.25,max(0,deadline-time.monotonic())))
  raise GuardError('private-adb-original-target-registration-timeout')
 def start(self):
  self.phase='private-server-start';self.server=self.spawn('server',[PINS['adb'][0],'-P','5039','server','nodaemon'])
  deadline=time.monotonic()+30
  while time.monotonic()<deadline:
   require(self.n.live(self.server['handle']),'owned-server-exited-before-ready')
   if any(r['port']==5039 and r['state']==2 and r['ownerPid']==self.server['process'].pid for r in self.n.tcp()):break
   time.sleep(.2)
  self.server_guard();self.verify_worker()
  self.same_avd_guard('same-avd-before-head')
  self.server_guard();self.verify_worker()
  require(not any(r['port'] in (5572,5573,19329) for r in self.n.tcp()),'emulator-ports-no-longer-free')
  self.phase='original-avd-headless-start'
  self.head=self.spawn('head',[PINS['head'][0],'-avd',AVD_NAME,'-port','5572','-no-window','-no-snapshot-load','-no-snapshot-save','-gpu','swiftshader'])
  deadline=time.monotonic()+120
  while time.monotonic()<deadline:
   require(self.n.live(self.head['handle']),'owned-head-exited-before-ready')
   rows=[r for r in self.n.processes() if r[1]==self.head['process'].pid and r[2].lower()=='qemu-system-x86_64-headless.exe']
   require(len(rows)<=1,'multiple-owned-qemu-candidates')
   if rows and self.qemu is None:
    h=self.n.open(rows[0][0]);self.qemu={'pid':rows[0][0],'handle':h,'createdExact':self.n.birth(h)}
   if self.qemu is not None and all(any(r['port']==port and r['state']==2 and r['ownerPid']==self.qemu['pid'] for r in self.n.tcp()) for port in (5572,5573)):break
   time.sleep(.2)
  self.identity=self.refresh_owned()
  # These are target identity requests only, after native ownership is verified.
  adb=[PINS['adb'][0],'-H','127.0.0.1','-P','5039']
  self.wait_private_registration(adb)
  avd=self.command('avd-name-identity',adb+['-s','emulator-5572','emu','avd','name'],30)
  avd_lines=[line.strip() for line in avd['stdoutPath'].read_text(encoding='utf8').splitlines() if line.strip()]
  require(avd['exitCode']==0 and avd_lines==[AVD_NAME,'OK'],'native-avd-name-differs')
  self.serial_verified=True;self.phase='current-owned-target-ready';self.save('identity-safe.json',self.refresh_owned())
 def poll_children(self):
  for child in self.children:
   child['exitCode']=self.n.exit_code(child['handle'])
   if child['exitCode'] is not None:child['process'].returncode=child['exitCode']
 def control(self):
  file=self.work/'control.json'
  if not file.exists():return
  value=strict_json(file.read_bytes())
  require(value=={'schemaVersion':1,'action':'normal_quit','namespaceId':self.binding['namespaceId'],
                 'bindingSha256':self.args.bindings_sha256,'managerPid':os.getpid(),
                 'managerCreatedExact':self.manager_identity['createdExact']},'normal-quit-control-binding-differs')
  self.quit_requested=True
 def normal_cleanup(self):
  adb=[PINS['adb'][0],'-H','127.0.0.1','-P','5039']
  if self.head is not None and self.n.live(self.head['handle']):
   if 'emu-kill' not in self.quit_commands:
    self.refresh_owned();self.quit_commands.add('emu-kill')
    self.command('normal-emu-kill',adb+['-s','emulator-5572','emu','kill'],30)
   return
  if self.qemu is not None and self.n.live(self.qemu['handle']):return
  if any(c['label'] not in ('server','head') and self.n.live(c['handle']) for c in self.children):return
  if self.server is not None and self.n.live(self.server['handle']):
   if 'server-kill' not in self.quit_commands:
    self.verify_worker();self.server_guard();self.quit_commands.add('server-kill')
    self.command('normal-server-kill',adb+['kill-server'],30,False)
 def retained_loop(self):
  while True:
   try:
    self.poll_children();self.control()
    if self.quit_requested:self.normal_cleanup()
    if self.server is not None and self.head is not None and not self.quit_requested and self.failure is None:
     self.save('identity-safe.json',self.refresh_owned())
    # An emulator was launched but QEMU identity was never captured: retain
    # ownership as unknown, rather than treating an unobserved child as absent.
    qemu_terminal=(self.head is None) or (self.qemu is not None and not self.n.live(self.qemu['handle']))
    all_terminal=all(c['exitCode'] is not None for c in self.children)
    if all_terminal and qemu_terminal:
     ports=self.n.tcp()
     if not ports:
      codes={c['label']:c['exitCode'] for c in self.children}
      qemu_code=None if self.qemu is None else self.n.exit_code(self.qemu['handle'])
      completed=self.quit_requested and self.failure is None and qemu_code==0 and all(value==0 for value in codes.values())
      published=self.save('terminal-safe.json',{'status':'normal-owned-channel-terminal-and-ports-released' if completed else 'owned-channel-terminal-with-preserved-failure',
       'namespaceId':self.binding['namespaceId'],'bindingSha256':self.args.bindings_sha256,'manager':self.manager_identity,
       'originalCreatorExitCodes':codes,'qemuQueryHandleExitCode':qemu_code,
       'allCreatedNativeHandlesActuallyTerminal':True,'allFourPrivatePortsReleased':True,'normalQuitRequested':self.quit_requested,
       'failureCode':self.failure,'forcedTerminations':0,'appOrCredentialOrHttpOperations':0})
      if published:return 0 if completed else 1
    self.save('manager-current-safe.json',{'status':'owned-handles-retained-awaiting-normal-terminal','phase':self.phase,
     'namespaceId':self.binding['namespaceId'],'bindingSha256':self.args.bindings_sha256,'manager':self.manager_identity,
     'normalQuitRequested':self.quit_requested,'failureCode':self.failure,'originalCreatorExitCodes':{c['label']:c['exitCode'] for c in self.children},
     'timeoutDoesNotCauseForcedExit':True,'forcedTerminations':0,'appOrCredentialOrHttpOperations':0})
   except Exception as error:
    self.failure=error.code if isinstance(error,GuardError) else 'native-observation-or-control-failed'
    self.save('identity-safe.json',{'status':'owned-target-current-verification-failed','namespaceId':self.binding['namespaceId'],'failureCode':self.failure})
    self.save('failure-safe.json',{'status':'failure-owned-handles-retained','phase':self.phase,'failureCode':self.failure,
      'nativeError':error.native_error if isinstance(error,GuardError) else None,'errorType':type(error).__name__,
      'rawErrorsIncluded':False,'forcedTerminations':0,'appOrCredentialOrHttpOperations':0})
   time.sleep(.5)
 def run(self):
  try:
   self.preflight();self.start()
  except Exception as error:
   self.failure=error.code if isinstance(error,GuardError) else 'native-startup-preflight-failed'
   if self.work is not None:self.save('failure-safe.json',{'status':'preflight-or-startup-failed','phase':self.phase,'failureCode':self.failure,
    'errorType':type(error).__name__,'nativeError':error.native_error if isinstance(error,GuardError) else None,'rawErrorsIncluded':False,
    'childrenCreated':len(self.children),'forcedTerminations':0,'appOrCredentialOrHttpOperations':0})
   if not self.children:return 1
  return self.retained_loop()

def main():
 parser=argparse.ArgumentParser();parser.add_argument('--bindings',required=True);parser.add_argument('--bindings-sha256',required=True)
 args=parser.parse_args()
 if sys.platform!='win32' or not __debug__:
  print(json.dumps({'status':'windows-native-unoptimized-runtime-required','productsStarted':0}));return 1
 manager=Manager(args)
 try:return manager.run()
 finally:
  # run returns only before children start or after all original child HANDLEs exit.
  manager.n.close_queries()

if __name__=='__main__':sys.exit(main())
