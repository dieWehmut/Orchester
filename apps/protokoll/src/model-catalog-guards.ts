import {
  MODEL_CATALOG_SCHEMA_VERSION,
  type ActiveModelDto,
  type ModelCatalogDto,
  type ModelChoiceDto,
  type ModelProfileDto,
  type ProviderChoiceDto,
} from './api'

type UnknownRecord = Record<string, unknown>

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function hasOnlyKeys(record: UnknownRecord, allowed: readonly string[]): boolean {
  return Object.keys(record).every((key) => allowed.includes(key))
}

function text(value: unknown, maxLength = 160): string | null {
  if (typeof value !== 'string') return null
  const normalized = value.trim()
  if (!normalized || normalized.length > maxLength || /[\u0000-\u001f\u007f]/.test(normalized)) {
    return null
  }
  return normalized
}

function nullableText(value: unknown, maxLength = 160): string | null | undefined {
  if (value === null) return null
  return text(value, maxLength) ?? undefined
}

function safeField(value: unknown): string | null {
  const field = text(value, 240)
  return field !== null && /^[A-Za-z0-9_.\-[\]]+$/.test(field) ? field : null
}

function safeReason(value: unknown): string | null {
  const reason = text(value, 500)
  if (reason === null) return null
  if (/(?:https?|wss?|file):\/\//i.test(reason)) return null
  if (/(?:[A-Za-z]:\\|\\\\|\/(?:home|Users|private|tmp|var)\/)/i.test(reason)) return null
  if (/(?:api[_-]?key|credential|token|secret|password)\s*[:=]/i.test(reason)) return null
  return reason
}

function parseChoice(raw: unknown, profileRequired: false): ModelChoiceDto | null
function parseChoice(raw: unknown, profileRequired: true): ModelProfileDto | null
function parseChoice(
  raw: unknown,
  profileRequired: boolean,
): ModelChoiceDto | ModelProfileDto | null {
  if (
    !isRecord(raw) ||
    !hasOnlyKeys(raw, [
      'profile',
      'provider',
      'provider_name',
      'model',
      'reasoning_effort',
      'plan_reasoning_effort',
      'service_tier',
    ])
  ) {
    return null
  }

  const profile = profileRequired ? text(raw.profile) : nullableText(raw.profile)
  const provider = text(raw.provider)
  const providerName = text(raw.provider_name)
  const model = text(raw.model)
  const reasoningEffort = nullableText(raw.reasoning_effort)
  const planReasoningEffort = nullableText(raw.plan_reasoning_effort)
  const serviceTier = nullableText(raw.service_tier)
  if (
    profile === undefined ||
    provider === null ||
    providerName === null ||
    model === null ||
    reasoningEffort === undefined ||
    planReasoningEffort === undefined ||
    serviceTier === undefined
  ) {
    return null
  }

  return {
    profile,
    provider,
    provider_name: providerName,
    model,
    reasoning_effort: reasoningEffort,
    plan_reasoning_effort: planReasoningEffort,
    service_tier: serviceTier,
  }
}

function parseActiveModel(raw: unknown): ActiveModelDto | null {
  if (!isRecord(raw) || typeof raw.state !== 'string') return null
  if (raw.state === 'configured') {
    if (!hasOnlyKeys(raw, ['state', 'choice'])) return null
    const choice = parseChoice(raw.choice, false)
    return choice === null ? null : { state: 'configured', choice }
  }
  if (raw.state === 'unresolved') {
    if (!hasOnlyKeys(raw, ['state', 'field', 'reason'])) return null
    const field = safeField(raw.field)
    const reason = safeReason(raw.reason)
    return field === null || reason === null ? null : { state: 'unresolved', field, reason }
  }
  if (raw.state === 'not_configured') {
    return hasOnlyKeys(raw, ['state']) ? { state: 'not_configured' } : null
  }
  return null
}

function parseProvider(raw: unknown): ProviderChoiceDto | null {
  if (
    !isRecord(raw) ||
    !hasOnlyKeys(raw, [
      'id',
      'name',
      'active',
      'state',
      'model',
      'wire_api',
      'field',
      'reason',
    ]) ||
    typeof raw.active !== 'boolean'
  ) {
    return null
  }
  const id = text(raw.id)
  const name = text(raw.name)
  if (id === null || name === null) return null

  if (raw.state === 'selectable') {
    const model = text(raw.model)
    const wireApi = text(raw.wire_api)
    if (model === null || wireApi === null || raw.field !== null || raw.reason !== null) return null
    return {
      id,
      name,
      active: raw.active,
      state: 'selectable',
      model,
      wire_api: wireApi,
      field: null,
      reason: null,
    }
  }

  if (raw.state === 'unavailable') {
    const field = safeField(raw.field)
    const reason = safeReason(raw.reason)
    if (raw.model !== null || raw.wire_api !== null || field === null || reason === null) return null
    return {
      id,
      name,
      active: raw.active,
      state: 'unavailable',
      model: null,
      wire_api: null,
      field,
      reason,
    }
  }

  return null
}

export function parseModelCatalog(raw: unknown): ModelCatalogDto | null {
  if (
    !isRecord(raw) ||
    !hasOnlyKeys(raw, ['schema_version', 'active', 'selected_provider', 'providers', 'profiles']) ||
    raw.schema_version !== MODEL_CATALOG_SCHEMA_VERSION ||
    !Array.isArray(raw.providers) ||
    !Array.isArray(raw.profiles)
  ) {
    return null
  }

  const active = parseActiveModel(raw.active)
  const selectedProvider = nullableText(raw.selected_provider)
  if (active === null || selectedProvider === undefined) return null

  const providers: ProviderChoiceDto[] = []
  const providerIds = new Set<string>()
  for (const entry of raw.providers) {
    const provider = parseProvider(entry)
    if (provider === null || providerIds.has(provider.id)) return null
    providerIds.add(provider.id)
    providers.push(provider)
  }

  const profiles: ModelProfileDto[] = []
  const profileIds = new Set<string>()
  for (const entry of raw.profiles) {
    const profile = parseChoice(entry, true)
    if (profile === null || profileIds.has(profile.profile)) return null
    profileIds.add(profile.profile)
    profiles.push(profile)
  }

  if (selectedProvider !== null) {
    const selected = providers.find((provider) => provider.id === selectedProvider)
    if (!selected || !selected.active) return null
    if (providers.some((provider) => provider.id !== selectedProvider && provider.active)) return null
  } else if (providers.some((provider) => provider.active)) {
    return null
  }

  return {
    schema_version: MODEL_CATALOG_SCHEMA_VERSION,
    active,
    selected_provider: selectedProvider,
    providers,
    profiles,
  }
}

export function isModelCatalog(value: unknown): value is ModelCatalogDto {
  return parseModelCatalog(value) !== null
}
