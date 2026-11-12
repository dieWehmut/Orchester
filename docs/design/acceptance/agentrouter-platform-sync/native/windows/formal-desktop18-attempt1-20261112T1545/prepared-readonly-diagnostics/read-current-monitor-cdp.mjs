import {readFileSync,writeFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
const launchFile=process.argv[2],outputFile=process.argv[3];
let result={status:'failed',rawErrorsIncluded:false},exitCode=1;
try{
 const launchRaw=readFileSync(launchFile,'utf8').replace(/^\uFEFF/,'');const launch=JSON.parse(launchRaw);const createdExact=launchRaw.match(/\"Created\"\s*:\s*([0-9]+)/)?.[1];
 if(launch.ProcessId!==37324||createdExact!=='134389718070239855'||launch.binarySha256!=='e4aa765befa85c7fb053075844be5693685c00677dd62b4cfff4592ec68c1d05'||launch.cdpPort!==19322)throw new Error('original-launch-binding-differs');
 const response=await fetch('http://127.0.0.1:19322/json/list',{signal:AbortSignal.timeout(5000)});
 if(!response.ok)throw new Error('cdp-query-failed');const targets=await response.json();
 const views=[];
 for(const target of targets.filter(t=>t.type==='page')){
  const url=new URL(target.url);if(url.hostname!=='127.0.0.1'||url.protocol!=='http:')throw new Error('unexpected-target-origin');
  const endpoint=new URL(target.webSocketDebuggerUrl);if(endpoint.hostname!=='127.0.0.1'||endpoint.port!=='19322')throw new Error('unexpected-debugger-endpoint');
  const socket=new WebSocket(endpoint);await new Promise((ok,no)=>{const timeout=setTimeout(()=>{socket.close();no(new Error('cdp-connect-timeout'));},5000);socket.addEventListener('open',()=>{clearTimeout(timeout);ok();},{once:true});socket.addEventListener('error',()=>{clearTimeout(timeout);no(new Error('cdp-connect-failed'));},{once:true});});
  try{
   const expression=`(async()=>{const selector='[data-rail-section="fleet"] [data-agent-fleet] > div > .agent-fleet__rows > [data-agent-id]';const r=await fetch('/api/v1/agents/status');const d=r.status===200?await r.json():{};return {title:document.title,loopbackOrigin:true,path:location.pathname,theme:document.documentElement.getAttribute('data-theme'),visibleThemeButtons:[...document.querySelectorAll('[data-rail-action="theme"]')].filter(x=>x.getClientRects().length>0).length,rosterStatus:r.status,rows:[...document.querySelectorAll(selector)].filter(x=>x.getClientRects().length>0).map(x=>({provider:x.querySelector('[data-agent-provider]')?.getAttribute('data-agent-provider'),images:[...x.querySelectorAll('img')].map(i=>({loaded:i.complete&&i.naturalWidth>0,visible:getComputedStyle(i).display!=='none'}))})),safeDto:(d.agents??[]).map(a=>({provider:a.provider,count:a.window_count,source:a.window_count_source,capabilities:a.capabilities,processSources:(a.process_sources??[]).map(s=>({status:s.status,count:s.count}))}))};})()`;
   const value=await new Promise((ok,no)=>{const timeout=setTimeout(()=>no(new Error('cdp-response-timeout')),10000);socket.addEventListener('message',event=>{const data=JSON.parse(event.data);if(data.id===1){clearTimeout(timeout);if(data.error||data.result?.exceptionDetails)no(new Error('cdp-evaluation-failed'));else ok(data.result.result.value);}});socket.send(JSON.stringify({id:1,method:'Runtime.evaluate',params:{expression,awaitPromise:true,returnByValue:true}}));});
   views.push(value);
  }finally{socket.close();}
 }
 result={status:'original-owned-desktop-live-monitor-readonly-diagnostic-completed',observedAtUtc:new Date().toISOString(),expectedGuiPid:37324,expectedGuiCreatedExact:'134389718070239855',sourceRevision:launch.sourceRevision,views,domMutationCount:0,modelCalls:0,processOrInputSignals:0,rawCookiesCsrfQueryHostnamesSessionNamesOrUrlsIncluded:false};exitCode=0;
}catch(error){result={status:'readonly-diagnostic-failed',errorType:error.name,rawErrorsIncluded:false};}
result.sourceSha256=createHash('sha256').update(readFileSync(process.argv[1])).digest('hex');writeFileSync(outputFile,JSON.stringify(result,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify(result));process.exit(exitCode);
