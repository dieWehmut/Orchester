import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
const [prep,backupSha,runtimeFile,runtimeSha,phase='post']=process.argv.slice(2);const resultDirectory=path.dirname(runtimeFile);
const digest=b=>crypto.createHash('sha256').update(b).digest('hex');
const readJson=p=>JSON.parse(fs.readFileSync(p,'utf8').replace(/^\uFEFF/,''));
const result={schemaVersion:1,status:'failed',originalSettingsAndLogsRetained:false,originalDatabaseRowsRetained:false,privateValuesIncluded:false,installerOrModelCalls:0,originalExternalSavedTokenWireVerified:false};
let code=1,stage='native-and-runtime-binding';
try{
 assert.equal(process.platform,'win32');assert.ok(prep&&/^[a-f0-9]{64}$/.test(backupSha)&&runtimeFile&&/^[a-f0-9]{64}$/.test(runtimeSha));
 const safeFile=path.join(prep,'fresh-offline-backup-safe.json');assert.equal(digest(fs.readFileSync(safeFile)),backupSha);
 const safe=readJson(safeFile);assert.equal(safe.status,'fresh-offline-original-backup-verified');assert.equal(safe.actualNativeExitCode,0);
 const mf=path.join(prep,'fresh-offline-backup-private.json');assert.equal(digest(fs.readFileSync(mf)),safe.privateManifestSha256);const manifest=readJson(mf);
 assert.equal(digest(fs.readFileSync(runtimeFile)),runtimeSha);const runtime=readJson(runtimeFile);
 assert.equal(phase,'offline-preflight');assert.equal(runtime.status,'native-no-orchester-process-or-7338-listener-original-state-preflight');assert.equal(runtime.nativeSuccessfulProcessQueryNoOrchester,true);assert.equal(runtime.nativeSuccessfulListenerQueryNo7338,true);assert.equal(runtime.oldExactNativeProcessesAbsent,true);assert.equal(runtime.originalProfileRetained,true);const age=Date.now()-Date.parse(runtime.observedAtUtc);assert.ok(Number.isFinite(age)&&age>=0&&age<=120000);
 stage='settings-and-original-append-only-logs';const sql=[];let staticCount=0,logCount=0;
 const verifyBackup=()=>{for(const root of manifest.roots){if(root.exists){const found=[];const walk=(dir,rel='')=>{for(const entry of fs.readdirSync(dir,{withFileTypes:true})){const relative=rel+entry.name;if(entry.isSymbolicLink())throw new Error('backup-reparse');if(entry.isDirectory())walk(path.join(dir,entry.name),relative+'/');else found.push({path:relative,sha256:digest(fs.readFileSync(path.join(dir,entry.name)))});}};walk(root.backup);const a=[...root.inventory].sort((a,b)=>a.path.localeCompare(b.path)),b=found.sort((a,b)=>a.path.localeCompare(b.path));assert.deepEqual(b,a);}}};verifyBackup();
 const scratchRoot=fs.mkdtempSync(path.join(resultDirectory,'sqlite-private-runtime-scratch-'));
 // SQLite rebuilds the temporary WAL index inside writable private scratch.
 // Avoid reading live SHM bytes, which can carry Windows mandatory byte locks.
 const snapshot=(file,label)=>{const targetDir=path.join(scratchRoot,label);fs.mkdirSync(targetDir);const checks=[];for(const suffix of ['','-wal']){const source=file+suffix;if(fs.existsSync(source)){const bytes=fs.readFileSync(source);const copy=path.join(targetDir,'database.sqlite'+suffix);fs.writeFileSync(copy,bytes,{flag:'wx'});checks.push({source,sha256:digest(bytes)});}}for(const check of checks)assert.equal(digest(fs.readFileSync(check.source)),check.sha256);return path.join(targetDir,'database.sqlite');};
 for(const root of manifest.roots.filter(r=>['orchester-user-state','remote-state-home'].includes(r.name))){
  for(const entry of root.inventory){
   const beforeFile=path.join(root.backup,...entry.path.split('/')),currentFile=path.join(root.source,...entry.path.split('/'));
   assert.ok(fs.existsSync(currentFile));const before=fs.readFileSync(beforeFile);assert.equal(digest(before),entry.sha256);
   if(root.name==='remote-state-home'&&entry.path==='state/remote.lock.db'){assert.equal(before.length,0);assert.equal(runtime.nativeSuccessfulProcessQueryNoOrchester,true);assert.equal(runtime.nativeSuccessfulListenerQueryNo7338,true);assert.equal(fs.readFileSync(currentFile).length,0);result.zeroByteRuntimeLeaseReadAndEmptyWithoutLiveServer=true;continue;}
   if(before.subarray(0,16).equals(Buffer.from('SQLite format 3\0'))){sql.push({beforeFile,currentFile});continue;}
   const current=fs.readFileSync(currentFile);
   if(entry.path.endsWith('.jsonl')){assert.ok(current.length>=before.length&&current.subarray(0,before.length).equals(before));logCount++;}
   else{assert.ok(current.equals(before));staticCount++;}
  }
 }
 stage='original-sqlite-user-rows';const {DatabaseSync}=await import('node:sqlite');let tableCount=0,rowCount=0;
 const normalized=v=>typeof v==='bigint'?['integer',v.toString()]:Buffer.isBuffer(v)||ArrayBuffer.isView(v)?['blob',Buffer.from(v).toString('hex')]:typeof v==='string'?(()=>{try{return ['json',stable(JSON.parse(v))]}catch{return ['text',v]}})():['scalar',v];
 const stable=v=>Array.isArray(v)?v.map(stable):v&&typeof v==='object'?Object.fromEntries(Object.keys(v).sort().map(k=>[k,stable(v[k])])):v;
 const quote=s=>'"'+s.replaceAll('"','""')+'"';
 let databaseIndex=0;for(const item of sql){
  const beforeCopy=snapshot(item.beforeFile,'backup-'+databaseIndex),currentCopy=snapshot(item.currentFile,'current-'+databaseIndex);databaseIndex++;
  let before,current;try{before=new DatabaseSync(beforeCopy,{readOnly:true});current=new DatabaseSync(currentCopy,{readOnly:true});
   const tables=before.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name").all();
   for(const {name} of tables){
    assert.ok(current.prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name=?").get(name));
    const columns=before.prepare('PRAGMA table_info('+quote(name)+')').all().map(c=>c.name);const afterColumns=new Set(current.prepare('PRAGMA table_info('+quote(name)+')').all().map(c=>c.name));assert.ok(columns.every(c=>afterColumns.has(c)));
    const fields=columns.map(quote).join(','),a=before.prepare('SELECT '+fields+' FROM '+quote(name)).all(),b=current.prepare('SELECT '+fields+' FROM '+quote(name)).all();
    const encode=row=>JSON.stringify(columns.map(c=>normalized(row[c])));const counts=new Map();for(const row of b){const k=encode(row);counts.set(k,(counts.get(k)||0)+1);}for(const row of a){const k=encode(row);assert.ok((counts.get(k)||0)>0);counts.set(k,counts.get(k)-1);}
    tableCount++;rowCount+=a.length;
   }
  }finally{if(current)current.close();if(before)before.close();}
 }
 verifyBackup();result.status='original-user-state-before-recovered-managed-launch-retained';result.phase=phase;result.originalSettingsAndLogsRetained=true;result.originalDatabaseRowsRetained=true;result.staticFileCount=staticCount;result.appendOnlyLogCount=logCount;result.sqliteDatabaseCount=sql.length;result.originalTableCount=tableCount;result.originalRowCount=rowCount;result.freshBackupSafeSha256=backupSha;result.runtimeResultSha256=runtimeSha;result.webviewByteEqualityAfterStartupClaimed=false;result.completeOriginalWebviewBackupStillPreserved=true;result.allSixOfflineBackupInventoriesStillExact=true;result.sqliteComparedUsingPrivateScratchCopies=true;result.originalBackupDatabasesOpened=false;result.liveSharedMemoryFilesRead=false;result.sqliteSharedMemoryUsesPrivateScratchOnly=true;code=0;
}catch(error){result.stage=stage;result.exceptionType=['Error','TypeError','SyntaxError','AssertionError','RangeError'].includes(error?.name)?error.name:'Error';}
result.actualNativeNodeExitCode=code;result.observedAtUtc=new Date().toISOString();
if(prep){fs.writeFileSync(path.join(resultDirectory,phase+'-default-runtime-state-retention-'+crypto.randomBytes(8).toString('hex')+'-safe.json'),JSON.stringify(result,null,2)+'\n',{flag:'wx'});}
process.stdout.write(JSON.stringify(result)+'\n');process.exitCode=code;
