<script setup lang="ts">
/** A bounded surface with an optional titled header. */
withDefaults(defineProps<{ title?: string; padded?: boolean }>(), { padded: true })
</script>

<template>
  <section class="app-card">
    <header v-if="title || $slots.actions" class="app-card__header">
      <h2 v-if="title" class="app-card__title">{{ title }}</h2>
      <div v-if="$slots.actions" class="app-card__actions"><slot name="actions" /></div>
    </header>
    <div class="app-card__body" :class="{ 'app-card__body--padded': padded }"><slot /></div>
  </section>
</template>

<style scoped>
.app-card {
  display: flex;
  flex-direction: column;
  min-width: 0;
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-md);
  background: var(--color-bg-surface);
  box-shadow: var(--shadow-sm);
}

.app-card__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--color-border-base);
}

.app-card__title {
  margin: 0;
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  letter-spacing: 0.02em;
  text-transform: uppercase;
  color: var(--color-text-secondary);
}

.app-card__actions {
  display: flex;
  align-items: center;
  gap: var(--space-1);
}

.app-card__body {
  min-width: 0;
}

.app-card__body--padded {
  padding: var(--space-4);
}
</style>
