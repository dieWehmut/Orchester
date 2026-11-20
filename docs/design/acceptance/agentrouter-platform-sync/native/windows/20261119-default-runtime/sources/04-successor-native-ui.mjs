import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
const [resultDirectory,prep,nativeGateFile,nativeGateSha]=process.argv.slice(2);
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
const readJson=f=>JSON.parse(fs.readFileSync(f,'utf8').replace(/^\uFEFF/,''));
const ids=['codex','claude','deepseek','opencode','gemini','antigravity','cursor','workbuddy','workbuddy_global','qwen','openclaw','cline','roo','kilo','hermes','pi','crush','claude_app','trae','zcode','craft'];
let browser,main,disclosure,restoreDisclosure=false,code=1,stage='native-gate',observedApiPutCount=0,observedNonReadApiRequestCount=0;
const result={schemaVersion:1,status:'failed',methodScope:'GET and restored sidebar disclosure only',dataPutCount:0,modelCalls:0,loginOrPairingActions:0,privateValuesIncluded:false};
try{
 assert.equal(process.platform,'win32');assert.ok(resultDirectory&&prep&&nativeGateFile&&/^[a-f0-9]{64}$/.test(nativeGateSha));
 assert.equal(hash(fs.readFileSync(nativeGateFile)),nativeGateSha);const gate=readJson(nativeGateFile);
 assert.equal(gate.currentNativeGuiAndCdpOwnerVerified,true);assert.equal(gate.originalHandleHeldThroughoutNode,true);
 const gateAge=Date.now()-Date.parse(gate.observedAtUtc);assert.ok(Number.isFinite(gateAge)&&gateAge>=0&&gateAge<120000);
 const cfg=readJson(path.join(resultDirectory,'launch-result.json'));assert.equal(cfg.binarySha256,'e4aa765befa85c7fb053075844be5693685c00677dd62b4cfff4592ec68c1d05');assert.equal(cfg.sourceRevision,'71def643cb08b208cc79687f79255c504eb14dc6');assert.equal(cfg.defaultUserProfileUsed,true);assert.equal(cfg.ProcessId,gate.guiPid);assert.equal(cfg.cdpPort,gate.cdpPort);
 const backupSafe=readJson(path.join(prep,'fresh-offline-backup-safe.json'));assert.equal(hash(fs.readFileSync(path.join(prep,'fresh-offline-backup-private.json'))),backupSafe.privateManifestSha256);const manifest=readJson(path.join(prep,'fresh-offline-backup-private.json'));
 const state=manifest.roots.find(r=>r.name==='orchester-user-state');assert.ok(state&&state.exists);const originalBytes=fs.readFileSync(path.join(state.backup,'sessions.jsonl'));assert.equal(hash(originalBytes),state.inventory.find(r=>r.path==='sessions.jsonl').sha256);const originals=originalBytes.toString('utf8').split(/\r?\n/).filter(x=>x.trim()).map(x=>JSON.parse(x));
 stage='native-cdp';const {chromium}=await import('file://wsl.localhost/Ubuntu-22.04/home/han/.local/share/codex-dev-tools/node/node_modules/playwright/index.mjs');browser=await chromium.connectOverCDP('http://127.0.0.1:'+cfg.cdpPort);assert.equal(browser.contexts().length,1);const context=browser.contexts()[0];context.on('request',request=>{const method=request.method();let url;try{url=new URL(request.url())}catch{return}if(url.pathname.startsWith('/api/')&&!['GET','HEAD'].includes(method)){observedNonReadApiRequestCount++;if(method==='PUT')observedApiPutCount++;}});assert.equal(context.pages().length,1);main=context.pages()[0];main.setDefaultTimeout(60000);main.setDefaultNavigationTimeout(60000);
 await main.waitForURL(u=>u.protocol==='http:'&&u.hostname==='127.0.0.1'&&u.port!==String(cfg.cdpPort));
 const origin=new URL(main.url()).origin;result.embeddedPort=Number(new URL(main.url()).port);
 if(new URL(main.url()).pathname!=='/workspace'){await main.goto(origin+'/workspace');}
 const cookie=(await context.cookies()).find(c=>c.name==='orchester_session');assert.ok(cookie&&cookie.httpOnly&&cookie.sameSite==='Lax');result.httpOnlyLaxCookieObserved=true;
 stage='authenticated-readonly-apis';
 const data=await main.evaluate(async()=>{const urls=['/api/v1/session','/api/v1/library','/api/v1/agents/status'];const out=[];for(const url of urls){const r=await fetch(url,{credentials:'same-origin'});if(r.status!==200)throw new Error('readonly-api-status');out.push(await r.json());}return out;});
 assert.equal(typeof data[0].csrf_token,'string');assert.ok(data[0].csrf_token.length>0);assert.ok(Array.isArray(data[1].sessions)&&Array.isArray(data[2].agents));assert.ok(data[2].agents.every(a=>ids.includes(a.provider)));result.authenticatedApiGetStatuses=[200,200,200];result.currentProviderDtoCount=data[2].agents.length;
 stage='original-saved-session-api-retention';
 const details=await main.evaluate(async()=>{let cursor=null;const entries=[];for(let page=0;page<100;page++){const u='/api/v1/sessions?limit=100'+(cursor?'&cursor='+encodeURIComponent(cursor):'');const r=await fetch(u,{credentials:'same-origin'});if(r.status!==200)throw new Error('history-page-status');const p=await r.json();if(!Array.isArray(p.items))throw new Error('history-page-shape');entries.push(...p.items);if(!p.next_cursor)break;cursor=p.next_cursor;if(page===99)throw new Error('history-pagination-limit');}const out=[];for(const e of entries.filter(e=>e.source==='delegate')){const r=await fetch('/api/v1/sessions/'+encodeURIComponent(e.id),{credentials:'same-origin'});if(r.status!==200)throw new Error('history-detail-status');out.push(await r.json());}return out;});
 const usage=v=>['input_tokens','output_tokens','cached_input_tokens','reasoning_output_tokens'].map(k=>v?.[k]??0);
 const encode=v=>JSON.stringify([v.recorded_at_unix,v.agent,v.model??null,v.prompt,v.final_text,v.outcome,usage(v.usage)]);const counts=new Map();for(const v of details){const k=encode(v);counts.set(k,(counts.get(k)||0)+1);}for(const v of originals){const k=encode(v);assert.ok((counts.get(k)||0)>0);counts.set(k,counts.get(k)-1);}
 result.originalSavedRecordCount=originals.length;result.originalSavedRecordApiRetainedCount=originals.length;result.allOriginalSavedRecordApiProjectionFieldsRetained=true;
 stage='rendered-provider-and-official-image-mappings';
 disclosure=main.locator('[data-rail-disclosure="agents"]');if(await disclosure.count()===1&&await disclosure.getAttribute('aria-expanded')==='false'){restoreDisclosure=true;await disclosure.click();}
 const selector='[data-rail-section="fleet"] [data-agent-fleet] > div > .agent-fleet__rows > [data-agent-id]';
 await main.waitForFunction(({selector,ids})=>{const rows=[...document.querySelectorAll(selector)].filter(e=>e.getClientRects().length>0);return ids.every(id=>rows.some(row=>row.querySelector('[data-agent-provider]')?.getAttribute('data-agent-provider')===id));},{selector,ids});
 await main.waitForFunction(({selector,ids})=>{const rows=[...document.querySelectorAll(selector)].filter(e=>e.getClientRects().length>0);return ids.every(id=>{const row=rows.find(e=>e.querySelector('[data-agent-provider]')?.getAttribute('data-agent-provider')===id);if(!row)return false;const images=[...row.querySelectorAll('[data-agent-icon=\"'+id+'\"] img')];return images.length===2&&images.every(img=>img.complete&&img.naturalWidth>0);});},{selector,ids});
 const images=await main.evaluate(async({selector,ids})=>{const out=[];for(const id of ids){const row=[...document.querySelectorAll(selector)].find(e=>e.getClientRects().length>0&&e.querySelector('[data-agent-provider]')?.getAttribute('data-agent-provider')===id);const imgs=[...row.querySelectorAll('[data-agent-icon="'+id+'"] img')];if(imgs.length!==2||imgs.some(i=>!i.complete||i.naturalWidth<=0))throw new Error('official-image-not-loaded');for(const img of imgs){const rawUri=img.getAttribute('src');if(!rawUri)throw new Error('official-image-raw-src-absent');const u=new URL(rawUri,location.href);if(u.protocol!=='data:'&&u.origin!==location.origin)throw new Error('official-image-origin');if(u.protocol==='data:'&&!/^data:image\/(svg\+xml|png|webp|jpeg)[;,]/i.test(u.href))throw new Error('official-image-data-type');let b;if(u.protocol==='data:'){const comma=u.href.indexOf(',');if(comma<0)throw new Error('official-image-data-shape');const head=u.href.slice(0,comma),body=u.href.slice(comma+1);b=head.endsWith(';base64')?Uint8Array.from(atob(body),c=>c.charCodeAt(0)):new TextEncoder().encode(decodeURIComponent(body));}else{const r=await fetch(u.href,{credentials:'same-origin'});if(r.status!==200)throw new Error('official-image-http-status');b=await r.arrayBuffer();}const digest=await crypto.subtle.digest('SHA-256',b);out.push({id,scheme:u.protocol,uri:u.protocol==='data:'?rawUri:null,path:u.protocol==='data:'?null:u.pathname,sha256:[...new Uint8Array(digest)].map(v=>v.toString(16).padStart(2,'0')).join('')});}}return out;},{selector,ids});
 assert.equal(images.length,42);const web=path.resolve(cfg.packageDirectory,'web');const corpus=[];const visit=dir=>{for(const e of fs.readdirSync(dir,{withFileTypes:true})){const f=path.join(dir,e.name);if(e.isDirectory())visit(f);else if(/\.(js|css|html)$/.test(e.name))corpus.push(fs.readFileSync(f,'utf8'));}};visit(web);for(const image of images){if(image.scheme==='data:'){assert.ok(corpus.some(text=>text.includes(image.uri)));const comma=image.uri.indexOf(','),head=image.uri.slice(0,comma),body=image.uri.slice(comma+1);const bytes=head.endsWith(';base64')?Buffer.from(body,'base64'):Buffer.from(decodeURIComponent(body),'utf8');assert.equal(hash(bytes),image.sha256);}else{const file=path.resolve(web,'.'+decodeURIComponent(image.path));assert.ok(file.startsWith(web+path.sep));assert.equal(hash(fs.readFileSync(file)),image.sha256);}}
 result.renderedCanonicalProviderCount=21;result.nativeLoadedThemeMappingCount=42;result.exactInstalledImagePayloadMappingCount=42;result.dataUrisDecodedWithoutFetch=true;result.rawAttributeCorpusMembershipVerified=true;result.normalizedAndRawDecodedBytesMatchInstalled=true;result.currentTheme=await main.evaluate(()=>document.documentElement.getAttribute('data-theme'));assert.ok(['dark','light'].includes(result.currentTheme));result.themePreferenceChanged=false;
 stage='installed-html-bytes';const html=await main.evaluate(async()=>{const r=await fetch('/');if(r.status!==200)throw new Error('html-status');return r.text();});assert.equal(html,fs.readFileSync(path.join(web,'index.html'),'utf8'));result.installedHtmlBytesMatch=true;
 result.status='installed-default-native-ui-readonly-apis-and-original-session-projection-verified';code=0;
}catch(error){result.stage=stage;result.exceptionType=['Error','TypeError','SyntaxError','AssertionError','TimeoutError'].includes(error?.name)?error.name:'Error';}
finally{
 if(restoreDisclosure&&disclosure){
  try{
   // A failed click can still take effect. Restore the observed original false
   // state, and avoid opening an already collapsed disclosure during cleanup.
   if(await disclosure.getAttribute('aria-expanded')==='true'){await disclosure.click();}
   await main.waitForFunction(()=>document.querySelector('[data-rail-disclosure="agents"]')?.getAttribute('aria-expanded')==='false');
   result.initialDisclosureStateRestored=true;
  }catch{result.initialDisclosureStateRestored=false;code=1;}
 }
 if(browser){try{await browser.close();}catch{code=1;result.cdpDisconnectFailed=true;}}
}
// Freeze the observed request counts only after disclosure restoration and
// CDP disconnection, so cleanup requests participate in the same zero-write gate.
result.observedApiPutCount=observedApiPutCount;result.observedNonReadApiRequestCount=observedNonReadApiRequestCount;result.requestObservationIncludesDisclosureRestoreAndDisconnect=true;
try{assert.equal(result.observedApiPutCount,0);assert.equal(result.observedNonReadApiRequestCount,0);}catch(error){code=1;result.stage='final-observed-readonly-api-requests';result.exceptionType='AssertionError';}
if(code!==0&&result.status!=='failed')result.status='failed';result.actualNativeNodeExitCode=code;result.observedAtUtc=new Date().toISOString();fs.writeFileSync(path.join(resultDirectory,'default-native-readonly-ui-safe.json'),JSON.stringify(result,null,2)+'\n',{flag:'wx'});process.stdout.write(JSON.stringify(result)+'\n');process.exitCode=code;
