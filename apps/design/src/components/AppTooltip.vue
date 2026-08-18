<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'

let nextTooltipId = 0

const props = withDefaults(
  defineProps<{
    content: string
    delay?: number
    placement?: 'top' | 'right' | 'bottom' | 'left'
    disabled?: boolean
  }>(),
  {
    delay: 120,
    placement: 'top',
    disabled: false,
  },
)

const shown = ref(false)
const reducedMotion = ref(false)
const tooltipId = 'app-tooltip-' + ++nextTooltipId
const accessibilityAttributes = computed<Record<string, string>>(() =>
  shown.value ? { 'aria-describedby': tooltipId } : {},
)
let showTimer: ReturnType<typeof setTimeout> | undefined
let motionQuery: MediaQueryList | undefined

function clearShowTimer() {
  if (showTimer !== undefined) {
    clearTimeout(showTimer)
    showTimer = undefined
  }
}

function show() {
  if (props.disabled || shown.value) {
    return
  }

  clearShowTimer()
  if (props.delay <= 0) {
    shown.value = true
    return
  }

  showTimer = setTimeout(() => {
    showTimer = undefined
    if (!props.disabled) {
      shown.value = true
    }
  }, props.delay)
}

function hide() {
  clearShowTimer()
  shown.value = false
}

function onFocusout(event: FocusEvent) {
  const nextTarget = event.relatedTarget
  if (!(nextTarget instanceof Node) || !(event.currentTarget as HTMLElement).contains(nextTarget)) {
    hide()
  }
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    hide()
  }
}

function onMotionPreferenceChange(event: MediaQueryListEvent) {
  reducedMotion.value = event.matches
}

onMounted(() => {
  if (typeof window.matchMedia !== 'function') {
    return
  }

  motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)')
  reducedMotion.value = motionQuery.matches
  motionQuery.addEventListener('change', onMotionPreferenceChange)
})

onBeforeUnmount(() => {
  motionQuery?.removeEventListener('change', onMotionPreferenceChange)
  motionQuery = undefined
})

onBeforeUnmount(clearShowTimer)
</script>

<template>
  <span
    class="app-tooltip"
    :class="[
      'app-tooltip--' + placement,
      { 'app-tooltip--reduced-motion': reducedMotion },
    ]"
    v-bind="accessibilityAttributes"
    @pointerenter="show"
    @pointerleave="hide"
    @focus.capture="show"
    @focusin="show"
    @focusout="onFocusout"
    @keydown="onKeydown"
  >
    <slot name="trigger" :tooltip-id="tooltipId" :open="shown">{{ content }}</slot>
    <span v-if="shown" :id="tooltipId" class="app-tooltip__content" role="tooltip">
      {{ content }}
    </span>
  </span>
</template>

<style scoped>
.app-tooltip {
  position: relative;
  display: inline-flex;
  max-inline-size: 100%;
}

.app-tooltip__content {
  position: absolute;
  z-index: var(--z-popover, 30);
  max-inline-size: min(20rem, 80vw);
  padding: 0.375rem 0.5rem;
  border: 1px solid var(--color-border-strong, #394149);
  border-radius: 4px;
  background: var(--color-bg-elevated, #242932);
  box-shadow: var(--shadow-sm, 0 2px 8px rgb(0 0 0 / 24%));
  color: var(--color-text-primary, #e8eaed);
  font-size: var(--text-xs, 0.6875rem);
  line-height: 1.4;
  pointer-events: none;
  white-space: normal;
  transition: opacity var(--transition-fast, 140ms) var(--ease-out, ease-out);
}

.app-tooltip--reduced-motion .app-tooltip__content {
  transition-duration: 1ms;
}

.app-tooltip--top .app-tooltip__content {
  inset-block-end: calc(100% + 0.375rem);
  inset-inline-start: 50%;
  transform: translateX(-50%);
}

.app-tooltip--right .app-tooltip__content {
  inset-block-start: 50%;
  inset-inline-start: calc(100% + 0.375rem);
  transform: translateY(-50%);
}

.app-tooltip--bottom .app-tooltip__content {
  inset-block-start: calc(100% + 0.375rem);
  inset-inline-start: 50%;
  transform: translateX(-50%);
}

.app-tooltip--left .app-tooltip__content {
  inset-block-start: 50%;
  inset-inline-end: calc(100% + 0.375rem);
  transform: translateY(-50%);
}

@media (prefers-reduced-motion: reduce) {
  .app-tooltip__content {
    transition-duration: 1ms;
  }
}
</style>
