import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';

const base = '/home/han/.local/state/orchester-delivery/20261113/linux-original-one-wire';
const identity = JSON.parse(readFileSync(`${base}/current-native-identity-safe.json`, 'utf8'));
const allowedHosts = JSON.parse(readFileSync(`${base}/transport-allowlist-private.json`, 'utf8'));
const phase = process.argv[2] ?? 'metadata';
if (!['metadata', 'wire', 'after'].includes(phase)) throw new Error('unexpected-phase');
let result, socket;
try {
  const response = await fetch(`http://127.0.0.1:${identity.ownForwardPort}/json/list`, { signal: AbortSignal.timeout(5000) });
  if (!response.ok) throw new Error('cdp-list-failed');
  const pages = (await response.json()).filter(page => page.type === 'page');
  if (pages.length !== 1) throw new Error('unexpected-page-count');
  const target = pages[0];
  const url = new URL(target.url);
  if (url.hostname !== 'localhost' || !['http:', 'https:'].includes(url.protocol)) throw new Error('unexpected-product-origin');
  const endpoint = new URL(target.webSocketDebuggerUrl);
  if (endpoint.hostname !== '127.0.0.1' || Number(endpoint.port) !== identity.ownForwardPort) throw new Error('unexpected-debugger-origin');
  socket = new WebSocket(endpoint);
  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('cdp-open-timeout')), 5000);
    socket.addEventListener('open', () => { clearTimeout(timer); resolve(); }, { once: true });
    socket.addEventListener('error', () => { clearTimeout(timer); reject(new Error('cdp-open-error')); }, { once: true });
  });
  const expression = `(async () => {
    const phase = ${JSON.stringify(phase)};
    const allowedHosts = ${JSON.stringify(allowedHosts)};
    const devices = JSON.parse(localStorage.getItem('orchester.devices') ?? '[]');
    const hashes = {};
    for (const key of ['orchester.devices','orchester.panels','orchester.panel-columns','orchester.theme']) {
      const value = localStorage.getItem(key);
      hashes[key] = value === null ? null : Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(value)))).map(x=>x.toString(16).padStart(2,'0')).join('');
    }
    const safe = {originalRecordCount: Array.isArray(devices) ? devices.length : null, localStorageValueHashes:hashes, nativeBridgePresent:Boolean(window.Capacitor?.Plugins?.Credentials && window.Capacitor?.Plugins?.CapacitorHttp), selectedPalette:document.documentElement.dataset.theme, metadataMatchesManagedEndpoint:false, explicitCredentialGetCalls:0, explicitHttpRequests:0};
    if (!Array.isArray(devices) || devices.length !== 1 || !safe.nativeBridgePresent) return safe;
    const device = devices[0];
    if (typeof device.address !== 'string' || typeof device.name !== 'string') return safe;
    let address;
    try { address = new URL(device.address); } catch { return safe; }
    safe.metadataMatchesManagedEndpoint = address.protocol === 'http:' && address.port === '7337' && address.pathname === '/' && !address.username && !address.password && !address.search && !address.hash && allowedHosts.includes(address.hostname);
    if (phase !== 'wire' || !safe.metadataMatchesManagedEndpoint) return safe;
    safe.explicitCredentialGetCalls = 1;
    let credential;
    try { credential = await Capacitor.Plugins.Credentials.get({key:'orchester.token:'+address.origin}); } catch { safe.nativeAesDecryptionSucceeded=false; return safe; }
    safe.nativeAesDecryptionSucceeded = typeof credential.value === 'string' && credential.value.length > 0;
    if (!safe.nativeAesDecryptionSucceeded) return safe;
    safe.explicitHttpRequests = 1;
    try {
      const response = await Capacitor.Plugins.CapacitorHttp.request({url:address.origin+'/remote/v1/info',method:'GET',headers:{'Content-Type':'application/json',Authorization:'Bearer '+credential.value},responseType:'json',connectTimeout:10000,readTimeout:20000,disableRedirects:true});
      safe.actualHttpStatus=response.status;
      safe.infoObjectShapeMatches = response.data && typeof response.data === 'object' && typeof response.data.name === 'string';
      safe.originalSavedNameMatches = Boolean(safe.infoObjectShapeMatches && response.data.name === device.name);
    } catch { safe.nativeHttpRequestFailed=true; }
    credential = undefined;
    return safe;
  })()`;
  const value = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('cdp-evaluation-timeout')), 35000);
    socket.addEventListener('message', event => {
      const data = JSON.parse(event.data);
      if (data.id !== 1) return;
      clearTimeout(timer);
      if (data.error || data.result?.exceptionDetails) reject(new Error('cdp-evaluation-error'));
      else resolve(data.result.result.value);
    });
    socket.send(JSON.stringify({id:1,method:'Runtime.evaluate',params:{expression,awaitPromise:true,returnByValue:true}}));
  });
  result = {schemaVersion:1,status:'native-product-probe-completed',phase,observedAtUtc:new Date().toISOString(),targetHost:'Linux',targetAvd:'orchester-qa',targetAppPid:identity.launchedAppPid,appUid:10216,...value,automaticStartupReconnectRequestsNotMeasured:true,totalNativeCredentialAndHttpCallCountsNotClaimed:true,newPairSetRemoveOrConnectCallsIssuedByProbe:0,rawCredentialsDeviceAddressesNamesAndResponsesIncluded:false,originalWindowsFiveCredentialWireVerificationClaimed:false};
} catch (error) {
  result = {schemaVersion:1,status:'native-product-probe-failed',phase,observedAtUtc:new Date().toISOString(),errorType:error.name,rawErrorDetailsIncluded:false};
} finally { socket?.close(); }
result.sourceSha256 = createHash('sha256').update(readFileSync(process.argv[1])).digest('hex');
writeFileSync(`${base}/native-product-${phase}-safe.json`, JSON.stringify(result,null,2)+'\n', {flag:'wx'});
console.log(JSON.stringify(result));
process.exitCode = result.status === 'native-product-probe-completed' ? 0 : 1;
