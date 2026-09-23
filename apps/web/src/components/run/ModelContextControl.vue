<script setup lang="ts">
/**
 * The model picker: what the next run will use, and how to change it.
 *
 * The reference draws this as one compact control at the trailing edge of the
 * field, and this is the readout the composer already showed, made real. It was
 * a readout for as long as the runtime could not be asked to choose - the
 * catalog was read-only and a run's host is built fresh - and it is a control
 * now that `PUT /models/selection` exists.
 *
 * What it offers is what the runtime reports: the providers this workspace can
 * reach, the named profiles, and the reasoning effort. A provider the runtime
 * knows but cannot reach is drawn disabled with its reason, because a menu that
 * silently omits a provider leaves the reader wondering where it went.
 */
import { AppMenu, type AppMenuItem } from '@orchester/design'
import type { ModelCatalogDto, ModelSelectionRequestDto } from '@orchester/protokoll'
import { ChevronDown, CircleAlert, Sparkles } from '@lucide/vue'
import { computed } from 'vue'

import { useI18n } from '../../i18n'
import type { ModelCatalogStoreStatus } from '../../stores/model-catalog'

const props = withDefaults(
  defineProps<{
    catalog: ModelCatalogDto | null
    status?: ModelCatalogStoreStatus
  }>(),
  { status: 'idle' },
)

const emit = defineEmits<{
  /** The whole intent rather than a delta: the three axes the runtime takes. */
  select: [selection: ModelSelectionRequestDto]
}>()

const { t } = useI18n()

/**
 * The effort names this product offers, in the vocabulary the provider APIs use.
 *
 * The runtime passes any non-empty value through rather than holding a list, so
 * these are the names this surface knows how to say. A value the catalog reports
 * that is not among them stays visible in the trigger instead of being forced
 * into one of these.
 */
const EFFORT_NAMES = ['minimal', 'low', 'medium', 'high'] as const
const EFFORT_KEYS = {
  minimal: 'run.effortMinimal',
  low: 'run.effortLow',
  medium: 'run.effortMedium',
  high: 'run.effortHigh',
} as const

const configured = computed(() => props.catalog?.active.state === 'configured')
const activeChoice = computed(() =>
  configured.value && props.catalog?.active.state === 'configured'
    ? props.catalog.active.choice
    : null,
)
const activeProvider = computed(
  () => props.catalog?.providers.find((provider) => provider.active) ?? null,
)
const statusLabel = computed(() => {
  if (props.status === 'stale') return 'stale'
  if (props.status === 'loading' || props.status === 'refreshing') return 'loading'
  return ''
})
const modelLabel = computed(
  () => activeChoice.value?.model ?? activeProvider.value?.model ?? '',
)
const providerLabel = computed(
  () => activeChoice.value?.provider_name ?? activeProvider.value?.name ?? '',
)
const effortLabel = computed(() => activeChoice.value?.reasoning_effort ?? '')

/**
 * Which axis the reader last chose, read off the catalog rather than remembered
 * here: the runtime is the one that knows, and a value kept in this component
 * could disagree with it after a reload.
 */
const chosenAxis = computed<ModelSelectionRequestDto>(() => {
  if (activeChoice.value?.profile) return { profile: activeChoice.value.profile }
  const provider = props.catalog?.selected_provider
  return provider ? { provider } : {}
})

/** Whether there is anything to choose between. */
const selectable = computed(() => {
  const catalog = props.catalog
  if (!catalog) return false
  return (
    configured.value || catalog.providers.length > 0 || catalog.profiles.length > 0
  )
})

const items = computed<AppMenuItem[]>(() => {
  const catalog = props.catalog
  if (!catalog) return []

  const providerItems: AppMenuItem[] = catalog.providers.map((provider) => ({
    id: `provider:${provider.id}`,
    label: provider.name,
    hint: provider.state === 'selectable' ? (provider.model ?? '') : (provider.reason ?? ''),
    disabled: provider.state !== 'selectable',
    checked: provider.active || catalog.selected_provider === provider.id,
  }))

  const profileItems: AppMenuItem[] = catalog.profiles.map((profile) => ({
    id: `profile:${profile.profile}`,
    label: profile.profile,
    hint: profile.model,
    checked: activeChoice.value?.profile === profile.profile,
  }))

  const effortItems: AppMenuItem[] = [
    {
      id: 'effort:',
      label: t('run.effortOption', { name: t('run.effortDefault') }),
      checked: (activeChoice.value?.reasoning_effort ?? null) === null,
    },
    ...EFFORT_NAMES.map((name) => ({
      id: `effort:${name}`,
      label: t('run.effortOption', { name: t(EFFORT_KEYS[name]) }),
      checked: activeChoice.value?.reasoning_effort === name,
    })),
  ]

  return [...providerItems, ...profileItems, ...effortItems]
})

/**
 * Turn a chosen item into the whole intent.
 *
 * Picking a provider or a profile keeps the effort in force, and picking an
 * effort keeps the axis in force: the control changes one thing at a time, and a
 * request that omitted the other axis would silently reset it.
 */
function choose(id: string): void {
  const at = id.indexOf(':')
  const axis = at < 0 ? '' : id.slice(0, at)
  const value = at < 0 ? '' : id.slice(at + 1)

  if (axis === 'provider') {
    emit('select', { provider: value, effort: effortLabel.value || null })
    return
  }
  if (axis === 'profile') {
    emit('select', { profile: value, effort: effortLabel.value || null })
    return
  }
  if (axis === 'effort') {
    emit('select', { ...chosenAxis.value, effort: value.length > 0 ? value : null })
  }
}
</script>

<template>
  <div class="model-context" data-model-context>
    <AppMenu
      v-if="selectable"
      class="model-context__menu"
      data-model-picker
      placement="top"
      align="end"
      :label="t('run.modelSelection')"
      :items="items"
      @select="choose"
    >
      <template #trigger>
        <span class="model-context__readout">
          <span class="model-context__icon" aria-hidden="true">
            <Sparkles :size="13" />
          </span>
          <span class="model-context__model" data-model-context-model>{{ modelLabel }}</span>
          <span class="model-context__provider" data-model-context-provider>{{ providerLabel }}</span>
          <span v-if="effortLabel" class="model-context__effort" data-model-context-effort>{{
            effortLabel
          }}</span>
          <span v-if="statusLabel" class="model-context__status" data-model-context-status>{{
            statusLabel
          }}</span>
          <ChevronDown class="model-context__disclosure" :size="13" aria-hidden="true" />
        </span>
      </template>
    </AppMenu>

    <template v-else>
      <span class="model-context__unavailable" data-model-context-unavailable aria-disabled="true">
        <span class="model-context__icon" aria-hidden="true"><CircleAlert :size="13" /></span>
        <span>{{ t('run.modelUnavailable') }}</span>
      </span>
      <span v-if="statusLabel" class="model-context__status" data-model-context-status>
        {{ statusLabel }}
      </span>
    </template>
  </div>
</template>

<style scoped>
.model-context {
  display: inline-flex;
  min-inline-size: 0;
  align-items: center;
  gap: var(--space-2);
  color: var(--color-text-secondary);
  font-size: var(--text-xs);
}

/* The readout is the trigger: the reference's selector shows what is chosen and
   opens the list, rather than being a field that only says "model". */
.model-context__readout {
  display: inline-flex;
  min-inline-size: 0;
  align-items: center;
  gap: var(--space-2);
  min-block-size: var(--hit-target-min, 32px);
  padding: 0 var(--space-2);
  border-radius: var(--radius-sm);
}

.model-context__menu :deep(.app-menu__trigger) {
  min-block-size: var(--hit-target-min, 32px);
  padding: 0;
  border: 0;
  background: transparent;
}

.model-context__readout:hover {
  background: var(--color-bg-element);
}

.model-context__icon {
  display: grid;
  inline-size: 1.25rem;
  block-size: 1.25rem;
  flex: 0 0 1.25rem;
  place-items: center;
  border: 1px solid var(--color-accent-border);
  border-radius: 50%;
  color: var(--color-accent);
}

.model-context__model,
.model-context__provider {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.model-context__model {
  color: var(--color-text-primary);
  font-weight: var(--weight-medium);
}

.model-context__provider,
.model-context__effort,
.model-context__status {
  color: var(--color-text-tertiary);
}

.model-context__effort,
.model-context__status {
  padding: 0.125rem 0.375rem;
  border: 1px solid var(--color-border-base);
  border-radius: 999px;
  white-space: nowrap;
}

.model-context__disclosure {
  flex: 0 0 auto;
  color: var(--color-text-tertiary);
}

.model-context__unavailable {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--color-text-tertiary);
}
</style>