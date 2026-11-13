import argparse,hashlib,hmac,json,os,secrets,re
from pathlib import Path
from datetime import datetime,timezone,timedelta
p=argparse.ArgumentParser();p.add_argument('operation',choices=['run','stop']);p.add_argument('--script');a=p.parse_args()
state=json.loads(Path('/home/han/.local/state/orchester-delivery/20261113/root-resumed-continuation/native-worker-state-private.json').read_text());root=Path(state['linuxRoot'])
ready=json.loads((root/'worker.ready-safe.json').read_text(encoding='utf-8-sig'));heartbeat=json.loads((root/'worker.heartbeat-safe.json').read_text(encoding='utf-8-sig'))
heartbeat_age_text=re.sub(r'(\.\d{6})\d+(?=[+-]\d{2}:\d{2}$)',r'\1',heartbeat['timeUtc'].replace('Z','+00:00'))
now=datetime.now(timezone.utc);age=(now-datetime.fromisoformat(heartbeat_age_text)).total_seconds()
assert ready['state']=='ready' and heartbeat['state']=='running' and 0<=age<15
assert ready['pid']==heartbeat['pid'] and ready['exactBirthUtc']==heartbeat['exactBirthUtc'] and ready['nonce']==heartbeat['nonce']
assert ready['sourceSha256']==hashlib.sha256((root/'worker.ps1').read_bytes()).hexdigest()
if a.operation=='run':assert a.script
body=Path(a.script).read_text() if a.script else '# Authenticated normal stop; no source command is executed.\n'
id=secrets.token_hex(16);out=state['windowsRoot']+'\\jobs\\'+id
body=body.replace('__JOB_OUTPUT_WINDOWS__',out)
source=root/'sources'/(id+'.ps1');source.write_text(body,encoding='utf-8');sha=hashlib.sha256(source.read_bytes()).hexdigest()
expires=(now+timedelta(minutes=10)).isoformat();filename=source.name
canonical='\n'.join([id,a.operation,filename,sha,expires]).encode('utf-8')
key=(root/'bridge-key-private.bin').read_bytes();assert len(key)==32
request={'id':id,'operation':a.operation,'filename':filename,'sourceSha256':sha,'expiresAtUtc':expires,'mac':hmac.new(key,canonical,hashlib.sha256).hexdigest()}
temp=root/'requests'/('request-'+id+'.tmp');temp.write_text(json.dumps(request)+'\n');os.replace(temp,root/'requests'/('request-'+id+'.json'))
print(json.dumps({'id':id,'operation':a.operation,'sourceSha256':sha,'workerPid':ready['pid'],'workerExactBirthUtc':ready['exactBirthUtc'],'heartbeatAgeSeconds':round(age,3),'requestSubmitted':True,'keyOrMacPrinted':False}))
