<script setup lang="ts">
/**
 * Reasoning, behind a disclosure.
 *
 * The agent thinking out loud is useful exactly when a result looks wrong and
 * noise the rest of the time, so it stays collapsed and says how much is
 * waiting behind the toggle. Opening it is then a choice made with
 * information rather than a guess.
 */
import { ChevronRight } from '@lucide/vue'
import { computed, ref } from 'vue'

const props = withDefaults(
  defineProps<{
    text?: string
    label?: string
    expandLabel?: string
    collapseLabel?: string
  }>(),
  {
    text: '',
    label: 'Reasoning',
    expandLabel: 'Show the reasoning',
    collapseLabel: 'Hide the reasoning',
  },
)

const expanded = ref(false)
const hasReasoning = computed(() => props.text.trim().length > 0)
const characterCount = computed(() => props.text.trim().length)
</script>

<template>
  <div v-if="hasReasoning" class="reasoning" data-reasoning-disclosure>
    <button
      class="reasoning__toggle"
      type="button"
      data-reasoning-toggle
      :aria-expanded="expanded"
      :aria-label="expanded ? collapseLabel : expandLabel"
      @click="expanded = !expanded"
    >
      <ChevronRight class="reasoning__chevron" :size="13" aria-hidden="true" />
      <span>{{ label }}</span>
      <span class="reasoning__summary" data-reasoning-summary>
        {{ characterCount }} characters
      </span>
    </button>
    <p v-if="expanded" class="reasoning__body" data-reasoning-body>{{ props.text }}</p>
  </div>
</template>

<style scoped>
.reasoning {
  display: grid;
  gap: var(--space-1);
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
}

.reasoning__toggle {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  border: 0;
  background: none;
  color: inherit;
  cursor: pointer;
  font: inherit;
}

.reasoning__toggle[aria-expanded='true'] .reasoning__chevron {
  transform: rotate(90deg);
}

.reasoning__summary {
  opacity: 0.8;
}

.reasoning__body {
  margin: 0;
  padding-inline-start: var(--space-4);
  color: var(--color-text-secondary);
  line-height: var(--leading-normal);
  white-space: pre-wrap;
}
</style>

