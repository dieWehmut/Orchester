<script setup lang="ts">
import { computed } from 'vue'

export type ConnectionBannerStatus =
  | 'idle'
  | 'connecting'
  | 'connected'
  | 'reconnecting'
  | 'offline'
  | 'closed'
  | 'error'

const props = withDefaults(
  defineProps<{
    status: ConnectionBannerStatus
    label?: string
  }>(),
  { label: 'Connection' },
)

const visible = computed(() => props.status !== 'connected' && props.status !== 'idle')
</script>

<template>
  <div
    v-if="visible"
    class="connection-banner"
    :class="`connection-banner--${props.status}`"
    role="status"
    aria-live="polite"
    data-connection-banner
  >
    <strong>{{ props.label }}</strong>
    <span>{{ props.status }}</span>
  </div>
</template>

<style scoped>
.connection-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-4);
  border-block-end: 1px solid var(--color-border-base);
  background: var(--color-bg-element);
  color: var(--color-text-secondary);
  font-size: var(--text-xs);
}

.connection-banner--offline,
.connection-banner--error {
  border-block-end-color: var(--color-status-error);
  color: var(--color-status-error);
}

.connection-banner--reconnecting,
.connection-banner--connecting {
  border-block-end-color: var(--color-status-warning);
  color: var(--color-status-warning);
}
</style>
