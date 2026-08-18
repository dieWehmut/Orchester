<script setup lang="ts">
/**
 * A status dot.
 *
 * Never the *only* carrier of the status — colour alone excludes anyone with a
 * colour vision deficiency — so `label` is required and rendered as the
 * accessible name even when the caller shows no text beside it.
 */
type Status = 'idle' | 'running' | 'waiting' | 'success' | 'error'

withDefaults(defineProps<{ status: Status; label: string; pulse?: boolean }>(), { pulse: true })
</script>

<template>
  <span
    class="status-dot"
    :class="[`status-dot--${status}`, { 'status-dot--pulse': pulse && status === 'running' }]"
    role="img"
    :aria-label="label"
    :title="label"
  />
</template>

<style scoped>
.status-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: var(--radius-full);
  background: currentcolor;
  flex: none;
}

.status-dot--idle {
  color: var(--color-status-neutral);
}

.status-dot--running {
  color: var(--color-accent);
}

.status-dot--waiting {
  color: var(--color-status-warning);
}

.status-dot--success {
  color: var(--color-status-success);
}

.status-dot--error {
  color: var(--color-status-error);
}

.status-dot--pulse {
  animation: status-dot-pulse 1.6s var(--ease-in-out) infinite;
}

@keyframes status-dot-pulse {
  0%,
  100% {
    opacity: 1;
    box-shadow: 0 0 0 0 var(--color-glow);
  }

  50% {
    opacity: 0.62;
    box-shadow: 0 0 0 4px var(--color-glow);
  }
}
</style>
