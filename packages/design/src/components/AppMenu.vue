<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import type { AppMenuItem } from './form-types'

let nextMenuId = 0

const props = withDefaults(
  defineProps<{
    label: string
    items: readonly AppMenuItem[]
    open?: boolean
    id?: string
    align?: 'start' | 'end'
    /** Which way the list opens. A menu at the foot of a column opens upward. */
    placement?: 'top' | 'bottom'
  }>(),
  {
    open: false,
    align: 'start',
    placement: 'bottom',
  },
)

const emit = defineEmits<{
  'update:open': [value: boolean]
  select: [id: string]
}>()

const root = ref<HTMLElement | null>(null)
const trigger = ref<HTMLButtonElement | null>(null)
const menu = ref<HTMLElement | null>(null)
const openState = ref(props.open)
const generatedMenuId = 'app-menu-' + ++nextMenuId
const menuId = computed(() => (props.id ? props.id + '-menu' : generatedMenuId))

function enabledItems(): HTMLButtonElement[] {
  return Array.from(menu.value?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]') ?? [])
    .filter((item) => !item.disabled)
}

function focusFirstItem() {
  void nextTick(() => {
    enabledItems()[0]?.focus()
  })
}

function focusTrigger() {
  void nextTick(() => {
    trigger.value?.focus()
  })
}

function onDocumentPointerdown(event: PointerEvent) {
  const target = event.target
  if (target instanceof Node && !root.value?.contains(target)) {
    setOpen(false)
  }
}

function syncDocumentListener(value: boolean) {
  if (value) {
    document.addEventListener('pointerdown', onDocumentPointerdown)
  } else {
    document.removeEventListener('pointerdown', onDocumentPointerdown)
  }
}

function setOpen(value: boolean, announce = true) {
  if (openState.value === value) {
    return
  }

  openState.value = value
  syncDocumentListener(value)
  if (announce) {
    emit('update:open', value)
  }

  if (value) {
    focusFirstItem()
  } else {
    focusTrigger()
  }
}

function toggle() {
  setOpen(!openState.value)
}

function onTriggerKeydown(event: KeyboardEvent) {
  if (event.key === 'ArrowDown' || event.key === 'Enter' || event.key === ' ') {
    event.preventDefault()
    if (!openState.value) {
      setOpen(true)
    } else {
      focusFirstItem()
    }
  } else if (event.key === 'Escape' && openState.value) {
    event.preventDefault()
    setOpen(false)
  }
}

function onMenuKeydown(event: KeyboardEvent) {
  const items = enabledItems()
  if (items.length === 0) {
    return
  }

  const currentIndex = items.indexOf(document.activeElement as HTMLButtonElement)
  let nextIndex = currentIndex

  if (event.key === 'ArrowDown') {
    nextIndex = currentIndex < 0 ? 0 : (currentIndex + 1) % items.length
  } else if (event.key === 'ArrowUp') {
    nextIndex = currentIndex < 0 ? items.length - 1 : (currentIndex - 1 + items.length) % items.length
  } else if (event.key === 'Home') {
    nextIndex = 0
  } else if (event.key === 'End') {
    nextIndex = items.length - 1
  } else if (event.key === 'Escape') {
    event.preventDefault()
    setOpen(false)
    return
  } else {
    return
  }

  event.preventDefault()
  items[nextIndex]?.focus()
}

function selectItem(item: AppMenuItem) {
  if (item.disabled === true) {
    return
  }

  emit('select', item.id)
  setOpen(false)
}

watch(
  () => props.open,
  (value) => {
    if (value === openState.value) {
      return
    }

    openState.value = value
    syncDocumentListener(value)
    if (value) {
      focusFirstItem()
    }
  },
)

onMounted(() => {
  syncDocumentListener(openState.value)
  if (openState.value) {
    focusFirstItem()
  }
})

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', onDocumentPointerdown)
})
</script>

<template>
  <div
    ref="root"
    class="app-menu"
    :class="['app-menu--' + align, 'app-menu--' + placement]"
  >
    <button
      ref="trigger"
      class="app-menu__trigger"
      type="button"
      aria-haspopup="menu"
      :aria-expanded="openState"
      :aria-controls="menuId"
      :aria-label="label"
      @click="toggle"
      @keydown="onTriggerKeydown"
    >
      <slot name="trigger">{{ label }}</slot>
    </button>

    <div
      v-if="openState"
      :id="menuId"
      ref="menu"
      class="app-menu__list"
      role="menu"
      :aria-label="label"
      @keydown="onMenuKeydown"
    >
      <button
        v-for="item in items"
        :key="item.id"
        class="app-menu__item"
        type="button"
        role="menuitem"
        tabindex="-1"
        :disabled="item.disabled === true"
        @click="selectItem(item)"
      >
        {{ item.label }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.app-menu {
  position: relative;
  display: inline-flex;
}

.app-menu__trigger {
  display: inline-flex;
  min-inline-size: var(--hit-target-min, 32px);
  min-block-size: max(var(--hit-target-min, 32px), var(--control-height-md, 2.25rem));
  align-items: center;
  justify-content: center;
  padding: 0 0.625rem;
  border: 1px solid var(--color-border-control);
  border-radius: 6px;
  background: var(--color-bg-element);
  color: var(--color-text-primary);
  cursor: pointer;
}

.app-menu__trigger:focus-visible,
.app-menu__item:focus-visible {
  /* The vocabulary this file used - `--color-border`, `--color-surface`,
     `--color-focus` - is not defined anywhere in the token file, so every one
     of these fell back to a hard-coded hex. They now speak the real roles. */
  outline: 2px solid var(--color-border-focus);
  outline-offset: 2px;
}

.app-menu__list {
  position: absolute;
  z-index: var(--z-popover, 30);
  inset-block-start: calc(100% + 0.25rem);
  min-inline-size: 10rem;
  padding: 0.25rem;
  border: 1px solid var(--color-border-emphasis);
  border-radius: 6px;
  background: var(--color-bg-elevated);
  box-shadow: 0 10px 24px rgb(0 0 0 / 22%);
}

.app-menu--end .app-menu__list {
  inset-inline-end: 0;
}

/* The list is a popover, so the side it opens on is the caller's decision.
   The account row sits at the foot of the rail, and a list that opened
   downward there would be clipped by the window rather than shown. */
.app-menu--top .app-menu__list {
  inset-block-start: auto;
  inset-block-end: calc(100% + 0.25rem);
}

.app-menu__item {
  display: block;
  min-inline-size: var(--hit-target-min, 32px);
  inline-size: 100%;
  min-block-size: var(--hit-target-min, 32px);
  padding: 0.5rem 0.625rem;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--color-text-primary);
  text-align: start;
  cursor: pointer;
}

.app-menu__item:hover:not(:disabled),
.app-menu__item:focus-visible {
  background: var(--color-bg-element);
}

.app-menu__item:disabled {
  color: var(--color-text-disabled);
  cursor: not-allowed;
  opacity: 0.6;
}
</style>
