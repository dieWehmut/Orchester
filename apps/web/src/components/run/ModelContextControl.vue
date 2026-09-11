<script setup lang="ts">
import type { ModelCatalogDto } from '@orchester/protokoll'
import { CircleAlert, Sparkles } from '@lucide/vue'
import { computed } from 'vue'

import type { ModelCatalogStoreStatus } from '../../stores/model-catalog'

const props = withDefaults(
  defineProps<{
    catalog: ModelCatalogDto | null
    status?: ModelCatalogStoreStatus
  }>(),
  { status: 'idle' },
)

const configured = computed(() => props.catalog?.active.state === 'configured')
const activeChoice = computed(() =>
  configured.value && props.catalog?.active.state === 'configured'
    ? props.catalog.active.choice
    : null,
)
const activeProvider = computed(() =>
  props.catalog?.providers.find((provider) => provider.active) ?? null,
)
const statusLabel = computed(() => {
  if (props.status === 'stale') return 'stale'
  if (props.status === 'loading' || props.status === 'refreshing') return 'loading'
  return ''
})
const modelLabel = computed(() => activeChoice.value?.model ?? activeProvider.value?.model ?? '')
const providerLabel = computed(() => activeChoice.value?.provider_name ?? activeProvider.value?.name ?? '')
const effortLabel = computed(() => activeChoice.value?.reasoning_effort ?? 'default')
</script>

<template>
  <div class="model-context" data-model-context>
    <template v-if="configured && activeChoice">
      <span class="model-context__icon" aria-hidden="true"><Sparkles :size="13" /></span>
      <span class="model-context__copy">
        <span class="model-context__model" data-model-context-model>{{ modelLabel }}</span>
        <span class="model-context__provider" data-model-context-provider>{{ providerLabel }}</span>
      </span>
      <span class="model-context__effort" data-model-context-effort>{{ effortLabel }}</span>
    </template>
    <span
      v-else
      class="model-context__unavailable"
      data-model-context-unavailable
      aria-disabled="true"
    >
      <span class="model-context__icon" aria-hidden="true"><CircleAlert :size="13" /></span>
      <span>Model unavailable</span>
    </span>
    <span v-if="statusLabel" class="model-context__status" data-model-context-status>
      {{ statusLabel }}
    </span>
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

.model-context__icon {
  display: grid;
  inline-size: 1.25rem;
  block-size: 1.25rem;
  flex: 0 0 1.25rem;
  place-items: center;
  border: 1px solid var(--color-accent-border);
  border-radius: 50%;
  color: var(--color-accent);
  font-size: 0.7rem;
}

.model-context__copy {
  display: grid;
  min-inline-size: 0;
  line-height: var(--leading-tight);
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

.model-context__unavailable {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--color-text-tertiary);
}
</style>
