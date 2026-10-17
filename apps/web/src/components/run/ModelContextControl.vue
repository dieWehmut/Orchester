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
import { AppButton, Spinner } from '@orchester/design'
import type { ModelCatalogDto, ModelSelectionRequestDto } from '@orchester/protokoll'
import { ChevronDown, CircleAlert } from '@lucide/vue'
import { computed } from 'vue'

import { useI18n } from '../../i18n'
import type { ModelCatalogStoreStatus } from '../../stores/model-catalog'
import ModelSelectionMenu from './ModelSelectionMenu.vue'
import { effortName, modelMenuGroups, REASONING_EFFORTS } from './model-menu'

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
  configure: []
  retry: []
}>()

const { t } = useI18n()

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
  if (props.status === 'stale') return t('run.modelStale')
  if (props.status === 'loading' || props.status === 'refreshing') return t('run.modelLoading')
  return ''
})
const modelLabel = computed(
  () => activeChoice.value?.model ?? activeProvider.value?.model ?? t('run.selectModel'),
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
  const provider = props.catalog?.selected_provider
  if (provider) return { provider }
  return activeChoice.value?.profile ? { profile: activeChoice.value.profile } : {}
})

/** Whether there is anything to choose between. */
const selectable = computed(() => {
  const catalog = props.catalog
  if (!catalog) return false
  return (
    configured.value || catalog.providers.length > 0 || catalog.profiles.length > 0
  )
})
const pending = computed(() => ['loading', 'refreshing'].includes(props.status))
const needsSetup = computed(() => props.status === 'ready' && props.catalog !== null && !selectable.value)

const groups = computed(() => modelMenuGroups(props.catalog))
const efforts = computed(() => {
  const names: string[] = [...REASONING_EFFORTS]
  if (effortLabel.value && !names.includes(effortLabel.value)) names.push(effortLabel.value)
  return [
    { id: 'effort:', label: t('run.effortDefault'), checked: !effortLabel.value },
    ...names.map(name => ({ id: `effort:${name}`, label: effortName(name), checked: effortLabel.value === name })),
  ]
})

/**
 * Turn a chosen item into the whole intent.
 *
 * Picking a provider or a profile keeps the effort in force, and picking an
 * effort keeps the axis in force: the control changes one thing at a time, and a
 * request that omitted the other axis would silently reset it.
 */
function choose(id: string): void {
  if (id === 'configure:') { emit('configure'); return }
  if (id === 'retry:') { emit('retry'); return }
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
  if (axis === 'configured') {
    emit('select', { effort: effortLabel.value || null })
    return
  }
  if (axis === 'effort') {
    emit('select', { ...chosenAxis.value, effort: value.length > 0 ? value : null })
  }
}
</script>

<template>
  <div class="model-context" data-model-context>
    <ModelSelectionMenu
      v-if="selectable"
      class="model-context__menu"
      :label="t('run.modelSelection')"
      :model="modelLabel"
      :effort="effortName(effortLabel) || t('run.effortDefault')"
      :groups="groups"
      :efforts="efforts"
      :stale="props.status === 'stale'"
      @select="choose"
    >
      <template #default="{ open }">
        <span class="model-context__readout" :title="providerLabel">
          <span class="model-context__model" data-model-context-model>{{ modelLabel }}</span>
          <span v-if="effortLabel" class="model-context__effort" data-model-context-effort>{{
            effortName(effortLabel)
          }}</span>
          <span v-if="statusLabel" class="model-context__status" data-model-context-status>{{
            statusLabel
          }}</span>
          <ChevronDown class="model-context__disclosure" :class="{ 'is-open': open }" :size="13" aria-hidden="true" />
        </span>
      </template>
    </ModelSelectionMenu>

    <span v-else-if="pending" class="model-context__unavailable" data-model-context-loading role="status">
      <Spinner :size="13" :label="t('run.modelLoading')" />
      {{ t('run.modelLoading') }}
    </span>
    <template v-else>
      <AppButton class="model-context__setup" size="sm" variant="ghost" data-model-context-unavailable
        @click="needsSetup ? emit('configure') : emit('retry')">
        <span class="model-context__icon" aria-hidden="true"><CircleAlert :size="13" /></span>
        <span>{{ needsSetup ? t('run.setupModel') : t('run.retryModels') }}</span>
      </AppButton>
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
  font-size: var(--text-sm);
}

/* The readout is the trigger: the reference's selector shows what is chosen and
   opens the list, rather than being a field that only says "model". */
.model-context__readout {
  display: inline-flex;
  font-size: 15px;
  min-inline-size: 0;
  align-items: center;
  gap: var(--space-2);
  min-block-size: var(--hit-target-min, 32px);
  padding: 0 var(--space-2);
  border-radius: 16px;
}

.model-context__disclosure.is-open { transform: rotate(180deg); }

.model-context__icon {
  display: grid;
  inline-size: 1.25rem;
  block-size: 1.25rem;
  flex: 0 0 1.25rem;
  place-items: center;
  color: var(--color-text-secondary);
}

.model-context__model,
.model-context__provider {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.model-context__model {
  max-inline-size: 20ch;
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
  padding-inline: 2px;
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

.model-context__setup {
  min-inline-size: 0;
  color: var(--color-text-secondary);
  font-size: var(--text-sm);
}

.model-context__setup :deep(.app-button__label) {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  white-space: nowrap;
}
</style>
