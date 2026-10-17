<script setup lang="ts">
import { Check, ChevronLeft, ChevronRight } from '@lucide/vue'
import { nextTick, onBeforeUnmount, onMounted, ref, useId } from 'vue'
import { useI18n } from '../../i18n'
import type { ModelMenuChoice, ModelMenuGroup } from './model-menu'

const props = defineProps<{
  label: string
  model: string
  effort: string
  groups: ModelMenuGroup[]
  efforts: ModelMenuChoice[]
  stale: boolean
}>()
const emit = defineEmits<{ select: [id: string] }>()
const { t } = useI18n()
type Panel = 'models' | 'efforts'
const id = `model-menu-${useId()}`
const trigger = ref<HTMLButtonElement | null>(null)
const root = ref<HTMLElement | null>(null)
const child = ref<HTMLElement | null>(null)
const open = ref(false)
const panel = ref<Panel | null>(null)
const compact = ref(false)
const rootStyle = ref({ left: '8px', bottom: '8px', width: '284px', maxHeight: '400px' })
const childStyle = ref({ left: '8px', bottom: '8px', width: '484px', maxHeight: '400px' })
let observer: ResizeObserver | undefined

function measure(): void {
  if (!open.value || !trigger.value) return
  const box = trigger.value.getBoundingClientRect()
  const viewport = document.documentElement.clientWidth || window.innerWidth
  const width = Math.min(284, viewport - 16)
  const left = Math.max(8, Math.min(box.right - width, viewport - width - 8))
  const bottom = Math.max(8, window.innerHeight - box.top + 8)
  const height = Math.max(48, Math.min(520, window.innerHeight - bottom - 8))
  const desired = panel.value === 'efforts' ? 240 : 484
  const before = left - 16
  const after = viewport - left - width - 16
  const minimum = panel.value === 'efforts' ? 200 : 300
  compact.value = Math.max(before, after) < minimum
  const childWidth = compact.value ? Math.min(desired, viewport - 16) : Math.min(desired, before >= minimum ? before : after)
  const childLeft = compact.value
    ? Math.max(8, Math.min(box.right - childWidth, viewport - childWidth - 8))
    : before >= minimum ? left - childWidth - 8 : left + width + 8
  rootStyle.value = { left: `${left}px`, bottom: `${bottom}px`, width: `${width}px`, maxHeight: `${height}px` }
  childStyle.value = { left: `${childLeft}px`, bottom: `${bottom}px`, width: `${childWidth}px`, maxHeight: `${height}px` }
}

function buttons(surface: HTMLElement | null): HTMLButtonElement[] {
  return Array.from(surface?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') ?? [])
}

async function show(): Promise<void> {
  open.value = true
  panel.value = null
  measure()
  await nextTick()
  if (!open.value) return
  buttons(root.value)[0]?.focus()
}

function close(restore = false): void {
  open.value = false
  panel.value = null
  if (restore) trigger.value?.focus()
}

async function showPanel(value: Panel, focus = true): Promise<void> {
  panel.value = value
  measure()
  await nextTick()
  if (open.value && panel.value === value && focus) {
    const selected = child.value?.querySelector<HTMLButtonElement>('[aria-checked="true"]:not(:disabled)')
    ;(selected ?? buttons(child.value).find(button => !button.hasAttribute('data-model-back')))?.focus()
  }
}

async function back(): Promise<void> {
  const previous = panel.value
  panel.value = null
  await nextTick()
  root.value?.querySelector<HTMLButtonElement>(`[data-model-section="${previous}"]`)?.focus()
}

function choose(id: string): void {
  close(true)
  emit('select', id)
}

function onKeydown(event: KeyboardEvent, nested: boolean): void {
  if (event.key === 'Escape' || (nested && event.key === 'ArrowLeft')) {
    event.preventDefault()
    event.stopPropagation()
    if (panel.value) void back()
    else close(true)
    return
  }
  if (event.key === 'Tab') { close(true); return }
  if (!nested && event.key === 'ArrowRight') {
    const value = (document.activeElement as HTMLElement)?.dataset.modelSection as Panel | undefined
    if (value) { event.preventDefault(); void showPanel(value) }
    return
  }
  const enabled = buttons(nested ? child.value : root.value)
  const current = enabled.indexOf(document.activeElement as HTMLButtonElement)
  let target: number
  if (event.key === 'ArrowDown') target = (current + 1) % enabled.length
  else if (event.key === 'ArrowUp') target = (current - 1 + enabled.length) % enabled.length
  else if (event.key === 'Home') target = 0
  else if (event.key === 'End') target = enabled.length - 1
  else return
  event.preventDefault()
  enabled[target]?.focus()
}

function outside(event: PointerEvent): void {
  if (event.target instanceof Node && !trigger.value?.contains(event.target) && !root.value?.contains(event.target) && !child.value?.contains(event.target)) close()
}

onMounted(() => {
  document.addEventListener('pointerdown', outside)
  window.addEventListener('resize', measure)
  window.addEventListener('scroll', measure, true)
  if (typeof ResizeObserver !== 'undefined' && trigger.value) {
    observer = new ResizeObserver(measure)
    observer.observe(trigger.value)
  }
})
onBeforeUnmount(() => {
  observer?.disconnect()
  document.removeEventListener('pointerdown', outside)
  window.removeEventListener('resize', measure)
  window.removeEventListener('scroll', measure, true)
})
</script>

<template>
  <div class="model-selection" data-model-picker>
    <button ref="trigger" class="model-selection__trigger" type="button" aria-haspopup="menu"
      :aria-label="props.label" :aria-expanded="open" :aria-controls="id"
      @click="open ? close(true) : show()"
      @keydown.down.prevent="show" @keydown.up.prevent="show" @keydown.esc.prevent="close(true)">
      <slot :open="open" />
    </button>
    <Teleport to="body">
      <div v-if="open" v-show="!compact || !panel" :id="id" ref="root" class="model-selection__surface model-selection__root"
        data-model-root role="menu" :aria-label="props.label" :style="rootStyle" @keydown="onKeydown($event, false)">
        <button v-for="section in (['models', 'efforts'] as const)" :key="section" type="button" role="menuitem" tabindex="-1"
          class="model-selection__row" :class="{ 'is-open': panel === section }" :data-model-section="section"
          aria-haspopup="menu" :aria-expanded="panel === section" :aria-controls="`${id}-${section}`"
          @click="showPanel(section)" @pointerenter="!compact && showPanel(section, false)">
          <span>{{ section === 'models' ? t('run.modelMenuLabel') : t('run.reasoningMenuLabel') }}</span>
          <span class="model-selection__value">{{ section === 'models' ? props.model : props.effort }}</span>
          <ChevronRight :size="16" aria-hidden="true" />
        </button>
      </div>
      <div v-if="open && panel" :id="`${id}-${panel}`" ref="child" class="model-selection__surface model-selection__child"
        :data-model-submenu="panel" :data-compact="compact" role="menu" :aria-label="panel === 'models' ? t('run.modelMenuLabel') : t('run.reasoningMenuLabel')"
        :style="childStyle" @keydown="onKeydown($event, true)">
        <button v-if="compact" class="model-selection__row model-selection__back" data-model-back type="button" role="menuitem" tabindex="-1" @click="back">
          <ChevronLeft :size="16" aria-hidden="true" />{{ t('run.modelMenuBack') }}
        </button>
        <template v-if="panel === 'models'">
          <section v-for="group in props.groups" :key="group.id" class="model-selection__group" role="group" :aria-label="group.label">
            <h3 class="model-selection__heading" aria-hidden="true">{{ group.label }}</h3>
            <button v-for="choice in group.choices" :key="choice.id" class="model-selection__choice" type="button" role="menuitemradio"
              :data-model-choice="choice.id" :aria-checked="choice.checked" :disabled="choice.disabled === true" tabindex="-1"
              :title="[choice.label, choice.description].filter(Boolean).join(' — ')" @click="choose(choice.id)">
              <span class="model-selection__copy"><span class="model-selection__name">{{ choice.label }}</span>
                <span v-if="choice.description" class="model-selection__description">{{ choice.description }}</span></span>
              <Check v-if="choice.checked" :size="18" aria-hidden="true" />
            </button>
          </section>
          <div class="model-selection__actions" role="group">
            <button class="model-selection__row" type="button" role="menuitem" tabindex="-1" @click="choose('configure:')">{{ t('run.modelSettings') }}</button>
            <button v-if="props.stale" class="model-selection__row" type="button" role="menuitem" tabindex="-1" @click="choose('retry:')">{{ t('run.retryModels') }}</button>
          </div>
        </template>
        <template v-else>
          <button v-for="choice in props.efforts" :key="choice.id" class="model-selection__choice" type="button" role="menuitemradio"
            :data-model-choice="choice.id" :aria-checked="choice.checked" tabindex="-1" @click="choose(choice.id)">
            <span class="model-selection__copy">{{ choice.label }}</span><Check v-if="choice.checked" :size="18" aria-hidden="true" />
          </button>
        </template>
        <p class="model-selection__scope" role="none">{{ t('run.selectionScope') }}</p>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.model-selection { display: inline-flex; min-inline-size: 0; }
.model-selection__trigger { min-inline-size: 0; padding: 0; border: 0; border-radius: 16px; background: transparent; color: inherit; font: inherit; cursor: pointer; }
.model-selection__trigger:hover, .model-selection__trigger[aria-expanded='true'] { background: color-mix(in srgb, var(--color-text-primary) 8%, transparent); }
.model-selection__trigger:focus-visible { outline: 2px solid var(--color-border-focus); outline-offset: 2px; }
.model-selection__surface { position: fixed; z-index: var(--z-popover, 30); box-sizing: border-box; overflow: auto; overscroll-behavior: contain; padding: 6px; border: 1px solid var(--color-border-base); border-radius: 12px; background: var(--model-menu-bg, var(--color-bg-elevated)); color: var(--color-text-primary); box-shadow: 0 8px 28px #0003; font: 15px/1.4 var(--font-body); scrollbar-width: thin; scrollbar-color: var(--color-border-base) transparent; }
:global([data-theme='dark']) .model-selection__surface { --model-menu-bg: #2c2c2e; }
.model-selection__row, .model-selection__choice { display: flex; align-items: center; gap: 12px; inline-size: 100%; min-block-size: 44px; padding: 10px; border: 0; border-radius: 9px; background: transparent; color: inherit; text-align: start; font: inherit; cursor: pointer; }
.model-selection__row:hover, .model-selection__row:focus-visible, .model-selection__row.is-open, .model-selection__choice:hover, .model-selection__choice:focus-visible { background: color-mix(in srgb, var(--color-text-primary) 8%, transparent); outline: none; }
.model-selection__row:focus-visible, .model-selection__choice:focus-visible { box-shadow: inset 0 0 0 1px var(--color-border-focus); }
.model-selection__row > svg, .model-selection__choice > svg { flex-shrink: 0; }
.model-selection__row > svg { color: var(--color-text-tertiary); }
.model-selection__value { flex: 1; min-inline-size: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-align: end; color: var(--color-text-secondary); }
.model-selection__group + .model-selection__group { margin-block-start: 14px; }
.model-selection__heading { margin: 6px 10px; font-size: 13px; font-weight: 600; color: var(--color-text-secondary); }
.model-selection__copy { flex: 1; min-inline-size: 0; }
.model-selection__name, .model-selection__description { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.model-selection__name { font-weight: 600; }
.model-selection__description { margin-block-start: 3px; font-size: 13px; color: var(--color-text-secondary); }
.model-selection__choice:disabled { cursor: default; opacity: .5; background: transparent; }
.model-selection__actions { margin-block-start: 8px; padding-block-start: 4px; border-block-start: 1px solid var(--color-border-base); font-size: 13px; color: var(--color-text-secondary); }
.model-selection__actions .model-selection__row { min-block-size: 32px; padding-block: 6px; }
.model-selection__scope { margin: 6px 10px 3px; color: var(--color-text-tertiary); font-size: 11px; }
.model-selection__back { margin-block-end: 4px; color: var(--color-text-secondary); }
</style>
