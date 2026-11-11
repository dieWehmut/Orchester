const fs = require('fs');
const crypto = require('crypto');
const root = '/home/han/project/Orchester';
const out = '/home/han/.local/state/orchester-docs-audit-20261111/pages-live';
const runtime = JSON.parse(fs.readFileSync(root + '/.playwright-mcp/pages-current-loaded-images-20261111.json'));
const official = JSON.parse(fs.readFileSync(root + '/docs/design/acceptance/agentrouter-platform-sync/sources/icons/final21-official-current-24-assets.json'));
const vitePath = root + '/node_modules/.pnpm/vite@7.3.6_@types+node@25.9.5_jiti@2.7.0_lightningcss@1.33.0/node_modules/vite/dist/node/chunks/config.js';
const viteSource = fs.readFileSync(vitePath, 'utf8');
const a = viteSource.indexOf('const nestedQuotesRE =');
const b = viteSource.indexOf('\n\n//#endregion', a);
if (a < 0 || b < 0) throw new Error('Expected exact local Vite helper unavailable');
const helperSource = viteSource.slice(a, b);
const svgToDataURL = new Function(helperSource + '\nreturn svgToDataURL;')();
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
const mapping = new Map(official.identities.map(i=>[i.provider,i]));
const rows = runtime.providers.map(r=>{
 const identity = mapping.get(r.id);
 if (!identity) throw new Error('Unexpected provider');
 return {provider:r.id, actualName:r.name, expectedName:identity.displayName, nameMatches:r.name===identity.displayName,
 images:r.images.map(i=>{
  const theme = i.classes.includes('agent-icon__dark') ? 'dark' : i.classes.includes('agent-icon__light') ? 'light' : null;
  if (!theme) throw new Error('Unknown actual image theme class');
  const mark = identity.marks[theme];
  const canonical = fs.readFileSync(root + '/' + mark.path);
  if(hash(canonical)!==mark.sha256) throw new Error('Canonical bytes differ from official receipt');
  const rawMatches = i.bodySha256===mark.sha256 && i.bytes===canonical.length;
  const expectedURL = mark.path.endsWith('.svg') ? svgToDataURL(canonical) : null;
  const comma = expectedURL ? expectedURL.indexOf(',') : -1;
  const normalizedBody = expectedURL ? (expectedURL.slice(0,comma).endsWith(';base64') ? Buffer.from(expectedURL.slice(comma+1),'base64') : Buffer.from(decodeURIComponent(expectedURL.slice(comma+1)))) : null;
  const normalizedMatches = normalizedBody && i.currentSource.startsWith('data:') && hash(normalizedBody)===i.bodySha256 && normalizedBody.length===i.bytes;
  return {theme,canonicalPath:mark.path,canonicalSha256:mark.sha256,currentBodySha256:i.bodySha256,currentBytes:i.bytes,expectedNormalizedBodySha256:normalizedBody?hash(normalizedBody):null,encodedURLStringIdentical:expectedURL?expectedURL===i.currentSource:null,loaded:i.loaded,visible:i.visible,responseStatus:i.responseStatus,matchKind:rawMatches?'original-canonical-bytes':normalizedMatches?'exact-vite-7.3.6-svg-decoded-payload':'mismatch',matchesOfficialCanonical:rawMatches||normalizedMatches};
 })};
});
const all=rows.length===21&&runtime.themeImageCount===42&&runtime.uniqueImageSourceCount===24&&rows.every(r=>r.nameMatches&&r.images.length===2&&r.images.every(i=>i.loaded&&i.responseStatus===200&&i.matchesOfficialCanonical));
const result = {schemaVersion:2,observedAt:runtime.observedAt,initialURLStringOnlyCheck:{path:'initial-url-string-only-icon-mapping.json',actualExitCode:1,reason:'XML-prolog SVG URLs additionally encode apostrophes as %27 in the browser; encoded URL strings differ while decoded normalized SVG bytes remain equal.'},sourceRevision:'71def643cb08b208cc79687f79255c504eb14dc6',officialReceiptSourceRevision:official.sourceRevision,providerCount:rows.length,themeImageCount:runtime.themeImageCount,uniqueRuntimeSourceCount:runtime.uniqueImageSourceCount,uniqueCanonicalAssetCount:new Set(rows.flatMap(r=>r.images.map(i=>i.canonicalPath))).size,all42MappingsLoadedAndMatchCanonical:all,rawCanonicalImageReferences:rows.flatMap(r=>r.images).filter(i=>i.matchKind==='original-canonical-bytes').length,viteNormalizedImageReferences:rows.flatMap(r=>r.images).filter(i=>i.matchKind==='exact-vite-7.3.6-svg-decoded-payload').length,viteHelper:{path:vitePath,localSourceSha256:hash(Buffer.from(viteSource)),extractedHelperSha256:hash(Buffer.from(helperSource)),purpose:'Reproduce exact bundler SVG inline transformation; canonical source bytes are preserved separately.'},rows};
fs.writeFileSync(out+'/current-icon-mapping-verification.json',JSON.stringify(result,null,2));
console.log(JSON.stringify({providerCount:rows.length,themeImages:runtime.themeImageCount,uniqueAssets:result.uniqueCanonicalAssetCount,allMappings:all,rawImageReferences:result.rawCanonicalImageReferences,normalizedImageReferences:result.viteNormalizedImageReferences,receiptSha256:hash(fs.readFileSync(out+'/current-icon-mapping-verification.json'))}));
if(!all) process.exitCode=1;
