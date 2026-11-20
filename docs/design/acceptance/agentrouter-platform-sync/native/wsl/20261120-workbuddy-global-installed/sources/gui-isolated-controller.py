#!/usr/bin/env python3
import os,sys,json,subprocess,time,select,signal,hashlib
from pathlib import Path
from datetime import datetime,timezone
root=Path('/tmp/orchester-smoke');tools=root/'tools';home=root/'home';home.mkdir(exist_ok=True);(root/'runtime').mkdir(exist_ok=True);os.chmod(root/'runtime',0o700)
result={'startedAtUtc':datetime.now(timezone.utc).isoformat(),'status':'running','networkNamespaceIsolated':True,'originalHomeHidden':True,'freshHome':str(home),'appSignals':[],'nativeKeyboardActions':[],'pidfdIdentities':[]}
x=None;app=None;fds={};counts={};outf=None;errf=None
def collect():
 for p in Path('/proc').iterdir():
  if not p.name.isdigit() or int(p.name)==os.getpid():continue
  pid=int(p.name)
  try:
   parts=(p/'stat').read_text().rsplit(')',1)[1].split();start=int(parts[19]);identity=(pid,start)
   if identity not in fds:
    try:exe=Path(os.readlink(p/'exe')).name
    except OSError:exe='process-exited-during-own-namespace-observation'
    fd=os.pidfd_open(pid);counts[identity]={'pid':pid,'birthTicks':start,'exeBasename':exe,'terminal':False};fds[identity]=fd
  except (OSError,ValueError,IndexError):continue
 for identity,fd in fds.items():
  if select.select([fd],[],[],0)[0]:counts[identity]['terminal']=True

def tool(args,timeout=4):
 return subprocess.run([str(tools/'usr/bin/xdotool')]+args,env=os.environ.copy(),capture_output=True,text=True,timeout=timeout)
try:
 Path('/tmp/.X11-unix').mkdir(mode=0o1777,exist_ok=True);os.chmod('/tmp/.X11-unix',0o1777)
 rfd,wfd=os.pipe();xlog=(root/'xvfb.stderr.private').open('wb');x=subprocess.Popen([str(tools/'usr/bin/Xvfb'),'-displayfd',str(wfd),'-screen','0','1200x800x24','-nolisten','tcp','-ac','-xkbdir','/usr/share/X11/xkb'],pass_fds=(wfd,),stdout=subprocess.DEVNULL,stderr=xlog);os.close(wfd)
 ready=select.select([rfd],[],[],8)[0]
 if not ready:raise RuntimeError('owned-Xvfb-displayfd-timeout')
 number=os.read(rfd,64).decode().strip();os.close(rfd)
 if not number.isdigit():raise RuntimeError('owned-Xvfb-display-not-integer')
 os.environ['DISPLAY']=':'+number;result['ownedDisplay']=os.environ['DISPLAY'];result['xvfbPid']=x.pid
 outf=(root/'app.stdout.private').open('wb');errf=(root/'app.stderr.private').open('wb')
 app=subprocess.Popen(['/home/han/.local/bin/workbuddy-ai','--ozone-platform=x11','--disable-gpu','--disable-dev-shm-usage','--remote-debugging-address=127.0.0.1','--remote-debugging-port=9335'],stdout=outf,stderr=errf,start_new_session=True)
 result['appPid']=app.pid;collect();deadline=time.monotonic()+45;window=None
 while time.monotonic()<deadline:
  collect()
  if app.poll() is not None:raise RuntimeError('app-exited-before-window')
  r=tool(['search','--onlyvisible','--class','[Ww]orkbuddy'])
  if r.returncode==0 and r.stdout.strip():
   for candidate in r.stdout.split():
    g=tool(['getwindowgeometry','--shell',candidate]);geom={k:int(v) for k,v in [line.split('=',1) for line in g.stdout.splitlines() if '=' in line] if k in ['X','Y','WIDTH','HEIGHT','SCREEN','WINDOW']}
    if geom.get('WIDTH',0)>=400 and geom.get('HEIGHT',0)>=300:window=candidate;result['windowGeometry']=geom;break
  if window:break
  time.sleep(.25)
 if not window:raise RuntimeError('owned-native-window-not-visible')
 result['window']=int(window);result['windowTitle']=tool(['getwindowname',window]).stdout.strip();result['mappedWindowObserved']=True
 for _ in range(12):collect();time.sleep(.25)
 result['freshInternationalProfileCreated']=(home/'.workbuddy-ai/app').is_dir();result['domesticProfileCreated']=(home/'.workbuddy').exists()
 focus=tool(['windowfocus','--sync',window]);result['windowFocusExitCode']=focus.returncode
 quitcmd=tool(['key','--clearmodifiers','ctrl+q']);result['nativeKeyboardActions'].append({'keys':'ctrl+q','exitCode':quitcmd.returncode,'targetOwnedDisplay':True})
 deadline=time.monotonic()+18
 while app.poll() is None and time.monotonic()<deadline:collect();time.sleep(.25)
 result['appNativeExitCode']=app.poll()
 if app.poll() is None:raise RuntimeError('normal-CtrlQ-did-not-exit-app')
 result['normalProductQuitVerified']=app.returncode==0
 for _ in range(16):collect();time.sleep(.1)
 result['status']='pass' if app.returncode==0 else 'app-nonzero-native-exit'
except Exception as e:
 result['status']='failed';result['failure']=str(e);result['exceptionType']=type(e).__name__
finally:
 collect()
 if app and app.poll() is None:
  result['appSignals'].append({'signal':'SIGTERM','scope':'dedicated-test-app-process-group','failureCleanupOnly':True});os.killpg(app.pid,signal.SIGTERM)
  try:app.wait(timeout=6)
  except subprocess.TimeoutExpired:result['appSignals'].append({'signal':'SIGKILL','scope':'dedicated-test-app-process-group','failureCleanupOnly':True});os.killpg(app.pid,signal.SIGKILL);app.wait(timeout=3)
  result['cleanupAppExitCode']=app.returncode
 collect()
 appident=[v for k,v in counts.items() if not x or k[0]!=x.pid]
 result['allObservedAppPidfdsTerminalBeforeXvfbStop']=all(v['terminal'] for v in appident);result['observedAppIdentityCount']=len(appident)
 if x:
  x.terminate()
  try:x.wait(timeout=5)
  except subprocess.TimeoutExpired:x.kill();x.wait(timeout=3)
  result['ownedXvfbExitCode']=x.returncode
 collect();result['pidfdIdentities']=list(counts.values());result['allObservedPidfdsTerminalAfterOwnCleanup']=all(v['terminal'] for v in counts.values())
 for fd in fds.values():os.close(fd)
 if outf:outf.close()
 if errf:errf.close()
 result['endedAtUtc']=datetime.now(timezone.utc).isoformat()
 (root/'result-safe.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print(json.dumps(result,ensure_ascii=False))
 sys.exit(0 if result['status']=='pass' and not result['appSignals'] else 1)
