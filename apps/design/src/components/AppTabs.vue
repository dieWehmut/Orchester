<script setup lang="ts">
import { computed, nextTick, ref } from 'vue'

import type { AppTabOption } from './form-types'

const props = withDefaults(
  defineProps<{
    modelValue: string
    tabs: readonly AppTabOption[]
    ariaLabel: string
    orientation?: 'horizontal' | 'vertical'
  }>(),
  { orientation: 'horizontal' },
)

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const tablist = ref<HTMLElement | null>(null)
const enabledIndexes = computed(() =>
  props.tabs.flatMap((tab, index) => (tab.disabled === true ? [] : [index])),
)
const selectedIndex = computed(() =>
  props.tabs.findIndex((tab) => tab.id === props.modelValue && tab.disabled !== true),
)
const rovingIndex = computed(() => {
  if (selectedIndex.value >= 0) return selectedIndex.value
  return enabledIndexes.value[0] ?? -1
})

function focusTab(index: number): void {
  void nextTick(() => {
    tablist.value?.querySelectorAll<HTMLButtonElement>('[role="tab"]')[index]?.focus()
  })
}

function activate(index: number): void {
  const tab = props.tabs[index]
  if (!tab || tab.disabled === true) return
  emit('update:modelValue', tab.id)
  focusTab(index)
}

function onKeydown(event: KeyboardEvent, index: number): void {
  const enabled = enabledIndexes.value
  const position = enabled.indexOf(index)
  if (position < 0 || enabled.length === 0) return

  const forward =
    props.orientation === 'vertical'
      ? event.key === 'ArrowDown'
      : event.key === 'ArrowRight'
  const backward =
    props.orientation === 'vertical' ? event.key === 'ArrowUp' : event.key === 'ArrowLeft'
  let nextPosition: number | null = null

  if (forward) nextPosition = (position + 1) % enabled.length
  if (backward) nextPosition = (position - 1 + enabled.length) % enabled.length
  if (event.key === 'Home') nextPosition = 0
  if (event.key === 'End') nextPosition = enabled.length - 1
  if (nextPosition === null) return

  event.preventDefault()
  const nextIndex = enabled[nextPosition]
  if (nextIndex !== undefined) activate(nextIndex)
}
</script>

<template>
  <div
    ref="tablist"
    class="app-tabs"
    :class="{ 'app-tabs--vertical': orientation === 'vertical' }"
    role="tablist"
    :aria-label="ariaLabel"
    :aria-orientation="orientation"
  >
    <button
      v-for="(tab, index) in tabs"
      :key="tab.id"
      class="app-tabs__tab"
      :class="{ 'app-tabs__tab--selected': modelValue === tab.id }"
      type="button"
      role="tab"
      :aria-selected="modelValue === tab.id"
      :disabled="tab.disabled === true"
      :tabindex="rovingIndex === index ? 0 : -1"
      @click="activate(index)"
      @keydown="onKeydown($event, index)"
    >
      {{ tab.label }}
    </button>
  </div>
</template>

<style scoped>
.app-tabs {
  display: flex;
  align-items: stretch;
  gap: var(--space-1);
  min-width: 0;
  border-bottom: 1px solid var(--color-border-base);
}

.app-tabs--vertical {
  flex-direction: column;
  align-items: stretch;
  border-right: 1px solid var(--color-border-base);
  border-bottom: 0;
}

.app-tabs__tab {
  position: relative;
  min-width: 0;
  min-height: var(--control-height-md);
  padding: var(--space-2) var(--space-3);
  border: 0;
  background: transparent;
  color: var(--color-text-secondary);
  font: inherit;
  font-size: var(--text-sm);
  text-align: left;
  cursor: pointer;
}

.app-tabs__tab::after {
  position: absolute;
  right: var(--space-2);
  bottom: -1px;
  left: var(--space-2);
  height: 2px;
  background: transparent;
  content: '';
}

.app-tabs--vertical .app-tabs__tab::after {
  top: var(--space-2);
  right: -1px;
  bottom: var(--space-2);
  left: auto;
  width: 2px;
  height: auto;
}

.app-tabs__tab:hover:not(:disabled) {
  color: var(--color-text-primary);
}

.app-tabs__tab--selected {
  color: var(--color-accent);
}

.app-tabs__tab--selected::after {
  background: var(--color-accent);
}

.app-tabs__tab:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}
</style>
