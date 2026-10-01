<script setup lang="ts">
import OrchesterMark from './OrchesterMark.vue'

withDefaults(
  defineProps<{
    title?: string
    description?: string
  }>(),
  {
    title: 'What should we orchestrate?',
    description: 'Choose a project or describe the next task for your agents.',
  },
)
</script>

<template>
  <section class="empty-workspace" data-empty-workspace aria-labelledby="empty-workspace-title">
    <OrchesterMark :size="64" />
    <h1 id="empty-workspace-title">{{ title }}</h1>
    <p>{{ description }}</p>
  </section>
</template>

<style scoped>
.empty-workspace {
  display: grid;
  min-block-size: 0;
  flex: 1;
  align-content: center;
  justify-items: center;
  gap: var(--space-3);
  padding: var(--space-6);
  color: var(--color-text-secondary);
  text-align: center;
}

.empty-workspace h1,
.empty-workspace p {
  margin: 0;
}

.empty-workspace h1 {
  color: var(--color-text-primary);
  max-inline-size: 32rem;
  font-size: var(--text-xl);
  font-weight: var(--weight-medium);
  letter-spacing: -0.02em;
}

.empty-workspace p {
  max-inline-size: 34rem;
  font-size: var(--text-base);
  line-height: var(--leading-relaxed);
}
</style>
