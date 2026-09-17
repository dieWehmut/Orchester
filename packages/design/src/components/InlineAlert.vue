<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(
  defineProps<{
    tone?: 'info' | 'success' | 'warning' | 'error'
    title?: string
    dismissible?: boolean
    dismissLabel?: string
  }>(),
  {
    tone: 'info',
    dismissible: false,
    dismissLabel: 'Dismiss',
  },
)

const emit = defineEmits<{
  dismiss: []
}>()

const role = computed(() => (props.tone === 'error' || props.tone === 'warning' ? 'alert' : 'status'))
const live = computed(() => (role.value === 'alert' ? 'assertive' : 'polite'))
</script>

<template>
  <div
    class="inline-alert"
    :class="'inline-alert--' + tone"
    :role="role"
    :aria-live="live"
  >
    <div class="inline-alert__content">
      <strong v-if="title" class="inline-alert__title">{{ title }}</strong>
      <div class="inline-alert__message">
        <slot />
      </div>
    </div>
    <button
      v-if="dismissible"
      class="inline-alert__dismiss"
      type="button"
      :aria-label="dismissLabel"
      @click="emit('dismiss')"
    >
      {{ dismissLabel }}
    </button>
  </div>
</template>

<style scoped>
.inline-alert {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-3, 0.75rem);
  padding: var(--space-3, 0.75rem) var(--space-4, 1rem);
  border: 1px solid var(--color-border-base, #262b34);
  border-inline-start-width: 3px;
  border-radius: var(--radius-sm, 6px);
  background: var(--color-bg-surface, #16191f);
  color: var(--color-text-primary, #e8eaed);
}

.inline-alert--info {
  border-inline-start-color: var(--color-status-info, #6ea8e8);
}

.inline-alert--success {
  border-inline-start-color: var(--color-status-success, #5fbf8a);
}

.inline-alert--warning {
  border-inline-start-color: var(--color-status-warning, #e0b25e);
}

.inline-alert--error {
  border-inline-start-color: var(--color-status-error, #e4736f);
}

.inline-alert__content {
  min-inline-size: 0;
}

.inline-alert__title {
  display: block;
  margin-block-end: var(--space-1, 0.25rem);
}

.inline-alert__message {
  color: var(--color-text-secondary, #a4abb6);
}

.inline-alert__dismiss {
  flex: 0 0 auto;
  min-block-size: var(--control-height-sm, 2rem);
  padding: 0 var(--space-2, 0.5rem);
  border: 1px solid var(--color-border-base, #262b34);
  border-radius: var(--radius-xs, 4px);
  background: transparent;
  color: var(--color-text-secondary, #a4abb6);
  cursor: pointer;
}

.inline-alert__dismiss:focus-visible {
  outline: 2px solid var(--color-accent, #d8a24a);
  outline-offset: 2px;
}
</style>
