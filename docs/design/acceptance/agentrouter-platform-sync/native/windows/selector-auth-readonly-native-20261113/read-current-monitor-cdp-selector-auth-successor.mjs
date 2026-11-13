import {readFileSync, writeFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

export const CANONICAL_PROVIDERS = Object.freeze([
  'codex', 'claude', 'deepseek', 'opencode', 'gemini', 'antigravity', 'cursor',
  'workbuddy', 'workbuddy_global', 'qwen', 'openclaw', 'cline', 'roo', 'kilo',
  'hermes', 'pi', 'crush', 'claude_app', 'trae', 'zcode', 'craft',
]);
export const SERVICE_PROVIDERS = Object.freeze([
  'openclaw', 'workbuddy', 'workbuddy_global', 'claude_app', 'trae', 'zcode', 'craft',
]);

// agent_id and DOM attributes are compared in memory. No raw IDs, names,
// URLs, titles, process metadata, or arbitrary capability strings are returned.
export function summarizeMonitorObservation(observation, canonical = CANONICAL_PROVIDERS, services = SERVICE_PROVIDERS) {
  const member = (value, allowed) => allowed.includes(value) ? value : null;
  const count = value => Number.isSafeInteger(value) && value >= 0 ? value : null;
  const has = (value, key) => Object.prototype.hasOwnProperty.call(value, key);
  const sourceTimestampValid = value => {
    if (typeof value !== 'string' || !Number.isFinite(Date.parse(value))) return false;
    const parts = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/.exec(value);
    if (parts === null) return false;
    const year = Number(parts[1]), month = Number(parts[2]), day = Number(parts[3]);
    const leapYear = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
    const days = [31, leapYear ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    return month >= 1 && month <= 12 && day >= 1 && day <= days[month - 1] &&
      Number(parts[4]) <= 23 && Number(parts[5]) <= 59 && Number(parts[6]) <= 59;
  };
  const dtoReadSucceeded = observation.rosterStatus === 200;
  const agents = dtoReadSucceeded && Array.isArray(observation.agents) ? observation.agents : [];
  const rows = Array.isArray(observation.rows) ? observation.rows : [];
  const schemaVersion = member(observation.schemaVersion, [2, 3]);
  const theme = member(observation.theme, ['dark', 'light']);
  const safeAgents = agents.filter(agent => agent && canonical.includes(agent.provider)).map(agent => {
    const activeWindows = count(agent.active_windows);
    const source = member(agent.window_count_source, ['managed_sessions', 'tauri_windows', 'external_processes']);
    const capabilitiesValid = Array.isArray(agent.capabilities) && agent.capabilities.every(value =>
      typeof value === 'string' && value.trim().length > 0 && value.length <= 80 && !/[\\/]/.test(value));
    const capabilities = {
      monitorOnly: Array.isArray(agent.capabilities) && agent.capabilities.includes('monitor_only'),
      processPresence: Array.isArray(agent.capabilities) && agent.capabilities.includes('process_presence'),
      servicePresence: Array.isArray(agent.capabilities) && agent.capabilities.includes('service_presence'),
    };
    const sourceFieldPresent = has(agent, 'process_sources');
    const processSources = Array.isArray(agent.process_sources) ? agent.process_sources.map(value => {
      const recordValid = value !== null && typeof value === 'object' && !Array.isArray(value);
      const entry = recordValid ? value : {};
      const keysValid = recordValid && Object.keys(entry).every(key => ['environment', 'status', 'count', 'observed_at'].includes(key));
      const status = member(entry.status, ['available', 'stale', 'unavailable']);
      const environment = member(entry.environment, ['windows', 'wsl', 'local']);
      const observedAtValid = sourceTimestampValid(entry.observed_at);
      const observedCount = count(entry.count);
      return {
        environment, status, count: observedCount,
        observed_at_is_null: entry.observed_at === null,
        observed_at_valid: observedAtValid,
        contractFieldsValid: keysValid && environment !== null && status !== null &&
          (status === 'unavailable' ? entry.count === null && entry.observed_at === null : observedCount !== null && observedAtValid),
      };
    }) : null;
    // Missing/malformed data is flagged, never normalized into confirmed zero.
    // Schema 2 omits this field and the shared parser normalizes it to null.
    const legacyUnobserved = schemaVersion === 2 && !sourceFieldPresent;
    const processSourcesValid = legacyUnobserved || (schemaVersion === 3 && sourceFieldPresent &&
      (agent.process_sources === null || (processSources !== null && processSources.length > 0 &&
        processSources.every(value => value.contractFieldsValid))));
    const sourceComplete = processSourcesValid && processSources !== null && processSources.every(value => value.status === 'available');
    const unit = source === null ? null : source !== 'external_processes' ? 'windows' : capabilities.servicePresence ? 'services' : 'processes';
    let presentedCount = null, quality = 'unknown';
    if (activeWindows !== null && source !== null && processSourcesValid) {
      if (sourceComplete || (source !== 'external_processes' && activeWindows > 0)) {
        presentedCount = activeWindows; quality = 'exact';
      } else if (activeWindows > 0) {
        presentedCount = activeWindows; quality = 'lower_bound';
      }
    }
    const availableCount = processSourcesValid && processSources !== null
      ? processSources.reduce((total, value) => total + (value.status === 'available' ? value.count : 0), 0) : null;
    const externalCountMatches = source === 'external_processes' && availableCount !== null && activeWindows !== null
      ? Number.isSafeInteger(availableCount) && availableCount === activeWindows : null;
    return {
      provider: agent.provider,
      active_windows: activeWindows,
      activeWindowsContractFieldValid: activeWindows !== null,
      active_sessions: count(agent.active_sessions),
      active_runs: count(agent.active_runs),
      active_subagents: count(agent.active_subagents),
      window_count_source: source,
      process_sources: processSources,
      processSourcesFieldPresent: sourceFieldPresent,
      processSourcesLegacyUnobserved: legacyUnobserved,
      processSourcesContractFieldsValid: processSourcesValid,
      capabilitiesContractFieldsValid: capabilitiesValid,
      mappedCountAndSourceFieldsValid: activeWindows !== null && source !== null && processSourcesValid &&
        count(agent.active_sessions) !== null && count(agent.active_runs) !== null && count(agent.active_subagents) !== null &&
        capabilitiesValid && externalCountMatches !== false,
      activity: member(agent.activity, ['offline', 'idle', 'starting', 'running', 'waiting_approval', 'stopping', 'error', 'unknown']),
      capabilities,
      presentedCount: {unit, count: presentedCount, quality},
      externalAvailableSourceCountMatches: externalCountMatches,
      externalServiceUnitMatchesCatalog: source === 'external_processes'
        ? (unit === 'services') === services.includes(agent.provider) : null,
    };
  });
  const safeRows = rows.map((row, index) => {
    const provider = member(row.provider, canonical);
    const matches = typeof row.agentId === 'string' && row.agentId.length > 0
      ? agents.filter(agent => agent && agent.agent_id === row.agentId) : [];
    const dto = matches.length === 1 ? matches[0] : null;
    const source = dto ? member(dto.window_count_source, ['managed_sessions', 'tauri_windows', 'external_processes']) : null;
    const unit = source === null ? null : source !== 'external_processes' ? 'windows'
      : Array.isArray(dto.capabilities) && dto.capabilities.includes('service_presence') ? 'services' : 'processes';
    const images = Array.isArray(row.images) ? row.images : [];
    const darkCount = images.filter(value => value.theme === 'dark').length;
    const lightCount = images.filter(value => value.theme === 'light').length;
    const visible = images.filter(value => value.visible === true);
    const units = Array.isArray(row.units) ? row.units : [];
    const presenceUnits = units.filter(value => ['windows', 'processes', 'services'].includes(value));
    return {
      rowIndex: index, provider,
      agentIdPresent: typeof row.agentId === 'string' && row.agentId.length > 0,
      dtoMatchCount: dtoReadSucceeded ? matches.length : null,
      dtoProviderMatches: dto ? dto.provider === provider && provider !== null : null,
      expectedDtoUnit: unit,
      domPresenceUnits: presenceUnits,
      domPresenceUnitCheckApplicable: row.metricsRendered === true,
      domPresenceUnitMatchesDto: row.metricsRendered !== true || unit === null ? null : presenceUnits.length === 1 && presenceUnits[0] === unit,
      ariaIncludesExpectedChineseUnit: unit === null ? null : row.ariaUnitIncludes?.[unit] === true,
      iconHolderCount: count(row.iconHolderCount),
      themeImageCount: images.length,
      loadedThemeImageCount: images.filter(value => value.loaded === true).length,
      visibleThemeImageCount: visible.length,
      darkThemeImageCount: darkCount,
      lightThemeImageCount: lightCount,
      currentLayoutThemePairComplete: provider !== null && row.iconHolderCount === 1 && images.length === 2 &&
        darkCount === 1 && lightCount === 1 && images.every(value => value.loaded === true) && visible.length === 1,
      visibleImageMatchesCurrentTheme: theme === null ? null : visible.length === 1 && visible[0].theme === theme,
    };
  });
  const providers = canonical.map(provider => {
    const visibleRows = safeRows.filter(row => row.provider === provider);
    const unitRows = visibleRows.filter(row => row.domPresenceUnitCheckApplicable);
    const dtoInstances = safeAgents.filter(agent => agent.provider === provider);
    return {
      provider, expectedExternalUnit: services.includes(provider) ? 'services' : 'processes',
      visibleRowCount: visibleRows.length, dtoInstanceCount: dtoReadSucceeded ? dtoInstances.length : null,
      everyVisibleRowMatchesDto: dtoReadSucceeded && visibleRows.length > 0
        ? visibleRows.every(row => row.dtoMatchCount === 1 && row.dtoProviderMatches === true) : null,
      metricUnitRowCheckCount: unitRows.length,
      everyVisibleRowUnitMatchesDto: dtoReadSucceeded && unitRows.length > 0
        ? unitRows.every(row => row.domPresenceUnitMatchesDto === true) : null,
      everyVisibleRowHasTwoLoadedThemeImages: visibleRows.length > 0
        ? visibleRows.every(row => row.currentLayoutThemePairComplete) : null,
    };
  });
  return {
    schemaVersion,
    rosterStatus: count(observation.rosterStatus),
    theme, visibleThemeButtons: count(observation.visibleThemeButtons),
    legacyRailThemeButtonCount: count(observation.legacyRailThemeButtonCount),
    themeSelectorScope: 'workspace-navigation-theme-zh-CN',
    dtoReadSucceeded, dtoCorrespondenceChecked: dtoReadSucceeded,
    layoutScope: 'rendered-fleet-rows-current-page',
    canonicalProviderCount: canonical.length, serviceProviderCount: services.length,
    missingVisibleProviders: providers.filter(value => value.visibleRowCount === 0).map(value => value.provider),
    missingDtoProviders: dtoReadSucceeded ? providers.filter(value => value.dtoInstanceCount === 0).map(value => value.provider) : null,
    nonCanonicalVisibleRowCount: safeRows.filter(value => value.provider === null).length,
    nonCanonicalDtoCount: dtoReadSucceeded ? agents.filter(value => !value || !canonical.includes(value.provider)).length : null,
    providers, rows: safeRows, safeDto: dtoReadSucceeded ? safeAgents : null,
    hiddenRowsOrDetailsChecked: false, themeSwitched: false,
    viewportClippingOrScrollVisibilityChecked: false,
    officialIconPayloadBytesVerified: false, fullDesktop18AcceptanceClaimed: false,
  };
}

async function captureReadOnlyPageObservation() {
  const selector = '[data-rail-section="fleet"] [data-agent-fleet] > div > .agent-fleet__rows > [data-agent-id]';
  const response = await fetch('/api/v1/agents/status', {method: 'GET', redirect: 'error', signal: AbortSignal.timeout(5000)});
  const fleet = response.status === 200 ? await response.json() : {};
  return {
    schemaVersion: fleet.schema_version,
    rosterStatus: response.status,
    agents: fleet.agents,
    theme: document.documentElement.getAttribute('data-theme'),
    visibleThemeButtons: [...document.querySelectorAll('[data-workspace-navigation] button[aria-label="主题"]')].filter(value => value.getClientRects().length > 0).length,
    legacyRailThemeButtonCount: [...document.querySelectorAll('[data-rail-action="theme"]')].filter(value => value.getClientRects().length > 0).length,
    rows: [...document.querySelectorAll(selector)].filter(row => row.getClientRects().length > 0).map(row => {
      const provider = row.querySelector('[data-agent-provider]')?.getAttribute('data-agent-provider');
      const holders = [...row.querySelectorAll('[data-agent-icon]')].filter(value => value.getAttribute('data-agent-icon') === provider);
      const aria = row.querySelector('button')?.getAttribute('aria-label') ?? '';
      return {
        agentId: row.getAttribute('data-agent-id'), provider, iconHolderCount: holders.length,
        images: holders.flatMap(holder => [...holder.querySelectorAll('img')]).map(image => ({
          loaded: image.complete && image.naturalWidth > 0,
          visible: getComputedStyle(image).display !== 'none' && image.getClientRects().length > 0,
          theme: image.classList.contains('agent-icon__dark') ? 'dark' : image.classList.contains('agent-icon__light') ? 'light' : null,
        })),
        units: [...row.querySelectorAll('[data-agent-count-unit]')].map(value => value.getAttribute('data-agent-count-unit')),
        metricsRendered: row.querySelector('[data-agent-metrics]') !== null,
        ariaUnitIncludes: {windows: aria.includes('窗口'), processes: aria.includes('进程'), services: aria.includes('服务进程')},
      };
    }),
  };
}

export function createReadOnlyExpression() {
  return `(async()=>(${summarizeMonitorObservation.toString()})(await (${captureReadOnlyPageObservation.toString()})(),${JSON.stringify(CANONICAL_PROVIDERS)},${JSON.stringify(SERVICE_PROVIDERS)}))()`;
}

export function validateLaunchAndCurrentNativeGate(launchRaw, gateRaw, expectedGateSha256, nowMs = Date.now()) {
  const sha256 = value => createHash('sha256').update(value).digest('hex');
  if (!/^[a-f0-9]{64}$/.test(expectedGateSha256 ?? '') || sha256(gateRaw) !== expectedGateSha256) throw new Error('native-gate-pin-differs');
  const launchText = launchRaw.toString('utf8').replace(/^\uFEFF/, '');
  const launch = JSON.parse(launchText);
  const createdExact = launchText.match(/"Created"\s*:\s*([0-9]+)/)?.[1];
  if (launch.ProcessId !== 37324 || createdExact !== '134389718070239855' || launch.cdpPort !== 19322 ||
      launch.binarySha256 !== 'e4aa765befa85c7fb053075844be5693685c00677dd62b4cfff4592ec68c1d05' ||
      launch.sourceRevision !== '71def643cb08b208cc79687f79255c504eb14dc6') throw new Error('original-launch-binding-differs');
  const gate = JSON.parse(gateRaw.toString('utf8').replace(/^\uFEFF/, ''));
  const observedMs = Date.parse(gate.observedAtUtc);
  if (gate.schemaVersion !== 1 || gate.status !== 'current-native-original-gui-and-cdp-owner-verified' || gate.actualTargetHost !== 'Windows' ||
      gate.launchResultSha256 !== sha256(launchRaw) || gate.actualNativeQueryExitCode !== 0 ||
      gate.originalNativeHandleIdentityVerified !== true || gate.currentCdpPortOwnerIdentityVerified !== true ||
      gate.originalReadonlyNodeAttemptsAccountedFor !== true ||
      !Number.isFinite(observedMs) || nowMs - observedMs < 0 || nowMs - observedMs > 300000) throw new Error('fresh-current-native-gate-required');
  return {sourceRevision: launch.sourceRevision, currentNativeGateSha256: expectedGateSha256};
}

async function readCdpValue(socket) {
  return new Promise((accept, reject) => {
    const timeout = setTimeout(() => reject(new Error('cdp-response-timeout')), 10000);
    socket.addEventListener('message', event => {
      try {
        const data = JSON.parse(event.data);
        if (data.id !== 1) return;
        clearTimeout(timeout);
        if (data.error || data.result?.exceptionDetails || !data.result?.result?.value) reject(new Error('cdp-evaluation-failed'));
        else accept(data.result.result.value);
      } catch { clearTimeout(timeout); reject(new Error('cdp-response-invalid')); }
    });
    socket.send(JSON.stringify({id: 1, method: 'Runtime.evaluate', params: {expression: createReadOnlyExpression(), awaitPromise: true, returnByValue: true}}));
  });
}

async function main() {
  // The extra pinned receipt is an external native prerequisite, not a claim
  // that this JavaScript independently verifies Windows HANDLE or port owners.
  const [launchFile, outputFile, currentNativeGateFile, expectedNativeGateSha256] = process.argv.slice(2);
  let result, exitCode = 1, stage = 'launch-and-current-native-gate';
  try {
    if (!launchFile || !outputFile || !currentNativeGateFile || !expectedNativeGateSha256) throw new Error('four-arguments-required');
    const binding = validateLaunchAndCurrentNativeGate(readFileSync(launchFile), readFileSync(currentNativeGateFile), expectedNativeGateSha256);
    stage = 'target-enumeration';
    const response = await fetch('http://127.0.0.1:19322/json/list', {method: 'GET', redirect: 'error', signal: AbortSignal.timeout(5000)});
    if (!response.ok) throw new Error('cdp-query-failed');
    const targets = await response.json();
    if (!Array.isArray(targets)) throw new Error('cdp-target-list-invalid');
    const pages = targets.filter(target => target.type === 'page');
    if (pages.length === 0) throw new Error('no-page-targets');
    const views = [];
    for (const target of pages) {
      const url = new URL(target.url), endpoint = new URL(target.webSocketDebuggerUrl);
      if (url.hostname !== '127.0.0.1' || url.protocol !== 'http:' || url.username || url.password ||
          endpoint.hostname !== '127.0.0.1' || endpoint.port !== '19322' || endpoint.protocol !== 'ws:' || endpoint.username || endpoint.password) throw new Error('unexpected-target-origin');
      stage = 'readonly-page-observation';
      const socket = new WebSocket(endpoint);
      try {
        await new Promise((accept, reject) => {
          const timeout = setTimeout(() => reject(new Error('cdp-connect-timeout')), 5000);
          socket.addEventListener('open', () => { clearTimeout(timeout); accept(); }, {once: true});
          socket.addEventListener('error', () => { clearTimeout(timeout); reject(new Error('cdp-connect-failed')); }, {once: true});
        });
        views.push({viewIndex: views.length, ...await readCdpValue(socket)});
      } finally { socket.close(); }
    }
    result = {
      schemaVersion: 1, status: 'readonly-monitor-diagnostic-collected-not-desktop18-acceptance',
      observedAtUtc: new Date().toISOString(), ...binding, views,
      currentNativeIdentityAndPortOwnershipIndependentlyVerifiedByThisScript: false,
      guiLivenessOrDesktop18PassClaimed: false, domMutationCount: 0, modelCalls: 0, processOrInputSignals: 0,
      rawCredentialsPidAgentIdsNamesTitlesPathsOrUrlsIncluded: false,
    };
    exitCode = 0;
  } catch (error) {
    result = {schemaVersion: 1, status: 'readonly-successor-diagnostic-failed', stage,
      errorType: ['Error', 'TypeError', 'SyntaxError', 'RangeError'].includes(error?.name) ? error.name : 'Error',
      rawErrorsIncluded: false, guiLivenessOrDesktop18PassClaimed: false};
  }
  result.sourceSha256 = createHash('sha256').update(readFileSync(fileURLToPath(import.meta.url))).digest('hex');
  if (outputFile) writeFileSync(outputFile, JSON.stringify(result, null, 2) + '\n', {flag: 'wx'});
  console.log(JSON.stringify(result));
  process.exitCode = exitCode;
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) await main();
