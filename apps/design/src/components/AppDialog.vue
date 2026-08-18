<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

let nextDialogId = 0

const props = withDefaults(
  defineProps<{
    open?: boolean
    title: string
    description?: string
    closeLabel?: string
    closeOnOverlay?: boolean
  }>(),
  {
    open: false,
    closeLabel: 'Close',
    closeOnOverlay: true,
  },
)

const emit = defineEmits<{
  'update:open': [value: boolean]
  close: []
}>()

const dialog = ref<HTMLElement | null>(null)
const closeButton = ref<HTMLButtonElement | null>(null)
const openState = ref(props.open)
const titleId = 'app-dialog-title-' + ++nextDialogId
const descriptionId = 'app-dialog-description-' + nextDialogId
const dialogAttributes = computed<Record<string, string>>(() =>
  props.description ? { 'aria-describedby': descriptionId } : {},
)
let previousFocus: HTMLElement | null = null
let previousBodyOverflow = ''
let active = false

function focusableElements(): HTMLElement[] {
  return Array.from(
    dialog.value?.querySelectorAll<HTMLElement>(
      'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
    ) ?? [],
  )
}

function focusInitial() {
  void nextTick(() => {
    const elements = focusableElements()
    ;(closeButton.value ?? elements[0])?.focus()
  })
}

function activate() {
  if (active) {
    return
  }

  const current = document.activeElement
  previousFocus = current instanceof HTMLElement ? current : null
  previousBodyOverflow = document.body.style.overflow
  document.body.style.overflow = 'hidden'
  active = true
  focusInitial()
}

function restoreFocus() {
  const target = previousFocus
  previousFocus = null
  if (target?.isConnected) {
    void nextTick(() => target.focus())
  }
}

function deactivate() {
  if (!active) {
    return
  }

  active = false
  document.body.style.overflow = previousBodyOverflow
  previousBodyOverflow = ''
  restoreFocus()
}

function setOpen(value: boolean) {
  if (openState.value === value) {
    return
  }

  openState.value = value
  if (value) {
    activate()
  } else {
    deactivate()
    emit('close')
  }
  emit('update:open', value)
}

function onBackdropMousedown(event: MouseEvent) {
  if (props.closeOnOverlay && event.target === event.currentTarget) {
    setOpen(false)
  }
}

function onDialogKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    setOpen(false)
    return
  }

  if (event.key !== 'Tab') {
    return
  }

  const elements = focusableElements()
  if (elements.length === 0) {
    event.preventDefault()
    dialog.value?.focus()
    return
  }

  const currentIndex = elements.indexOf(document.activeElement as HTMLElement)
  const nextIndex = event.shiftKey
    ? currentIndex <= 0
      ? elements.length - 1
      : currentIndex - 1
    : currentIndex === elements.length - 1
      ? 0
      : currentIndex + 1

  event.preventDefault()
  elements[nextIndex]?.focus()
}

watch(
  () => props.open,
  (value) => {
    if (value === openState.value) {
      return
    }

    openState.value = value
    if (value) {
      activate()
    } else {
      deactivate()
    }
  },
)

onMounted(() => {
  if (openState.value) {
    activate()
  }
})

onBeforeUnmount(() => {
  deactivate()
})
</script>

<template>
  <div
    v-if="openState"
    class="app-dialog__backdrop"
    @mousedown="onBackdropMousedown"
  >
    <section
      ref="dialog"
      class="app-dialog"
      role="dialog"
      aria-modal="true"
      :aria-labelledby="titleId"
      v-bind="dialogAttributes"
      tabindex="-1"
      @keydown="onDialogKeydown"
    >
      <header class="app-dialog__header">
        <h2 :id="titleId" class="app-dialog__title">{{ title }}</h2>
        <button
          ref="closeButton"
          class="app-dialog__close"
          data-dialog-close
          type="button"
          :aria-label="closeLabel"
          @click="setOpen(false)"
        >
          {{ closeLabel }}
        </button>
      </header>

      <p v-if="description" :id="descriptionId" class="app-dialog__description">
        {{ description }}
      </p>

      <div class="app-dialog__body">
        <slot />
      </div>

      <footer v-if="$slots.footer" class="app-dialog__footer">
        <slot name="footer" />
      </footer>
    </section>
  </div>
</template>

<style scoped>
.app-dialog__backdrop {
  position: fixed;
  z-index: var(--z-dialog, 60);
  inset: 0;
  display: grid;
  place-items: center;
  padding: var(--space-4, 1rem);
  background: var(--color-bg-overlay, rgb(8 9 11 / 72%));
}

.app-dialog {
  display: flex;
  flex-direction: column;
  inline-size: min(32rem, 100%);
  max-block-size: min(40rem, 100%);
  overflow: auto;
  border: 1px solid var(--color-border-strong, #363d49);
  border-radius: var(--radius-md, 8px);
  background: var(--color-bg-elevated, #242932);
  color: var(--color-text-primary, #e8eaed);
  box-shadow: var(--shadow-lg, 0 16px 44px rgb(0 0 0 / 46%));
}

.app-dialog__header,
.app-dialog__footer {
  display: flex;
  align-items: center;
  gap: var(--space-3, 0.75rem);
  padding: var(--space-4, 1rem);
}

.app-dialog__header {
  justify-content: space-between;
  border-block-end: 1px solid var(--color-border-base, #262b34);
}

.app-dialog__title {
  margin: 0;
  font-size: var(--text-lg, 1.1875rem);
}

.app-dialog__close {
  flex: 0 0 auto;
  min-block-size: var(--control-height-sm, 2rem);
  padding: 0 var(--space-2, 0.5rem);
  border: 1px solid var(--color-border-base, #262b34);
  border-radius: var(--radius-sm, 6px);
  background: transparent;
  color: var(--color-text-secondary, #a4abb6);
  cursor: pointer;
}

.app-dialog__close:focus-visible,
.app-dialog__footer :deep(button):focus-visible {
  outline: 2px solid var(--color-accent, #d8a24a);
  outline-offset: 2px;
}

.app-dialog__description,
.app-dialog__body {
  padding-inline: var(--space-4, 1rem);
}

.app-dialog__description {
  margin-block: var(--space-4, 1rem) 0;
  color: var(--color-text-secondary, #a4abb6);
}

.app-dialog__body {
  padding-block: var(--space-4, 1rem);
}

.app-dialog__footer {
  justify-content: flex-end;
  border-block-start: 1px solid var(--color-border-base, #262b34);
}
</style>
