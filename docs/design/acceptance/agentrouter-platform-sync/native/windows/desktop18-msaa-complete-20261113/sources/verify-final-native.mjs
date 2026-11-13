import {readFileSync,writeFileSync,existsSync,renameSync} from 'node:fs';
import {join} from 'node:path';
import {createHash,randomBytes} from 'node:crypto';
import assert from 'node:assert/strict';

const resultDirectory=process.argv[2];
let stage='configuration',exitCode=1,browser;
const checks=[],writes=[],pageErrors=[],windows={},screenshotAcknowledgements=[];
function requireMonitorDtoProvider(agent,provider){assert.ok(agent);assert.equal(agent.provider,provider);}
let uiFontProof;
const knownTypes=new Set(['Error','TypeError','SyntaxError','ReferenceError','RangeError','URIError','EvalError','AssertionError','TimeoutError']);
const errorType=error=>knownTypes.has(error?.name)?error.name:'Error';
const readJson=file=>JSON.parse(readFileSync(file,'utf8').replace(/^\uFEFF/,''));
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
function writeJson(file,value){const temporary=file+'.'+randomBytes(8).toString('hex')+'.tmp';writeFileSync(temporary,JSON.stringify(value,null,2)+'\n',{flag:'wx'});renameSync(temporary,file);}
let requestNumber=0,configuration,fixtureId;
async function nativeAction(action){
 const id=String(++requestNumber).padStart(4,'0');
 const responseFile=join(resultDirectory,'window-response-'+id+'.json');
 writeJson(join(resultDirectory,'window-request-'+id+'.json'),{id,action});
 const deadline=Date.now()+20000;
 while(!existsSync(responseFile)){assert.ok(Date.now()<deadline,'native worker response timed out');await delay(50);}
 const response=readJson(responseFile);assert.equal(response.id,id);assert.equal(response.status,'ok');return response.value;
}
const resize=async()=>assert.ok(await nativeAction('resize')>0);
const snapshot=async()=>{const rows=await nativeAction('snapshot');assert.ok(Array.isArray(rows));return rows.filter(row=>row.Title==='Orchester');};
async function screenshotAndWaitForAck(page,name,index){
 const screenshot=join(resultDirectory,name);
 await page.screenshot({path:screenshot});
 // Publish immediately after screenshot completion. No following UI action
 // executes until root reviews this PNG and writes its matching ACK.
 const sha256=createHash('sha256').update(readFileSync(screenshot)).digest('hex');
 const id=randomBytes(16).toString('hex'),nonce=randomBytes(24).toString('hex');
 const ackFile=join(resultDirectory,'screenshot-ack-'+id+'.json');
 const ready={event:'screenshot_ready',index,id,nonce,name,sha256,ackFile,sourceRevision:configuration.sourceRevision};
 writeJson(join(resultDirectory,'screenshot-ready.json'),ready);
 writeJson(join(resultDirectory,'screenshot-ready-'+index+'.json'),ready);
 stage='screenshot-'+index+'-awaiting-ack';
 const deadline=Date.now()+configuration.ackTimeoutSeconds*1000;
 while(!existsSync(ackFile)){assert.ok(Date.now()<deadline,'screenshot ACK timed out');await delay(100);}
 const ack=readJson(ackFile);
 assert.equal(ack.event,'screenshot_reviewed');assert.equal(ack.id,id);assert.equal(ack.nonce,nonce);assert.equal(ack.name,name);assert.equal(ack.sha256,sha256);assert.equal(ack.accepted,true);
 screenshotAcknowledgements.push({index,id,name,sha256,accepted:true});
}
try{
 assert.ok(resultDirectory);configuration=readJson(join(resultDirectory,'launch-result.json'));
 assert.equal(configuration.cdpPort,19322);assert.equal(configuration.modelCalls,0);
 stage='playwright-import';
 const {chromium}=await import('file://wsl.localhost/Ubuntu-22.04/home/han/.local/share/codex-dev-tools/node/node_modules/playwright/index.mjs');
 stage='native-cdp-connect';browser=await chromium.connectOverCDP('http://127.0.0.1:19322');
 const context=browser.contexts()[0];assert.ok(context);assert.equal(browser.contexts().length,1);assert.equal(context.pages().length,1);
 const main=context.pages()[0];main.setDefaultTimeout(20000);main.setDefaultNavigationTimeout(20000);
 await main.waitForURL(url=>url.protocol==='http:'&&url.hostname==='127.0.0.1'&&url.port!=='19322');
 const embeddedPort=Number(new URL(main.url()).port);assert.ok(Number.isInteger(embeddedPort)&&embeddedPort>0&&embeddedPort<65536);
 writeJson(join(resultDirectory,'runtime-listener.json'),{embeddedPort,cdpPort:19322});
 const capturePageError=error=>pageErrors.push({type:errorType(error)});
 context.on('page',page=>{page.setDefaultTimeout(20000);page.setDefaultNavigationTimeout(20000);page.on('pageerror',capturePageError);});main.on('pageerror',capturePageError);
 const cookie=async()=>{const session=(await context.cookies()).find(value=>value.name==='orchester_session');assert.ok(session);assert.ok(session.httpOnly);assert.equal(session.sameSite,'Lax');return session.value;};
 const invoke=(page,command,args={})=>page.evaluate(async({command,args})=>{try{return {ok:true,value:await window.__TAURI_INTERNALS__.invoke(command,args)}}catch(error){return {ok:false,error:String(error)}}},{command,args});
 const library=page=>page.evaluate(async()=>(await(await fetch('/api/v1/library')).json()));
 const put=(page,token,state)=>page.evaluate(async({token,state})=>{const response=await fetch('/api/v1/library',{method:'PUT',headers:{'content-type':'application/json','x-csrf-token':token},body:JSON.stringify(state)});return {status:response.status,body:await response.json()};},{token,state});
 async function reloadWithSession(page,url){const response=page.waitForResponse(reply=>new URL(reply.url()).pathname==='/api/v1/session'&&reply.status()===200);if(url)await page.goto(url);else await page.reload();const session=await(await response).json();await page.locator('[data-window-action="close"]').waitFor();return session;}
 async function renameUI(page,title,label){
  const menu=page.locator('[data-session-menu="'+fixtureId+'"]:visible').last();
  if(!await menu.isVisible())await page.locator('[data-rail-toggle]').click();
  await menu.waitFor();await page.locator('[data-session-row="'+fixtureId+'"]:visible').last().hover();await menu.click();await page.getByRole('menuitem',{name:/^重命名/}).click();
  const dialog=page.locator('[role="dialog"]:has(#library-name)');await dialog.locator('#library-name').waitFor();await dialog.locator('#library-name').fill(title);
  const saved=page.waitForResponse(reply=>new URL(reply.url()).pathname==='/api/v1/library'&&reply.request().method()==='PUT'&&reply.status()===200);await dialog.locator('button[type="submit"]').click();const response=await saved;const state=await response.json();await dialog.waitFor({state:'hidden'});
  const closer=page.locator('[data-drawer-close]:visible');if(await closer.count())await closer.last().click();
  await page.locator('[data-thread-title]').filter({hasText:title}).waitFor();writes.push({label,status:response.status(),revision:state.revision});return state;
 }
 stage='fixture-and-main-save';await resize();
 assert.equal(await main.evaluate(async()=>await(await fetch('/')).text()),readFileSync(join(configuration.packageDirectory,'web','index.html'),'utf8'));
 const seed=await main.evaluate(async()=>{const items=(await(await fetch('/api/v1/sessions')).json()).items;return items.find(value=>value.title==='Native smoke saved question');});assert.ok(seed);fixtureId=seed.id;
 const linked=new URL('/workspace?session='+seed.id,main.url()).href;
 const mainSession=await reloadWithSession(main,linked),initialCookie=await cookie();
 await main.getByText('Native smoke saved answer',{exact:true}).waitFor();
 await renameUI(main,'Native main before child','main UI before child');checks.push('saved fixture transcript and main UI save succeeded');
 stage='main-hwnd';windows.main=await snapshot();assert.equal(windows.main.length,1);assert.equal(windows.main[0].Visible,true);assert.equal(windows.main[0].MaximizeHit,9);checks.push('main HWND shown with native maximize hit testing');
 stage='invalid-conversation';const invalid=await invoke(main,'open_conversation_window',{id:'s-../settings'});assert.equal(invalid.ok,false);assert.match(invalid.error,/Invalid conversation identifier/);assert.equal(context.pages().length,1);checks.push('invalid conversation identifier rejected without creating window');
 stage='secondary-bootstrap';const opened=context.waitForEvent('page'),openedResponse=await invoke(main,'open_conversation_window',{id:seed.id});assert.equal(openedResponse.ok,true);const child=await opened;await resize();
 await child.waitForURL('**/workspace?session='+seed.id);await child.getByText('Native smoke saved answer',{exact:true}).waitFor();await child.locator('[data-thread-title]').filter({hasText:'Native main before child'}).waitFor();
 const childSession=await child.evaluate(async()=>(await(await fetch('/api/v1/session')).json()));
 assert.equal(await cookie(),initialCookie);assert.equal(childSession.csrf_token,mainSession.csrf_token);assert.equal(childSession.expires_at,mainSession.expires_at);checks.push('secondary bootstrap retains shared HttpOnly Cookie, CSRF token and expiry');
 windows.secondary=await snapshot();assert.equal(windows.secondary.length,2);assert.ok(windows.secondary.every(value=>value.Visible&&value.MaximizeHit===9));checks.push('actual secondary HWND shown with native chrome, renamed title and authenticated saved transcript');
 stage='main-original-csrf';let state=await library(main);state.sections=state.sections.filter(value=>value.id!=='native-api-section');state.sections.push({id:'native-api-section',name:'Native API section'});let saved=await put(main,mainSession.csrf_token,state);assert.equal(saved.status,200);writes.push({label:'main original CSRF after child bootstrap',status:saved.status,revision:saved.body.revision});checks.push('original main-window CSRF permits PUT 200 after secondary bootstrap');
 stage='two-window-ui-saves';await renameUI(child,'Native child UI saved','child UI after main API save');await renameUI(main,'Native main UI saved','main UI after child UI save');checks.push('main and secondary UI saves both succeed with revision conflict recovery');
 stage='secondary-original-csrf';state=await library(child);state.sessions.find(value=>value.id===seed.id).pinned=true;saved=await put(child,childSession.csrf_token,state);assert.equal(saved.status,200);writes.push({label:'child original CSRF after main UI save',status:saved.status,revision:saved.body.revision});assert.equal(await cookie(),initialCookie);checks.push('secondary original CSRF permits PUT 200 after main changes');
 stage='secondary-window-permissions';const childLabel=await child.evaluate(()=>window.__TAURI_INTERNALS__.metadata.currentWindow.label),maximized=await invoke(child,'plugin:window|is_maximized',{label:childLabel});assert.equal(maximized.ok,true);assert.equal(typeof maximized.value,'boolean');const denied=await invoke(child,'plugin:window|hide',{label:childLabel});assert.equal(denied.ok,false);assert.match(denied.error,/not allowed|not permitted|denied/i);checks.push('secondary receives allowed window controls and ungranted hide remains denied');
 stage='nested-window';const nestedOpened=context.waitForEvent('page'),nestedReply=await invoke(child,'open_conversation_window',{id:seed.id});assert.equal(nestedReply.ok,true);const nested=await nestedOpened;await resize();await nested.waitForURL('**/workspace?session='+seed.id);await nested.getByText('Native smoke saved answer',{exact:true}).waitFor();
 assert.equal(await cookie(),initialCookie);windows.nested=await snapshot();assert.equal(windows.nested.length,3);assert.ok(windows.nested.every(value=>value.Visible&&value.MaximizeHit===9));state=await library(main);state.sessions.find(value=>value.id===seed.id).pinned=false;saved=await put(main,mainSession.csrf_token,state);assert.equal(saved.status,200);writes.push({label:'main original CSRF after third window bootstrap',status:saved.status,revision:saved.body.revision});checks.push('secondary caller opens another authorized native window and original main CSRF stays valid');
 stage='navigation-policy';const childUrl=child.url();await child.evaluate(()=>location.assign('http://127.0.0.1:1/'));await delay(300);assert.equal(child.url(),childUrl);checks.push('native navigation rejects a different loopback origin');
 stage='popup-policy';await child.evaluate(()=>window.open('http://127.0.0.1:1/','_blank'));await delay(300);assert.equal(context.pages().length,3);checks.push('native popup policy denies browser-created windows');
 const monitorIds=['codex','claude','deepseek','opencode','gemini','antigravity','cursor','workbuddy','workbuddy_global','qwen','openclaw','cline','roo','kilo','hermes','pi','crush','claude_app','trae','zcode','craft'];
 const serviceIds=new Set(['openclaw','workbuddy','workbuddy_global','claude_app','trae','zcode','craft']);
 async function verifyMonitorRoster(page){
  const disclosure=page.locator('[data-rail-disclosure="agents"]:visible');if(await disclosure.getAttribute('aria-expanded')==='false')await disclosure.click();
  const rowSelector='[data-rail-section="fleet"] [data-agent-fleet] > div > .agent-fleet__rows > [data-agent-id]';
  const rows=page.locator(rowSelector+':visible');await rows.first().waitFor();
  await page.waitForFunction(({selector,ids})=>{
   const rows=[...document.querySelectorAll(selector)].filter(e=>e.getClientRects().length>0);
   return ids.every(id=>rows.some(row=>row.querySelector('[data-agent-provider]')?.getAttribute('data-agent-provider')===id))&&rows.every(row=>{
    const provider=row.querySelector('[data-agent-provider]')?.getAttribute('data-agent-provider');if(!ids.includes(provider))return true;
    const holder=row.querySelector('[data-agent-icon="'+provider+'"]');if(!holder)return false;
    const images=holder.matches('img')?[holder]:[...holder.querySelectorAll('img')];
    return images.length===2&&images.every(image=>image.complete&&image.naturalWidth>0)&&images.filter(image=>getComputedStyle(image).display!=='none').length===1;
   });
  },{selector:rowSelector,ids:monitorIds});
  const themeButton=page.locator('[data-workspace-navigation] button[aria-label="主题"]:visible');assert.equal(await themeButton.count(),1);
  const initialTheme=await page.evaluate(()=>document.documentElement.getAttribute('data-theme'));
  assert.ok(initialTheme==='dark'||initialTheme==='light');
  await themeButton.click();await page.waitForFunction(initial=>document.documentElement.getAttribute('data-theme')!==initial,initialTheme);
  const changedTheme=await page.evaluate(()=>document.documentElement.getAttribute('data-theme'));assert.equal(changedTheme,initialTheme==='dark'?'light':'dark');
  await page.waitForFunction(({selector,ids})=>[...document.querySelectorAll(selector)].filter(e=>e.getClientRects().length>0).every(row=>{
   const id=row.querySelector('[data-agent-provider]')?.getAttribute('data-agent-provider');if(!ids.includes(id))return true;
   const images=[...row.querySelectorAll('[data-agent-icon="'+id+'"] img')];return images.length===2&&images.every(image=>image.complete&&image.naturalWidth>0)&&images.filter(image=>getComputedStyle(image).display!=='none').length===1;
  }),{selector:rowSelector,ids:monitorIds});
  await themeButton.click();await page.waitForFunction(initial=>document.documentElement.getAttribute('data-theme')===initial,initialTheme);
  const themeSwitchObserved=new Set([initialTheme,changedTheme]).size===2;
  const fleet=await page.evaluate(async()=>{const response=await fetch('/api/v1/agents/status');if(response.status!==200)throw new Error('monitor-status-read-failed');return response.json()});assert.ok(Array.isArray(fleet.agents));
  const proof=[],units=[],shared=[];
  for(const id of monitorIds){
   const matching=rows.filter({has:page.locator('[data-agent-provider="'+id+'"]')});const instanceCount=await matching.count();assert.ok(instanceCount>=1);const unitInstances=[];
   for(let n=0;n<instanceCount;n++){
    const row=matching.nth(n),agentId=await row.getAttribute('data-agent-id'),agent=fleet.agents.find(value=>value.agent_id===agentId);requireMonitorDtoProvider(agent,id);
    const imageEvidence=await row.locator('[data-agent-icon="'+id+'"] img').evaluateAll(images=>({count:images.length,loaded:images.every(image=>image.complete&&image.naturalWidth>0),visible:images.filter(image=>getComputedStyle(image).display!=='none').length}));assert.equal(imageEvidence.count,2);assert.equal(imageEvidence.loaded,true);assert.equal(imageEvidence.visible,1);
    const external=agent.window_count_source==='external_processes',unit=external?(agent.capabilities.includes('service_presence')?'services':'processes'):'windows';
    if(external)assert.equal(unit,serviceIds.has(id)?'services':'processes');
    const aria=await row.locator('button').getAttribute('aria-label');assert.ok(aria.includes(unit==='services'?'服务进程':unit==='processes'?'进程':'窗口'));
    unitInstances.push({agentId,provider:agent.provider,unit,externalSource:external,actualDtoAndNativeUiAgree:true,loaded:imageEvidence.loaded,themeImagesLoaded:imageEvidence.count,darkAndLightSwitchObserved:themeSwitchObserved});
    if(serviceIds.has(id)||['cline','roo','kilo'].includes(id)){
     const displayName=(await row.locator('strong').first().innerText()).trim();await row.locator('button').click();
     const details=page.locator('[data-agent-details]:visible');await details.locator('[data-agent-icon="'+id+'"]').waitFor();
     await page.waitForFunction(({id,name})=>{const details=[...document.querySelectorAll('[data-agent-details]')].find(e=>e.getClientRects().length>0);return details?.querySelector('[data-agent-icon="'+id+'"]')&&details.querySelector('[data-agent-details-name]')?.textContent.trim()===name},{id,name:displayName});
     const metric=details.locator('[data-agent-count-unit="'+unit+'"]');assert.equal(await metric.count(),1);if(unit==='services'){assert.equal(await metric.locator('[data-active-services]').count(),1);assert.equal((await metric.locator('dt').innerText()).trim(),'服务进程');}
     if(['cline','roo','kilo'].includes(id)){
      const source=details.locator('[data-agent-process-sources] [data-process-source-status="unavailable"]');await source.first().waitFor();assert.ok(agent.process_sources.some(value=>value.status==='unavailable'&&value.count===null));
      await page.waitForFunction(()=>{const details=[...document.querySelectorAll('[data-agent-details]')].find(e=>e.getClientRects().length>0);const count=details?.querySelector('[data-active-processes]')?.textContent.trim();const activity=details?.querySelector('[data-agent-details-activity]')?.textContent.trim();return count==='—'?activity==='状态未知':/^至少 [1-9][0-9]*$/.test(count??'')&&activity==='服务运行'});
      const count=(await metric.locator('[data-active-processes]').innerText()).trim();if(n===0)shared.push({id,unavailableSourceVisible:true,countKind:count==='—'?'unknown':'at-least-positive'});
     }
    }
   }
   proof.push({id,officialSourceBoundByBuildInputs:true,loaded:unitInstances.every(instance=>instance.loaded),themeImagesLoaded:unitInstances[0].themeImagesLoaded,darkAndLightSwitchObserved:themeSwitchObserved,instanceCount});units.push({id,unit:serviceIds.has(id)?'services':'processes',instanceCount,instances:unitInstances});
  }
  const panel=page.locator('[data-thread-action="panel"]:visible');if(await panel.count())await panel.click();
  checks.push('all twenty-one canonical provider identities and every corresponding official image load in the native WebView');
  checks.push('actual DTO and native service units agree and shared editor sources remain unavailable with unknown or lower-bound counts');
  return {roster:proof,units,sharedEditors:shared};
 }
 let monitorProof;
 stage='secondary-reload';const reloaded=await reloadWithSession(child);assert.equal(reloaded.csrf_token,mainSession.csrf_token);assert.equal(await cookie(),initialCookie);await child.locator('[data-thread-title]').filter({hasText:'Native main UI saved'}).waitFor();await child.getByText('Native smoke saved answer',{exact:true}).scrollIntoViewIfNeeded();stage='twenty-monitor-roster-and-truth';monitorProof=await verifyMonitorRoster(child);await screenshotAndWaitForAck(child,'desktop-secondary-history.png',1);checks.push('secondary reload preserves authentication and restores final saved title and answer');
 stage='secondary-close';for(const page of [nested,child]){const closed=page.waitForEvent('close');await page.locator('[data-window-action="close"]').click();await closed;}
 windows.afterSecondaryClose=await snapshot();assert.equal(windows.afterSecondaryClose.length,1);assert.equal(context.pages().length,1);checks.push('closing secondary windows destroys their HWNDs and WebViews');
 stage='main-save-after-close';await renameUI(main,'Native main saved after close','main UI after secondary windows close');assert.equal(await cookie(),initialCookie);await main.getByText('Native smoke saved answer',{exact:true}).scrollIntoViewIfNeeded();await screenshotAndWaitForAck(main,'desktop-main-after-secondary-close.png',2);checks.push('main UI remains writable after both secondary windows close');
 stage='main-close-hide';const health=await main.evaluate(async()=>(await fetch('/api/v1/health')).status);assert.equal(health,200);await main.locator('[data-window-action="close"]').click();await delay(200);windows.mainHidden=await snapshot();assert.equal(windows.mainHidden.length,1);assert.equal(windows.mainHidden[0].Visible,false);assert.equal(context.pages().length,1);const hiddenHealth=await main.evaluate(async()=>(await fetch('/api/v1/health')).status);assert.equal(hiddenHealth,200);checks.push('main close hides its HWND while WebView and embedded server remain healthy');
 stage='ui-font-proof';uiFontProof=await main.evaluate(async()=>{
  await document.fonts.ready;
  const body=getComputedStyle(document.body),titleElement=document.querySelector('[data-thread-title]'),title=titleElement?getComputedStyle(titleElement):null;
  const canvas=document.createElement('canvas'),context=canvas.getContext('2d');context.font='400 16px '+body.fontFamily;
  return {body:{fontFamily:body.fontFamily,fontWeight:body.fontWeight},threadTitle:title?{fontFamily:title.fontFamily,fontWeight:title.fontWeight}:null,lxgwFaces:[...document.fonts].filter(face=>face.family.replace(/["']/g,'')==='LXGW WenKai').map(face=>({family:face.family,status:face.status})),measurement:{text:'字体一致 Font 123',fontSizePx:16,fontWeight:400,fontFamily:body.fontFamily,width:context.measureText('字体一致 Font 123').width}};
 });uiFontProof.assetPins=configuration.uiFontAssetPins;
 stage='result';assert.equal(checks.length,18);assert.equal(writes.length,7);assert.ok(writes.every((value,index)=>value.status===200&&Number.isInteger(value.revision)&&(index===0||value.revision>writes[index-1].revision)));assert.deepEqual(pageErrors,[]);assert.equal(screenshotAcknowledgements.length,2);
 writeJson(join(resultDirectory,'native-windows-result.json'),{passed:true,sourceRevision:configuration.sourceRevision,expectedPlanSha256:configuration.expectedPlanSha256,expectedFinalResultSha256:configuration.expectedFinalResultSha256,binarySha256:configuration.binarySha256,checks,writes,authentication:{cookieStable:true,csrfStable:true,expiryStable:true,httpOnly:true,sameSite:'Lax',actualMainPutAfterSecondary:200},windows,health:{afterSecondaryClose:health,afterMainHide:hiddenHealth},pageErrors,uiFontProof,monitorProof,scope:{isolatedExecutable:true,isolatedOrchesterHome:true,isolatedWebViewProfile:true,savedDelegateFixture:true,installedUserApplicationTouched:false,modelRunsRequested:false},screenshotAcknowledgements});exitCode=0;
}catch(error){
 if(resultDirectory&&existsSync(resultDirectory)){try{writeJson(join(resultDirectory,'native-final-failure.json'),{status:'failed',stage,type:errorType(error),completedChecks:checks.length,completedWrites:writes.length,pageErrorCount:pageErrors.length,modelCalls:0});}catch{}}
 // No raw errors, URLs, Cookie values, CSRF tokens, or stacks reach stdout.
}finally{
 // Exiting our own Node process disconnects CDP without signalling Browser.close.
 // The owning PowerShell worker performs native process cleanup by original HANDLE.
 process.exit(exitCode);
}
