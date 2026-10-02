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
    <OrchesterMark :size="48" />
    <h1 id="empty-workspace-title">{{ title }}</h1>
    <p>{{ description }}</p>
  </section>
</template>

<style scoped>
.empty-workspace {
  display: grid;
  min-block-size: min-content;
  flex: 1;
  align-content: safe center;
  justify-items: center;
  gap: var(--space-4);
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
  font-size: clamp(22px, 2.2vw, 28px);
  font-weight: var(--weight-normal);
  letter-spacing: -0.02em;
}

.empty-workspace p {
  max-inline-size: 34rem;
  font-size: var(--text-sm);
  line-height: var(--leading-relaxed);
}
</style>
