#!/usr/bin/env python3
"""Read-only comparison of the recorded731 public source inputs to Git and disk."""
import hashlib
import json
import subprocess
from pathlib import Path

def check(root):
    base=root/'docs/design/acceptance/agentrouter-platform-sync'
    source=base/'sources/final71-source-binding.json'
    original=json.loads(source.read_bytes())
    rows=original['sourceRows']
    def blobs(revision,paths):
        query=''.join(f'{revision}:{p}\n' for p in paths).encode()
        raw=subprocess.check_output(['git','cat-file','--batch'],input=query,cwd=root)
        result={};pos=0
        for path in paths:
            end=raw.index(b'\n',pos);head=raw[pos:end].decode().split();pos=end+1
            if len(head)!=3 or head[1]!='blob':raise ValueError(f'missing Git input: {path}')
            size=int(head[2]);result[path]=raw[pos:pos+size];pos+=size+1
        return result
    paths=[r['path'] for r in rows]
    frozen=blobs(original['sourceRevision'],paths)
    current=blobs('HEAD',paths)
    changed=[];backend=0;frontend=0
    expected_changes={'apps/web/site/index.html':'4234c704d98b20e625359a4e2c5dbca7c12acac0','apps/web/site/src/styles/site.css':'4234c704d98b20e625359a4e2c5dbca7c12acac0','packages/design/src/fonts/README.md':'db80049c0f4aaebb6340d920c7c89b2cd6f002b4'}
    for row in rows:
        path=row['path'];actual=(root/path).read_bytes()
        if hashlib.sha256(frozen[path]).hexdigest()!=row['sha256']:raise ValueError('original row is not frozen Git blob: '+path)
        if actual!=current[path]:raise ValueError('working source differs HEAD: '+path)
        if 'backendFull368' in row['families']:
            if actual!=frozen[path]:raise ValueError('backend source changed: '+path)
            backend+=1
        if actual!=frozen[path]:
            commit=expected_changes.get(path)
            if not commit or actual!=blobs(commit,[path])[path]:raise ValueError('unreviewed source change: '+path)
            changed.append({'path':path,'frozen71Sha256':row['sha256'],'currentSha256':hashlib.sha256(actual).hexdigest(),'currentBytes':len(actual),'reviewedChangeCommit':commit})
        elif any('frontend' in family.lower() for family in row['families']):frontend+=1
    if set(x['path'] for x in changed)!=set(expected_changes):raise ValueError('expected reviewed source changes differ')
    icon='apps/web/site/public/favicon.svg';icon_bytes=(root/icon).read_bytes()
    if icon_bytes!=blobs(expected_changes['apps/web/site/index.html'],[icon])[icon]:raise ValueError('site favicon changed')
    return {'status':'pass','recordedSourceBindingSha256':hashlib.sha256(source.read_bytes()).hexdigest(),'recordedInputCount':len(rows),'frozenBaselineRevision':original['sourceRevision'],'unchangedRecordedInputs':len(rows)-len(changed),'backendInputsIdenticalToFrozen71':backend,'changedInputs':changed,'addedSiteFavicon':{'path':icon,'sha256':hashlib.sha256(icon_bytes).hexdigest(),'bytes':len(icon_bytes)},'scope':'Only site HTML/CSS and font README differ from frozen71. The separately deployed site uses its reviewed November16 source; old731all-identical claims remain historical. Native backend/mobile inputs remain frozen71. External installed tools are outside repository source checks.','gitMutations':0,'nativeOrNetworkActions':0}

if __name__=='__main__':
    root=Path(subprocess.check_output(['git','rev-parse','--show-toplevel'],text=True).strip())
    print(json.dumps(check(root),ensure_ascii=False,indent=2))
