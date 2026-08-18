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
  }>(),
  {
    open: false,
    align: 'start',
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
  <div ref="root" class="app-menu" :class="'app-menu--' + align">
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
  align-items: center;
  justify-content: center;
  min-block-size: var(--control-height-md, 2.25rem);
  padding: 0 0.625rem;
  border: 1px solid var(--color-border, #394149);
  border-radius: 6px;
  background: var(--color-surface, #15191d);
  color: var(--color-text, #f2f4f5);
  cursor: pointer;
}

.app-menu__trigger:focus-visible,
.app-menu__item:focus-visible {
  outline: 2px solid var(--color-focus, #e6a23c);
  outline-offset: 2px;
}

.app-menu__list {
  position: absolute;
  z-index: var(--z-popover, 30);
  inset-block-start: calc(100% + 0.25rem);
  min-inline-size: 10rem;
  padding: 0.25rem;
  border: 1px solid var(--color-border, #394149);
  border-radius: 6px;
  background: var(--color-surface-raised, #20262c);
  box-shadow: 0 10px 24px rgb(0 0 0 / 22%);
}

.app-menu--end .app-menu__list {
  inset-inline-end: 0;
}

.app-menu__item {
  display: block;
  inline-size: 100%;
  padding: 0.5rem 0.625rem;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--color-text, #f2f4f5);
  text-align: start;
  cursor: pointer;
}

.app-menu__item:hover:not(:disabled),
.app-menu__item:focus-visible {
  background: var(--color-surface-hover, #2a323a);
}

.app-menu__item:disabled {
  color: var(--color-text-muted, #89929b);
  cursor: not-allowed;
  opacity: 0.6;
}
</style>
