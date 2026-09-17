<script setup lang="ts">
let nextEmptyStateId = 0

withDefaults(
  defineProps<{
    title: string
    description?: string
    actionLabel?: string
  }>(),
  {},
)

const emit = defineEmits<{
  action: []
}>()

const titleId = 'empty-state-title-' + ++nextEmptyStateId
</script>

<template>
  <section
    class="empty-state"
    role="status"
    aria-live="polite"
    :aria-labelledby="titleId"
  >
    <div v-if="$slots.visual" class="empty-state__visual" aria-hidden="true">
      <slot name="visual" />
    </div>
    <h2 :id="titleId" class="empty-state__title">{{ title }}</h2>
    <p v-if="description" class="empty-state__description">{{ description }}</p>
    <div v-if="$slots.action || actionLabel" class="empty-state__action">
      <slot name="action">
        <button class="empty-state__button" type="button" @click="emit('action')">
          {{ actionLabel }}
        </button>
      </slot>
    </div>
  </section>
</template>

<style scoped>
.empty-state {
  display: grid;
  justify-items: center;
  gap: var(--space-2, 0.5rem);
  inline-size: 100%;
  padding: var(--space-8, 2rem) var(--space-4, 1rem);
  color: var(--color-text-secondary, #a4abb6);
  text-align: center;
}

.empty-state__visual {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-inline-size: 2rem;
  min-block-size: 2rem;
  color: var(--color-text-tertiary, #6d7581);
}

.empty-state__title {
  margin: 0;
  color: var(--color-text-primary, #e8eaed);
  font-size: var(--text-md, 1rem);
  font-weight: var(--weight-semibold, 600);
}

.empty-state__description {
  max-inline-size: 32rem;
  margin: 0;
}

.empty-state__action {
  margin-block-start: var(--space-2, 0.5rem);
}

.empty-state__button {
  min-block-size: var(--control-height-md, 2.25rem);
  padding: 0 var(--space-3, 0.75rem);
  border: 1px solid var(--color-accent-border, #80602f);
  border-radius: var(--radius-sm, 6px);
  background: var(--color-accent-muted, rgb(216 162 74 / 16%));
  color: var(--color-accent, #d8a24a);
  cursor: pointer;
}

.empty-state__button:focus-visible {
  outline: 2px solid var(--color-accent, #d8a24a);
  outline-offset: 2px;
}
</style>
