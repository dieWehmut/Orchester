#!/usr/bin/env python3
"""Read only explicitly selected installed public entries; never start agent CLIs."""
import datetime,hashlib,json,os,pathlib,shutil
OUT=pathlib.Path(__file__).resolve().parent
BASE=OUT.parent
sources={}
checks=[]
def load(rel):
 p=BASE/rel; raw=p.read_bytes();sources[rel]={'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw)};return json.loads(raw)
def local(path):
 return pathlib.Path('/mnt/'+path[0].lower()+'/'+path[3:].replace('\\','/')) if len(path)>2 and path[1:3]==':\\' else pathlib.Path(path)
def check(platform,tool,kind,path,baseline=None,metadata=False):
 p=local(path);r={'platform':platform,'tool':tool,'kind':kind,'path':path,'exists':p.is_file()}
 if platform=='windows' and not pathlib.Path('/mnt/c/Windows').is_dir():
  r.update(exists=None,observationStatus='unavailable-in-current-runtime-no-Windows-mount');checks.append(r);return r
 if p.is_file():
  h=hashlib.sha256()
  with p.open('rb') as f:
   for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
  r.update(bytes=p.stat().st_size,sha256=h.hexdigest())
  if baseline:r.update(baselineSha256=baseline,matchesSelectedBaseline=h.hexdigest()==baseline)
  if metadata:
   x=json.loads(p.read_text());r['publicManifest']={k:x[k] for k in ['name','version','publisher','bin'] if k in x}
 checks.append(r);return r
win=load('windows/current-installation-matrix/final-current-windows-installation-matrix-safe.json')
for r in win['cliTools']+win['desktopApplications']:
 check('windows',r['id'],'installed-entry',r['entryWindowsPath'],r.get('entrySha256'))
 m=r.get('currentPublicPackageManifest')
 if m:check('windows',r['id'],'public-package-manifest',m['windowsPath'],m['sha256'],True)
for r in win['editorExtensions']:
 check('windows',r['id'],'extension-manifest',r['publicManifestWindowsPath'],r['publicManifestSha256'],True)
wsl=load('wsl/current-matrix/current-native-installation-matrix-safe.json')
for r in wsl['entries']:
 for key in ['entry','launcher','publicManifest']:
  m=r.get(key)
  if m:check('wsl',r['provider'],key,m['path'],m.get('currentSha256',m.get('expectedSha256')),key=='publicManifest')
 if r.get('currentPackageMetadataPath'):check('wsl',r['provider'],'public-package-manifest',r['currentPackageMetadataPath'],r.get('currentPackageMetadataSha256'),True)
legacy=load('windows/legacy-current-public-metadata/current-dsh-cursor-antigravity-wslhost-public-safe.json')
for r in legacy['files']:
 if not r['installerFilename'] and any(x in r['path'].lower() for x in ['antigravity.exe','cursor.exe']):check('windows','antigravity' if 'antigravity' in r['path'].lower() else 'cursor','installed-entry',r['path'],r['sha256'])
dsh=load('windows/deepseek-official-installed-20261112/corrected-verification-safe.json')
check('windows','deepseek','installed-entry',dsh['currentUserRegistration']['location']+'\\DeepSeek Harness.exe',dsh['installedImageSha256'])
legacywsl=load('wsl/legacy-current/final-wsl-install-safe.json')
for r in legacywsl['products']:
 check('wsl',r['id'],'launcher',r['launcher'])
 if r['id']=='cursor':check('wsl','cursor','installed-entry',r['installRoot']+'/usr/share/cursor/cursor',r['nativeElfSha256'])
 if r['id']=='antigravity':check('wsl','antigravity','installed-entry',r['installRoot']+'/Antigravity-x64/antigravity',r['nativeElfSha256'])
dshwsl=load('wsl/legacy-current/dsh-native-cli-checks-safe.json')['actualInstalledPackage']
check('wsl','deepseek','public-package-manifest',dshwsl['metadataPath'],dshwsl['sha256'],True)
additional=load('additional-agents/root-integration/command-entry-matrix-safe.json')
wrappers=load('additional-agents/root-integration/registered-command-entries-safe.json')
for r in wrappers['commands']:
 check('windows',r['command'],'registered-wrapper',r['wrapper'],r['sha256'])
 check('windows',r['command'],'wrapper-target',r['target'])
for r in additional['tools']:
 q=check('wsl',r['command'],'registered-command',r['wslUserBin'])
 q.update(recordedTarget=r['wsl'],currentRealPath=str(local(r['wslUserBin']).resolve()),recordedTargetRealPath=str(local(r['wsl']).resolve()),currentShellResolution=shutil.which(r['command']))
 q['registrationTargetMatches']=q['currentRealPath']==q['recordedTargetRealPath']
load('additional-agents/root-integration/registered-command-version-gates-safe.json')
load('additional-agents/root-integration/wsl-registered-command-version-gates-safe.json')
# Frozen reviewed Claude proof is referenced, not regenerated or mutated.
load('claude/20261119-official-installations/windows/original/current-registered-installed-proof-safe.json')
load('claude/20261119-official-installations/linux/evidence/20261119/final-native-linux-verification-safe.json')
result={'schemaVersion':1,'observedAtUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'scope':'selected installed public entries, manifests, and registered wrappers; source-backed historical execution retained separately','sources':sources,'checks':checks,'summary':{'checks':len(checks),'missingSelectedPaths':sum(r['exists'] is False for r in checks),'unavailableSelectedWindowsPaths':sum(r['exists'] is None for r in checks),'changedSelectedBaselineFiles':sum(r.get('matchesSelectedBaseline') is False for r in checks),'additionalWslBindingsMatch':all(r.get('registrationTargetMatches',True) for r in checks)},'limits':['The current worker runtime exposes no /mnt/c/Windows or /mnt/c/Users/han; Windows selected paths are unavailable to observe, not absent.','No agent CLI, Windows Codex, GUI, login, model, quota, installer, profile or credential operation was run.','Hashes prove only selected files, not whole packages, executable version output, model support, account availability, or GUI acceptance.','Historical registration is not refreshed by mounted-file hashing.','WorkBuddy Global Linux is assigned to its independent owner; no official Linux payload inferred from domestic WorkBuddy.','Claude official installation receipts are frozen source references from 2026-11-19.']}
(OUT/'current-public-entry-audit-safe.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(result['summary']))
for r in checks:
 if r['exists'] is False or r.get('matchesSelectedBaseline') is False: print(json.dumps(r,ensure_ascii=False))
