import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
const base='/home/han/.local/state/orchester-delivery/20261113/linux-original-one-wire';
const identity=JSON.parse(readFileSync(`${base}/current-native-identity-safe.json`,'utf8'));
const target=(await (await fetch(`http://127.0.0.1:${identity.ownForwardPort}/json/list`,{signal:AbortSignal.timeout(5000)})).json()).find(x=>x.type==='page' && new URL(x.url).hostname==='localhost');
if(!target) throw new Error('product-page-not-found');
const endpoint=new URL(target.webSocketDebuggerUrl);
if(endpoint.hostname!=='127.0.0.1' || Number(endpoint.port)!==identity.ownForwardPort) throw new Error('unexpected-debugger');
const socket=new WebSocket(endpoint);
try {
 await new Promise((resolve,reject)=>{const t=setTimeout(()=>reject(new Error('cdp-open-timeout')),5000);socket.addEventListener('open',()=>{clearTimeout(t);resolve();},{once:true});});
 const value=await new Promise((resolve,reject)=>{
  const t=setTimeout(()=>reject(new Error('cdp-response-timeout')),10000);
  socket.addEventListener('message',event=>{const d=JSON.parse(event.data);if(d.id!==1)return;clearTimeout(t);if(d.error||d.result?.exceptionDetails)reject(new Error('exit-expression-failed'));else resolve(d.result.result.value);});
  socket.send(JSON.stringify({id:1,method:'Runtime.evaluate',params:{expression:"(() => { const available=typeof window.Capacitor?.Plugins?.App?.exitApp === 'function'; if(available) Capacitor.Plugins.App.exitApp(); return {normalProductExitRequested:available,forcedStopCalls:0}; })()",returnByValue:true}}));
 });
 const result={status:'normal-product-exit-requested',observedAtUtc:new Date().toISOString(),targetHost:'Linux',targetAppPid:identity.launchedAppPid,...value,appProcessDestructionClaimed:false,sourceSha256:createHash('sha256').update(readFileSync(process.argv[1])).digest('hex')};
 writeFileSync(`${base}/normal-product-exit-request-safe.json`,JSON.stringify(result,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify(result));
 if(!value.normalProductExitRequested)process.exitCode=1;
} finally {socket.close();}
