<script setup lang="ts">
import { computed, onBeforeUnmount, watch } from 'vue'

import type { ToastItem, ToastTone } from './toast-types'

const props = withDefaults(
  defineProps<{
    items: readonly ToastItem[]
    label?: string
    timeout?: number
    maxVisible?: number
    dismissLabel?: string
  }>(),
  {
    label: 'Notifications',
    timeout: 5000,
    maxVisible: 4,
    dismissLabel: 'Dismiss',
  },
)

const emit = defineEmits<{
  dismiss: [id: string]
}>()

const timers = new Map<string, ReturnType<typeof setTimeout>>()
const visibleItems = computed(() => {
  const limit = Math.max(1, Math.floor(props.maxVisible))
  return props.items.slice(-limit)
})

function tone(item: ToastItem): ToastTone {
  return item.tone ?? 'info'
}

function role(item: ToastItem): 'alert' | 'status' {
  const itemTone = tone(item)
  return itemTone === 'error' || itemTone === 'warning' ? 'alert' : 'status'
}

function clearTimer(id: string) {
  const timer = timers.get(id)
  if (timer !== undefined) {
    clearTimeout(timer)
    timers.delete(id)
  }
}

function dismiss(id: string) {
  clearTimer(id)
  emit('dismiss', id)
}

function scheduleItems(items: readonly ToastItem[]) {
  const activeIds = new Set(items.map((item) => item.id))
  for (const id of timers.keys()) {
    if (!activeIds.has(id)) {
      clearTimer(id)
    }
  }

  for (const item of items) {
    if (timers.has(item.id)) {
      continue
    }

    const duration = item.timeout ?? props.timeout
    if (!Number.isFinite(duration) || duration <= 0) {
      continue
    }

    const timer = setTimeout(() => {
      timers.delete(item.id)
      emit('dismiss', item.id)
    }, duration)
    timers.set(item.id, timer)
  }
}

watch(
  () => [props.items, props.timeout] as const,
  () => scheduleItems(props.items),
  { immediate: true },
)

onBeforeUnmount(() => {
  for (const timer of timers.values()) {
    clearTimeout(timer)
  }
  timers.clear()
})
</script>

<template>
  <section class="toast-region" role="region" :aria-label="label">
    <article
      v-for="item in visibleItems"
      :key="item.id"
      data-toast-item
      class="toast-region__item"
      :class="'toast-region__item--' + tone(item)"
      :role="role(item)"
      :aria-live="role(item) === 'alert' ? 'assertive' : 'polite'"
    >
      <div class="toast-region__content">
        <strong v-if="item.title" class="toast-region__title">{{ item.title }}</strong>
        <p class="toast-region__message">{{ item.message }}</p>
      </div>
      <button
        data-toast-dismiss
        class="toast-region__dismiss"
        type="button"
        :aria-label="dismissLabel"
        @click="dismiss(item.id)"
      >
        {{ dismissLabel }}
      </button>
    </article>
  </section>
</template>

<style scoped>
.toast-region {
  position: fixed;
  z-index: var(--z-toast, 80);
  inset-block-start: calc(var(--header-height, 56px) + var(--space-3, 0.75rem));
  inset-inline-end: var(--space-4, 1rem);
  display: grid;
  gap: var(--space-2, 0.5rem);
  inline-size: min(22rem, calc(100vw - 2rem));
  pointer-events: none;
}

.toast-region__item {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-3, 0.75rem);
  padding: var(--space-3, 0.75rem);
  border: 1px solid var(--color-border-strong, #363d49);
  border-inline-start-width: 3px;
  border-radius: var(--radius-md, 8px);
  background: var(--color-bg-elevated, #242932);
  box-shadow: var(--shadow-md, 0 4px 14px rgb(0 0 0 / 38%));
  color: var(--color-text-primary, #e8eaed);
  pointer-events: auto;
}

.toast-region__item--info {
  border-inline-start-color: var(--color-status-info, #6ea8e8);
}

.toast-region__item--success {
  border-inline-start-color: var(--color-status-success, #5fbf8a);
}

.toast-region__item--warning {
  border-inline-start-color: var(--color-status-warning, #e0b25e);
}

.toast-region__item--error {
  border-inline-start-color: var(--color-status-error, #e4736f);
}

.toast-region__content {
  min-inline-size: 0;
}

.toast-region__title {
  display: block;
  margin-block-end: var(--space-1, 0.25rem);
}

.toast-region__message {
  margin: 0;
  color: var(--color-text-secondary, #a4abb6);
}

.toast-region__dismiss {
  flex: 0 0 auto;
  min-block-size: var(--control-height-sm, 2rem);
  padding: 0 var(--space-2, 0.5rem);
  border: 1px solid var(--color-border-base, #262b34);
  border-radius: var(--radius-xs, 4px);
  background: transparent;
  color: var(--color-text-secondary, #a4abb6);
  cursor: pointer;
}

.toast-region__dismiss:focus-visible {
  outline: 2px solid var(--color-accent, #d8a24a);
  outline-offset: 2px;
}
</style>
