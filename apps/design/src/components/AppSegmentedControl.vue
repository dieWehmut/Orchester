<script setup lang="ts">
import { computed, nextTick, ref } from 'vue'

import type { AppSegmentOption } from './form-types'

const props = defineProps<{
  modelValue: string
  options: readonly AppSegmentOption[]
  ariaLabel: string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const group = ref<HTMLElement | null>(null)
const enabledIndexes = computed(() =>
  props.options.flatMap((option, index) => (option.disabled === true ? [] : [index])),
)
const selectedIndex = computed(() =>
  props.options.findIndex(
    (option) => option.id === props.modelValue && option.disabled !== true,
  ),
)
const rovingIndex = computed(() => selectedIndex.value >= 0 ? selectedIndex.value : (enabledIndexes.value[0] ?? -1))

function focusOption(index: number): void {
  void nextTick(() => {
    group.value?.querySelectorAll<HTMLButtonElement>('[role="radio"]')[index]?.focus()
  })
}

function activate(index: number): void {
  const option = props.options[index]
  if (!option || option.disabled === true) return
  emit('update:modelValue', option.id)
  focusOption(index)
}

function onKeydown(event: KeyboardEvent, index: number): void {
  const enabled = enabledIndexes.value
  const position = enabled.indexOf(index)
  if (position < 0 || enabled.length === 0) return

  let nextPosition: number | null = null
  if (event.key === 'ArrowRight') nextPosition = (position + 1) % enabled.length
  if (event.key === 'ArrowLeft') nextPosition = (position - 1 + enabled.length) % enabled.length
  if (event.key === 'Home') nextPosition = 0
  if (event.key === 'End') nextPosition = enabled.length - 1
  if (nextPosition === null) return

  event.preventDefault()
  const nextIndex = enabled[nextPosition]
  if (nextIndex !== undefined) activate(nextIndex)
}
</script>

<template>
  <div ref="group" class="app-segmented-control" role="radiogroup" :aria-label="ariaLabel">
    <button
      v-for="(option, index) in options"
      :key="option.id"
      class="app-segmented-control__option"
      :class="{ 'app-segmented-control__option--selected': modelValue === option.id }"
      type="button"
      role="radio"
      :aria-checked="modelValue === option.id"
      :disabled="option.disabled === true"
      :tabindex="rovingIndex === index ? 0 : -1"
      @click="activate(index)"
      @keydown="onKeydown($event, index)"
    >
      {{ option.label }}
    </button>
  </div>
</template>

<style scoped>
.app-segmented-control {
  display: inline-flex;
  align-items: stretch;
  max-width: 100%;
  padding: var(--space-1);
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-sm);
  background: var(--color-bg-element);
}

.app-segmented-control__option {
  min-width: 0;
  min-height: var(--control-height-sm);
  padding: var(--space-1) var(--space-3);
  border: 1px solid transparent;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--color-text-secondary);
  font: inherit;
  font-size: var(--text-sm);
  white-space: nowrap;
  cursor: pointer;
}

.app-segmented-control__option:hover:not(:disabled) {
  color: var(--color-text-primary);
}

.app-segmented-control__option--selected {
  border-color: var(--color-border-base);
  background: var(--color-bg-surface);
  box-shadow: var(--shadow-sm);
  color: var(--color-text-primary);
}

.app-segmented-control__option:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}
</style>
