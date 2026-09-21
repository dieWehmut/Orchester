<script setup lang="ts">
/**
 * One of the three theme cards on the appearance screen.
 *
 * The miniature is drawn from literal greys rather than from the live tokens.
 * A preview that inherits the current theme would render the light card dark on
 * a dark desktop — which is exactly the card a user reaches for when the
 * contrast is what they came to fix. Each card therefore carries its own
 * scoped palette and paints itself, and the `system` card paints half of each.
 */
import { computed } from 'vue'
import { Check } from '@lucide/vue'

import type { ThemePreference } from '../theme'

const props = defineProps<{
  value: ThemePreference
  label: string
  selected: boolean
  /** Announced as the radio group's name for this card. */
  groupLabel?: string
}>()

defineEmits<{ select: [value: ThemePreference] }>()

const checkLabel = computed(() => `${props.label} selected`)
</script>

<template>
  <button
    class="theme-card"
    type="button"
    role="radio"
    :aria-checked="selected"
    :aria-label="groupLabel ? `${groupLabel}: ${label}` : label"
    :tabindex="selected ? 0 : -1"
    :data-theme-option="value"
    :data-preview-theme="value"
    @click="$emit('select', value)"
  >
    <span class="theme-card__frame">
      <span
        class="theme-card__pane theme-card__pane--rail"
        data-preview-pane="rail"
        :data-preview-side="value === 'system' ? 'light' : 'dark'"
      >
        <span class="theme-card__dot" />
        <span class="theme-card__dot" />
        <span class="theme-card__dot" />
        <span class="theme-card__rail-line" />
        <span class="theme-card__rail-line theme-card__rail-line--short" />
        <span class="theme-card__rail-line" />
      </span>
      <span class="theme-card__pane theme-card__pane--split" :data-preview-theme-value="value">
        <span class="theme-card__header" data-preview-pane="header">
          <span class="theme-card__light" />
          <span class="theme-card__light" />
          <span class="theme-card__go" />
        </span>
        <span class="theme-card__transcript" data-preview-pane="transcript">
          <span class="theme-card__line" />
          <span class="theme-card__line theme-card__line--mid" />
          <span class="theme-card__line theme-card__line--short" />
        </span>
      </span>
      <span v-if="selected" class="theme-card__check" data-preview-check :title="checkLabel">
        <Check :size="13" :stroke-width="3" aria-hidden="true" />
      </span>
    </span>
    <span class="theme-card__label">{{ label }}</span>
  </button>
</template>

<style scoped>
.theme-card {
  display: grid;
  gap: var(--space-2);
  min-inline-size: 0;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--color-text-secondary);
  font: inherit;
  font-size: var(--text-sm);
  cursor: pointer;
  text-align: center;
}

.theme-card__frame {
  position: relative;
  display: grid;
  grid-template-columns: 4.5rem minmax(0, 1fr);
  block-size: 5.4rem;
  overflow: hidden;
  border: 1px solid var(--color-border-default);
  border-radius: 0.6rem;
  background: #f7f7f8;
  transition:
    border-color var(--transition-fast) var(--ease-out),
    box-shadow var(--transition-fast) var(--ease-out);
}

.theme-card:hover .theme-card__frame {
  border-color: var(--color-border-emphasis);
}

.theme-card[aria-checked='true'] .theme-card__frame {
  border-color: var(--color-accent);
  box-shadow: 0 0 0 1px var(--color-accent);
}

.theme-card:focus-visible {
  outline: none;
}

.theme-card:focus-visible .theme-card__frame {
  outline: 2px solid var(--color-border-focus);
  outline-offset: 2px;
}

.theme-card__pane {
  display: grid;
  align-content: start;
  gap: 5px;
  padding: 8px 6px;
}

.theme-card__pane--rail {
  grid-template-columns: auto auto auto;
  gap: 4px 3px;
  border-inline-end: 1px solid rgb(0 0 0 / 8%);
  background: #ececee;
}

.theme-card__pane--split {
  padding: 0;
  gap: 0;
}

.theme-card__dot {
  inline-size: 5px;
  block-size: 5px;
  border-radius: var(--radius-full);
  background: #c9c9ce;
}

.theme-card__rail-line {
  grid-column: 1 / -1;
  block-size: 5px;
  border-radius: 3px;
  background: #d9d9dd;
}

.theme-card__rail-line--short {
  inline-size: 70%;
}

.theme-card__header {
  display: flex;
  align-items: center;
  gap: 3px;
  padding: 7px 8px;
  background: #fdfdfd;
}

.theme-card__light {
  inline-size: 5px;
  block-size: 5px;
  border-radius: var(--radius-full);
  background: #d7d7db;
}

.theme-card__go {
  inline-size: 5px;
  block-size: 5px;
  border-radius: var(--radius-full);
  background: #6fcf7f;
}

.theme-card__transcript {
  display: grid;
  align-content: start;
  gap: 5px;
  block-size: 100%;
  padding: 9px 10px;
  background: #fff;
}

.theme-card__line {
  block-size: 5px;
  border-radius: 3px;
  background: #e2e2e6;
}

.theme-card__line--mid {
  inline-size: 78%;
}

.theme-card__line--short {
  inline-size: 52%;
}

/* The dark miniature. These are literal, because a preview must show the theme
   it selects rather than the theme that is currently active. */
.theme-card[data-preview-theme='dark'] .theme-card__frame,
.theme-card[data-preview-theme='system'] .theme-card__pane--split {
  background: #17181a;
}

.theme-card[data-preview-theme='dark'] .theme-card__pane--rail,
.theme-card[data-preview-theme='system'] .theme-card__pane--rail--dark {
  border-inline-end-color: rgb(255 255 255 / 10%);
  background: #1d1f22;
}

.theme-card[data-preview-theme='dark'] .theme-card__dot {
  background: #45484d;
}

.theme-card[data-preview-theme='dark'] .theme-card__rail-line {
  background: #35383d;
}

.theme-card[data-preview-theme='dark'] .theme-card__header {
  background: #1b1c1f;
}

.theme-card[data-preview-theme='dark'] .theme-card__light {
  background: #45484d;
}

.theme-card[data-preview-theme='dark'] .theme-card__transcript {
  background: #1f2124;
}

.theme-card[data-preview-theme='dark'] .theme-card__line {
  background: #35383d;
}

/* The system miniature: the rail keeps the light palette above and a clip
   paints the second half in the dark one, which is what "split down the
   middle" means at 5px tall. */
.theme-card[data-preview-theme='system'] .theme-card__pane--rail {
  background: linear-gradient(to right, #ececee 50%, #1d1f22 50%);
}

.theme-card[data-preview-theme='system'] .theme-card__pane--split {
  background: linear-gradient(to right, #fff 50%, #1f2124 50%);
}

.theme-card[data-preview-theme='system'] .theme-card__header {
  background: linear-gradient(to right, #fdfdfd 50%, #1b1c1f 50%);
}

.theme-card__check {
  position: absolute;
  inset-block-start: 6px;
  inset-inline-end: 6px;
  display: grid;
  inline-size: 1.15rem;
  block-size: 1.15rem;
  place-items: center;
  border-radius: var(--radius-full);
  background: var(--color-accent);
  color: var(--color-accent-contrast);
}

.theme-card__label {
  color: var(--color-text-secondary);
  font-size: var(--text-sm);
}

.theme-card[aria-checked='true'] .theme-card__label {
  color: var(--color-text-primary);
}
</style>
