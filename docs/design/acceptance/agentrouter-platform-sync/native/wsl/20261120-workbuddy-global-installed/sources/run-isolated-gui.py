import json,subprocess,os,time,hashlib
from pathlib import Path
from datetime import datetime,timezone
b=Path(__file__).parent;fresh=b/'gui-xvfb-attempt3';fresh.mkdir(exist_ok=False);(fresh/'tools').mkdir();(fresh/'host-bin').mkdir();tools=Path('/home/han/.local/state/orchester-delivery/20261120/workbuddy-gui-tools/root');controller=b/'gui-isolated-controller.py';(fresh/'controller.py').write_bytes(controller.read_bytes());prefix=Path('/home/han/.local/opt/workbuddy-ai/5.7.6.40488862-3e43260f');runtime=Path('/home/han/.local/lib/orchester-linux-gui-runtime');launcher=Path('/home/han/.local/bin/workbuddy-ai')
args=['bwrap','--unshare-all','--die-with-parent','--new-session','--ro-bind','/','/','--tmpfs','/home','--dir','/home/han']
for p in [prefix,runtime,launcher]:args+=['--ro-bind',str(p),str(p)]
args+=['--proc','/proc','--dev','/dev','--tmpfs','/tmp','--bind',str(fresh),'/tmp/orchester-smoke','--ro-bind',str(tools),'/tmp/orchester-smoke/tools','--ro-bind','/usr/bin','/tmp/orchester-smoke/host-bin','--ro-bind',str(tools.parent/'bin-mirror'),'/usr/bin','--tmpfs','/run','--clearenv']
for k,v in {'PATH':'/usr/bin:/bin','HOME':'/tmp/orchester-smoke/home','USER':'han','LOGNAME':'han','LANG':'C.UTF-8','XDG_CONFIG_HOME':'/tmp/orchester-smoke/home/.config','XDG_CACHE_HOME':'/tmp/orchester-smoke/home/.cache','XDG_DATA_HOME':'/tmp/orchester-smoke/home/.local/share','XDG_RUNTIME_DIR':'/tmp/orchester-smoke/runtime','LD_LIBRARY_PATH':'/tmp/orchester-smoke/tools/usr/lib/x86_64-linux-gnu:'+str(runtime/'usr/lib/x86_64-linux-gnu'),'GDK_BACKEND':'x11'}.items():args+=['--setenv',k,v]
args+=['--chdir','/tmp/orchester-smoke','/usr/bin/python3','-B','/tmp/orchester-smoke/controller.py']
beg=time.monotonic();p=subprocess.Popen(args,stdout=(fresh/'outer.stdout.safe').open('wb'),stderr=(fresh/'outer.stderr.private').open('wb'));print(json.dumps({'liveLinuxProcess':p.pid,'startedAtUtc':datetime.now(timezone.utc).isoformat(),'fresh':str(fresh)}),flush=True)
try:code=p.wait(timeout=110)
except subprocess.TimeoutExpired:print(json.dumps({'liveProcess':p.pid,'observationTimeout':True}),flush=True);raise
r={'command':args,'actualOuterExitCode':code,'elapsedSeconds':time.monotonic()-beg,'controllerSha256':hashlib.sha256(controller.read_bytes()).hexdigest()};(fresh/'outer-execution-safe.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r));print((fresh/'result-safe.json').read_text() if (fresh/'result-safe.json').exists() else 'no-inner-result')
