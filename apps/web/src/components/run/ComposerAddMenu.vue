<script setup lang="ts">
import { Check, Folder, Lightbulb, Paperclip, PencilLine, Plus, Puzzle, Server, Target } from '@lucide/vue'
import { nextTick, onBeforeUnmount, onMounted, ref, useId, watch } from 'vue'
import AgentIcon from '../../features/agent-presence/components/AgentIcon.vue'
import type { ComposerAddGroup, ComposerAddItem } from './composer-add-menu'

const props = defineProps<{ label: string; groups: readonly ComposerAddGroup[]; anchor: HTMLElement | null; disabled?: boolean }>()
const emit = defineEmits<{ select: [id: string]; 'update:open': [open: boolean] }>()
const icons = { files: Paperclip, workspace: Folder, goal: Target, plan: Lightbulb, draw: PencilLine, server: Server, plugin: Puzzle }
const open = ref(false)
const trigger = ref<HTMLButtonElement | null>(null)
const menu = ref<HTMLElement | null>(null)
const menuId = `composer-add-${useId()}`
const position = ref({ left: '8px', bottom: '8px', width: '320px', maxHeight: '320px' })
let observer: ResizeObserver | undefined

function measure(): void {
  if (!open.value || !props.anchor) return
  const box = props.anchor.getBoundingClientRect()
  const viewportWidth = document.documentElement.clientWidth || window.innerWidth
  const width = Math.min(box.width, viewportWidth - 16)
  position.value = {
    left: `${Math.max(8, Math.min(box.left, viewportWidth - width - 8))}px`,
    bottom: `${Math.max(8, window.innerHeight - box.top + 8)}px`,
    width: `${width}px`,
    maxHeight: `${Math.max(40, Math.min(360, box.top - 16))}px`,
  }
}

function items(): HTMLButtonElement[] {
  return Array.from(menu.value?.querySelectorAll<HTMLButtonElement>('[data-add-action]:not(:disabled)') ?? [])
}

function close(restoreFocus = false): void {
  if (!open.value) return
  open.value = false
  emit('update:open', false)
  if (restoreFocus) void nextTick(() => trigger.value?.focus())
}

async function show(last = false): Promise<void> {
  if (props.disabled) return
  open.value = true
  emit('update:open', true)
  await nextTick()
  measure()
  const enabled = items()
  ;(last ? enabled.at(-1) : enabled[0])?.focus()
}

function choose(item: ComposerAddItem): void {
  if (props.disabled || item.disabled) return
  close()
  trigger.value?.focus()
  emit('select', item.id)
}
function attributes(item: ComposerAddItem): Record<string, string> {
  return item.checked === undefined ? { role: 'menuitem' } : { role: 'menuitemcheckbox', 'aria-checked': String(item.checked) }
}

function onTriggerKeydown(event: KeyboardEvent): void {
  if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return
  event.preventDefault()
  void show(event.key === 'ArrowUp')
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close(true); return }
  if (event.key === 'Tab') {
    event.preventDefault()
    const controls = Array.from(props.anchor?.querySelectorAll<HTMLElement>('button:not(:disabled), textarea:not(:disabled), input:not([type="file"]):not(:disabled), [href]') ?? [])
    const index = controls.indexOf(trigger.value!)
    const next = controls[index + (event.shiftKey ? -1 : 1)] ?? trigger.value
    close()
    void nextTick(() => next?.focus())
    return
  }
  const enabled = items()
  if (!enabled.length) return
  const index = enabled.indexOf(document.activeElement as HTMLButtonElement)
  let next: number
  if (event.key === 'ArrowDown') next = (index + 1) % enabled.length
  else if (event.key === 'ArrowUp') next = (index - 1 + enabled.length) % enabled.length
  else if (event.key === 'Home') next = 0
  else if (event.key === 'End') next = enabled.length - 1
  else return
  event.preventDefault()
  enabled[next]?.focus()
}

function onOutsidePointer(event: PointerEvent): void {
  if (event.target instanceof Node && !menu.value?.contains(event.target) && !trigger.value?.contains(event.target)) close()
}

watch(() => props.disabled, disabled => { if (disabled) close() })
watch([open, () => props.anchor], () => {
  observer?.disconnect()
  if (open.value && props.anchor) {
    measure()
    if (typeof ResizeObserver !== 'undefined') { observer = new ResizeObserver(measure); observer.observe(props.anchor) }
  }
}, { flush: 'post' })
onMounted(() => {
  document.addEventListener('pointerdown', onOutsidePointer)
  window.addEventListener('resize', measure)
  window.addEventListener('scroll', measure, true)
})
onBeforeUnmount(() => {
  observer?.disconnect()
  document.removeEventListener('pointerdown', onOutsidePointer)
  window.removeEventListener('resize', measure)
  window.removeEventListener('scroll', measure, true)
})
defineExpose({ close })
</script>

<template>
  <button ref="trigger" class="composer-add-trigger" type="button" data-composer-attach
    :aria-label="props.label" :title="props.label" aria-haspopup="menu" :aria-expanded="open" :aria-controls="menuId" :disabled="props.disabled"
    @click="open ? close(true) : show()" @keydown="onTriggerKeydown"><Plus :size="23" aria-hidden="true" /></button>
  <Teleport to="body">
    <div v-if="open" :id="menuId" ref="menu" class="composer-add-menu" data-composer-add-menu :style="position" role="menu" :aria-label="props.label" @keydown="onKeydown">
      <section v-for="group in props.groups" :key="group.id" class="composer-add-menu__group" role="group" :aria-labelledby="`${menuId}-${group.id}`">
        <h3 :id="`${menuId}-${group.id}`" class="composer-add-menu__heading">{{ group.label }}</h3>
        <button v-for="item in group.items" :key="item.id" type="button" class="composer-add-menu__item" :data-add-action="item.id" :data-add-icon="item.icon"
          v-bind="attributes(item)" :disabled="item.disabled === true" tabindex="-1" @click="choose(item)">
          <AgentIcon v-if="item.provider" :icon-key="item.provider" />
          <component :is="icons[item.icon]" v-else :size="20" aria-hidden="true" />
          <span class="composer-add-menu__label">{{ item.label }}</span><span v-if="item.description" class="composer-add-menu__description">{{ item.description }}</span>
          <Check v-if="item.checked" class="composer-add-menu__check" :size="17" aria-hidden="true" />
        </button>
        <p v-if="!group.items.length && group.emptyText" class="composer-add-menu__empty">{{ group.emptyText }}</p>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.composer-add-trigger { display: grid; place-items: center; inline-size: 34px; block-size: 34px; flex: 0 0 34px; padding: 0; border: 0; border-radius: 50%; background: transparent; color: var(--color-text-primary); cursor: pointer; }
.composer-add-trigger:hover, .composer-add-trigger[aria-expanded='true'] { background: var(--workspace-hover, var(--color-bg-element)); }
.composer-add-trigger:disabled { opacity: .4; cursor: default; }
.composer-add-trigger:focus-visible { outline: 2px solid var(--color-border-focus); outline-offset: 2px; }
.composer-add-menu { position: fixed; z-index: var(--z-popover, 30); overflow: auto; overscroll-behavior: contain; box-sizing: border-box; padding: 6px; border: 1px solid var(--color-border-control); border-radius: 18px; background: var(--color-bg-elevated, #2c2c2c); color: var(--color-text-primary); box-shadow: 0 8px 28px #0003; scrollbar-width: thin; scrollbar-color: var(--color-border-base) transparent; font: 14px/1.4 var(--font-body); }
.composer-add-menu__group + .composer-add-menu__group { margin-block-start: 8px; }
.composer-add-menu__heading { margin: 4px 8px 6px; font-size: 12px; font-weight: 400; color: var(--color-text-tertiary); }
.composer-add-menu__item { display: flex; align-items: center; gap: 9px; inline-size: 100%; min-block-size: 36px; padding: 7px 10px; border: 0; border-radius: 18px; background: transparent; color: var(--color-text-secondary); text-align: start; font: inherit; cursor: pointer; }
.composer-add-menu__item:hover, .composer-add-menu__item:focus-visible { outline: none; background: var(--workspace-hover, var(--color-bg-element)); color: var(--color-text-primary); }
.composer-add-menu__item:focus-visible { box-shadow: inset 0 0 0 1px var(--color-border-focus); }
.composer-add-menu__item:disabled { opacity: .45; cursor: default; }
.composer-add-menu__item > svg { flex: 0 0 20px; }
.composer-add-menu__item[data-add-icon='draw'] > svg { color: #00a990; }
.composer-add-menu__item :deep(.agent-icon) { inline-size: 20px; block-size: 20px; background: transparent; border: 0; }
.composer-add-menu__label { flex-shrink: 0; }
.composer-add-menu__description { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--color-text-tertiary); }
.composer-add-menu__item > .composer-add-menu__check { margin-inline-start: auto; }
.composer-add-menu__empty { margin: 6px 10px 10px; color: var(--color-text-tertiary); font-size: 12px; }
@media (max-width: 480px) { .composer-add-menu__item { flex-wrap: wrap; gap: 5px 8px; } .composer-add-menu__description { flex-basis: 100%; padding-inline-start: 28px; } }
</style>
