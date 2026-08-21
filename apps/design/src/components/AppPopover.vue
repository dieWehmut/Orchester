<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

let nextPopoverId = 0

const props = withDefaults(
  defineProps<{
    open?: boolean
    title: string
    placement?: 'top' | 'right' | 'bottom' | 'left'
    closeOnOutside?: boolean
  }>(),
  {
    open: false,
    placement: 'bottom',
    closeOnOutside: true,
  },
)

const emit = defineEmits<{
  'update:open': [value: boolean]
  close: []
}>()

const root = ref<HTMLElement | null>(null)
const panel = ref<HTMLElement | null>(null)
const openState = ref(props.open)
const popoverId = 'app-popover-' + ++nextPopoverId

function onDocumentPointerdown(event: PointerEvent) {
  const target = event.target
  if (
    props.closeOnOutside &&
    target instanceof Node &&
    !root.value?.contains(target)
  ) {
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

function setOpen(value: boolean) {
  if (openState.value === value) {
    return
  }

  openState.value = value
  syncDocumentListener(value)
  if (!value) {
    emit('close')
  }
  emit('update:open', value)
  if (value) {
    void nextTick(() => panel.value?.focus())
  }
}

function toggle() {
  setOpen(!openState.value)
}

function onPanelKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    setOpen(false)
  }
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
      void nextTick(() => panel.value?.focus())
    }
  },
)

onMounted(() => {
  syncDocumentListener(openState.value)
  if (openState.value) {
    void nextTick(() => panel.value?.focus())
  }
})

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', onDocumentPointerdown)
})
</script>

<template>
  <span ref="root" class="app-popover">
    <span
      class="app-popover__anchor"
      :aria-expanded="openState"
      aria-haspopup="dialog"
      :aria-controls="popoverId"
      @click="toggle"
    >
      <slot name="anchor">{{ title }}</slot>
    </span>

    <section
      v-if="openState"
      :id="popoverId"
      ref="panel"
      class="app-popover__panel"
      :class="'app-popover--' + placement"
      role="dialog"
      :aria-label="title"
      tabindex="-1"
      @keydown="onPanelKeydown"
    >
      <slot />
    </section>
  </span>
</template>

<style scoped>
.app-popover {
  position: relative;
  display: inline-flex;
  max-inline-size: 100%;
}

.app-popover__anchor {
  display: inline-flex;
  max-inline-size: 100%;
}

.app-popover__panel {
  position: absolute;
  z-index: var(--z-popover, 30);
  min-inline-size: 12rem;
  max-inline-size: min(24rem, 90vw);
  padding: var(--space-3, 0.75rem);
  border: 1px solid var(--color-border-strong, #363d49);
  border-radius: var(--radius-md, 8px);
  background: var(--color-bg-elevated, #242932);
  color: var(--color-text-primary, #e8eaed);
  box-shadow: var(--shadow-md, 0 4px 14px rgb(0 0 0 / 38%));
}

.app-popover--bottom {
  inset-block-start: calc(100% + var(--space-2, 0.5rem));
  inset-inline-start: 0;
}

.app-popover--top {
  inset-block-end: calc(100% + var(--space-2, 0.5rem));
  inset-inline-start: 0;
}

.app-popover--right {
  inset-block-start: 0;
  inset-inline-start: calc(100% + var(--space-2, 0.5rem));
}

.app-popover--left {
  inset-block-start: 0;
  inset-inline-end: calc(100% + var(--space-2, 0.5rem));
}

.app-popover__panel:focus-visible {
  outline: 2px solid var(--color-accent, #d8a24a);
  outline-offset: 2px;
}
</style>
