<script setup lang="ts">
/**
 * The accent picker: one swatch per scheme, as a radio group.
 *
 * A radio group rather than a row of buttons because exactly one is chosen at a
 * time, which gets arrow-key navigation and the "3 of 4" announcement for free.
 * Labels come from the caller, keyed by `ColorSchemeOption.labelKey`, so a
 * localised app and an unlocalised one can both use it.
 */
import { nextTick } from 'vue'

import { COLOR_SCHEME_OPTIONS, type ColorScheme } from '../theme'
import { useAppearance } from '../composables/useAppearance'

const props = withDefaults(
  defineProps<{
    /** Resolve a `labelKey` to display text. Identity-ish by default. */
    label?: (key: string, id: ColorScheme) => string
    groupLabel?: string
  }>(),
  {
    label: (_key: string, id: ColorScheme) => id,
    groupLabel: 'Accent colour',
  },
)

const { colorScheme, setColorScheme } = useAppearance()

function onKeydown(event: KeyboardEvent, index: number): void {
  const key = event.key
  const lastIndex = COLOR_SCHEME_OPTIONS.length - 1
  let nextIndex: number | null = null

  if (key === 'ArrowRight' || key === 'ArrowDown') nextIndex = index === lastIndex ? 0 : index + 1
  if (key === 'ArrowLeft' || key === 'ArrowUp') nextIndex = index === 0 ? lastIndex : index - 1
  if (key === 'Home') nextIndex = 0
  if (key === 'End') nextIndex = lastIndex
  if (nextIndex === null) return

  const nextOption = COLOR_SCHEME_OPTIONS[nextIndex]
  if (!nextOption) return

  event.preventDefault()
  setColorScheme(nextOption.id)

  const group = event.currentTarget instanceof HTMLElement
    ? event.currentTarget.closest('[role="radiogroup"]')
    : null
  void nextTick(() => {
    group?.querySelectorAll<HTMLButtonElement>('[role="radio"]')[nextIndex]?.focus()
  })
}
</script>

<template>
  <div class="scheme-picker" role="radiogroup" :aria-label="groupLabel">
    <button
      v-for="(option, index) in COLOR_SCHEME_OPTIONS"
      :key="option.id"
      class="scheme-picker__swatch"
      :class="[
        `scheme-picker__swatch--${option.id}`,
        { 'scheme-picker__swatch--active': colorScheme === option.id },
      ]"
      type="button"
      role="radio"
      :aria-checked="colorScheme === option.id"
      :tabindex="colorScheme === option.id ? 0 : -1"
      :title="props.label(option.labelKey, option.id)"
      :aria-label="props.label(option.labelKey, option.id)"
      @click="setColorScheme(option.id)"
      @keydown="onKeydown($event, index)"
    />
  </div>
</template>

<style scoped>
.scheme-picker {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
}

.scheme-picker__swatch {
  width: 16px;
  height: 16px;
  padding: 0;
  border: 2px solid transparent;
  border-radius: var(--radius-full);
  cursor: pointer;
  transition: transform var(--transition-fast) var(--ease-out);
}

.scheme-picker__swatch:hover {
  transform: scale(1.16);
}

.scheme-picker__swatch--active {
  border-color: var(--color-text-primary);
}

/* Literal hues, not tokens: a swatch has to show the colour you would be
   switching *to*, and the tokens always describe the scheme already active. */
.scheme-picker__swatch--amber {
  background: #d8a24a;
}

.scheme-picker__swatch--violet {
  background: #a58bf0;
}

.scheme-picker__swatch--teal {
  background: #4fbfad;
}

.scheme-picker__swatch--rose {
  background: #e88aa4;
}
</style>
