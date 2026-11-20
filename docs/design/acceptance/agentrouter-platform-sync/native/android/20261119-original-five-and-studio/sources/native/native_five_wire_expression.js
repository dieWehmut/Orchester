(async () => {
  const unlocked = [];
  try {
  const keys = __ORIGINAL_KEYS__;
  const originalDevices = __ORIGINAL_DEVICES__;
  const normalize = value => {
    const url = new URL(value.includes('://') ? value : 'http://' + value);
    if (!['http:', 'https:'].includes(url.protocol)) throw new Error('unsupported-original-saved-origin');
    return url.origin;
  };
  const rawBefore = localStorage.getItem('orchester.devices');
  const currentBefore = JSON.parse(rawBefore || '[]');
  const originalRecordsRetained = JSON.stringify(currentBefore) === JSON.stringify(originalDevices);
  const capacitor = globalThis.Capacitor;
  const platformAndroid = typeof capacitor?.getPlatform === 'function' && capacitor.getPlatform() === 'android';
  const nativePlatform = typeof capacitor?.isNativePlatform === 'function' && capacitor.isNativePlatform();
  if (!platformAndroid || !nativePlatform || keys.length !== 5 || !originalRecordsRetained) {
    return {status: 'native-original-five-before-check-failed', nativePlatform, platformAndroid,
      originalSavedDeviceRecordsExactlyRetained: originalRecordsRetained, nativeCredentialGetCount: 0,
      nativeHttpGetAttemptCount: 0, rawPrivateValuesIncluded: false};
  }
  const candidates = keys.flatMap((key, index) => {
    try {
      const origin = normalize(key.replace(/^orchester\.token:/, ''));
      if (new URL(origin).port !== '7337') return [];
      const matching = originalDevices.filter(device => {
        try { return normalize(device.address) === origin; } catch { return false; }
      });
      return matching.length === 1 ? [{index, origin, device: matching[0]}] : [];
    } catch { return []; }
  });
  if (candidates.length !== 1) {
    return {status: 'original-saved7337-selection-failed', originalSaved7337CandidateCount: candidates.length,
      nativeCredentialGetCount: 0, nativeHttpGetAttemptCount: 0, originalSavedDeviceRecordsExactlyRetained: true,
      addressesOrTokensIncluded: false};
  }
  const successes = [];
  for (const key of keys) {
    try {
      const result = await capacitor.Plugins.Credentials.get({key});
      const success = typeof result.value === 'string' && result.value.length > 0;
      successes.push(success);
      unlocked.push(success ? result.value : null);
    } catch { successes.push(false); unlocked.push(null); }
  }
  let attempted = 0, statusCode = null, nameMatch = false;
  if (successes.every(Boolean)) {
    const selected = candidates[0];
    attempted = 1;
    try {
      const response = await capacitor.Plugins.CapacitorHttp.request({
        method: 'GET', url: selected.origin + '/remote/v1/info',
        headers: {Authorization: 'Bearer ' + unlocked[selected.index], 'Content-Type': 'application/json'},
        responseType: 'json', connectTimeout: 10000, readTimeout: 20000, disableRedirects: true,
      });
      statusCode = Number.isInteger(response.status) ? response.status : 0;
      let body = response.data;
      if (typeof body === 'string') body = JSON.parse(body);
      nameMatch = statusCode === 200 && typeof body?.name === 'string' && body.name.length > 0
        && typeof selected.device.name === 'string' && selected.device.name.length > 0
        && body.name === selected.device.name;
    } catch { statusCode = 0; }
  }
  const rawAfter = localStorage.getItem('orchester.devices');
  const currentAfter = JSON.parse(rawAfter || '[]');
  return {
    status: 'native-original-five-and-original7337-observation-returned', nativePlatform, platformAndroid,
    originalEncryptedRecordCount: keys.length, nativeCredentialGetCount: successes.length,
    decryptSucceededCount: successes.filter(Boolean).length, decryptFailedCount: successes.filter(value => !value).length,
    originalSaved7337CandidateCount: candidates.length, nativeHttpGetAttemptCount: attempted,
    original7337GetStatusCodes: attempted ? [statusCode] : [], original7337InfoNameMatched: nameMatch,
    originalSavedOriginUsedWithoutRewrite: true, requestsOnlyOriginal7337Info: true,
    originalSavedDeviceRecordsExactlyRetained: originalRecordsRetained && JSON.stringify(currentAfter) === JSON.stringify(originalDevices),
    currentSavedDeviceStorageRawUnchanged: rawBefore === rawAfter,
    newPairingUsed: false, credentialSetOrRemoveInvoked: false, addressesNamesCiphertextOrTokensReturned: false,
    observerExplicitRequestScopeOnly: true,
    automaticStartupReconnectRequestsMeasured: false,
  };
  } finally {
    // Release array references; no memory zeroization claim is made.
    unlocked.fill(null);
  }
})()
