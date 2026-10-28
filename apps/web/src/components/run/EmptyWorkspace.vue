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
    <OrchesterMark :size="40" class="empty-workspace__mark" />
    <h1 id="empty-workspace-title">{{ title }}</h1>
    <p>{{ description }}</p>
  </section>
</template>

<style scoped>
/* The reference opens a new conversation with a line of type and a field, not
   with a hero: the greeting is set at the transcript's own reading size and
   the mark above it is a small signature rather than a badge. Everything here
   is centred on the same axis the composer below it is centred on. */
.empty-workspace {
  display: grid;
  min-block-size: min-content;
  flex: 1;
  align-content: safe center;
  justify-items: center;
  gap: var(--space-3);
  padding: var(--space-6);
  color: var(--color-text-secondary);
  text-align: center;
}

.empty-workspace__mark {
  margin-block-end: var(--space-1);
  opacity: 0.9;
}

.empty-workspace h1,
.empty-workspace p {
  margin: 0;
}

.empty-workspace h1 {
  color: var(--color-text-primary);
  max-inline-size: 30rem;
  font-size: 22px;
  font-weight: var(--weight-medium);
  letter-spacing: -0.01em;
  line-height: var(--leading-tight);
}

.empty-workspace p {
  max-inline-size: 30rem;
  color: var(--color-text-tertiary);
  font-size: var(--text-sm);
  line-height: var(--leading-relaxed);
}
</style>
