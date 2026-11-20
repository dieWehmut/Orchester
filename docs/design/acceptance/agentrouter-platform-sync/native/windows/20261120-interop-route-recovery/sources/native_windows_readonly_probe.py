import base64,json,os,subprocess,time
from pathlib import Path
root=Path(__file__).parent;cmd=str(root/'owned-native-cmd.exe')
def run_ps(script):
 enc=base64.b64encode(script.encode('utf-16le')).decode('ascii')
 env=os.environ.copy();env['WSL_INTEROP']='/run/WSL/357_interop'
 t=time.monotonic();p=subprocess.run(['/init',cmd,'cmd.exe','/d','/c',r'C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe','-NoLogo','-NoProfile','-NonInteractive','-EncodedCommand',enc],executable='/init',cwd=str(root),env=env,capture_output=True,timeout=15)
 return {'exitCode':p.returncode,'seconds':time.monotonic()-t,'stdout':p.stdout.decode(errors='replace')[:5000],'stderr':p.stderr.decode(errors='replace')[:2000]}
rows=[]
rows.append({'label':'ascii-interoperability-sentinel','command':'echo ORCHESTER_NATIVE_ROUTE_OK',**run_ps("Write-Output 'ORCHESTER_NATIVE_ROUTE_OK'")})
script="""$tcp=@(Get-NetTCPConnection -LocalPort 7338 -ErrorAction SilentlyContinue | Select-Object LocalAddress,LocalPort,State,OwningProcess);$owners=@();foreach($t in $tcp){try{$owners += Get-Process -Id $t.OwningProcess -ErrorAction Stop | Select-Object Id,ProcessName,StartTime}catch{}};$wsl=@();try{$wsl += (& C:\\Windows\\System32\\wsl.exe --status 2>&1 | Out-String).Trim()}catch{$wsl += 'WSL_STATUS_ERROR'};[pscustomobject]@{tcpCount=$tcp.Count;tcp=$tcp;ownerCount=$owners.Count;owners=$owners;wslStatus=$wsl} | ConvertTo-Json -Depth 5 -Compress"""
rows.append({'label':'windows-port-and-wsl-readonly','command':'Get-NetTCPConnection 7338 + wsl --status',**run_ps(script)})
print(json.dumps({'observedAtUtc':__import__('datetime').datetime.now(__import__('datetime').timezone.utc).isoformat(),'nativeReadOnly':True,'processSignals':0,'fileWrites':0,'probes':rows},ensure_ascii=False,indent=2))
(root/'native-windows-readonly-probe-safe.json').write_text(json.dumps({'probes':rows},ensure_ascii=False,indent=2)+'\n')
