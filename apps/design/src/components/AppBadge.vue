<script setup lang="ts">
/**
 * A small coloured label.
 *
 * `tone` names the meaning, not the colour, so that "running" can stop being
 * blue without a search-and-replace across every view.
 */
type Tone = 'neutral' | 'accent' | 'success' | 'warning' | 'error' | 'info'

withDefaults(defineProps<{ tone?: Tone; mono?: boolean }>(), { tone: 'neutral', mono: false })
</script>

<template>
  <span class="app-badge" :class="[`app-badge--${tone}`, { 'app-badge--mono': mono }]">
    <slot />
  </span>
</template>

<style scoped>
.app-badge {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 1px var(--space-2);
  border: 1px solid currentcolor;
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  font-weight: var(--weight-medium);
  line-height: 1.7;
  white-space: nowrap;
}

.app-badge--mono {
  font-family: var(--font-mono);
}

.app-badge--neutral {
  color: var(--color-text-tertiary);
}

.app-badge--accent {
  color: var(--color-accent);
  background: var(--color-accent-muted);
}

.app-badge--success {
  color: var(--color-status-success);
}

.app-badge--warning {
  color: var(--color-status-warning);
}

.app-badge--error {
  color: var(--color-status-error);
}

.app-badge--info {
  color: var(--color-status-info);
}
</style>
