<script setup lang="ts">
import { computed, ref } from 'vue'
import { AppButton, AppDialog } from '@orchester/design'
import { ArrowUpRight, Minus, PencilLine, Redo2, Square, Trash2, Type, Undo2 } from '@lucide/vue'
import { useI18n } from '../../i18n'
import { makeSketchAttachment, sketchPath, type ComposerAttachment, type SketchPoint, type SketchStroke } from './sketch'
const props = defineProps<{ attachmentError?: string }>()
const emit = defineEmits<{ close: []; attach: [attachment: ComposerAttachment] }>()
const { t } = useI18n()
const mode = ref<SketchStroke['kind']>('pen')
const color = ref('#202020')
const text = ref('')
const history = ref<SketchStroke[][]>([[]])
const index = ref(0)
const strokes = computed(() => history.value[index.value] ?? [])
const pending = ref<SketchStroke | null>(null)
const surface = ref<SVGSVGElement | null>(null)
let pointer: number | null = null
const tools = [{ id: 'pen', icon: PencilLine }, { id: 'line', icon: Minus }, { id: 'arrow', icon: ArrowUpRight }, { id: 'rect', icon: Square }, { id: 'text', icon: Type }] as const
const preview = computed(() => pending.value ? [...strokes.value, pending.value] : strokes.value)
const attachment = computed(() => makeSketchAttachment(strokes.value, t('add.sketchName')))
const tooLarge = computed(() => attachment.value.content.length > 18000)
function position(event: PointerEvent): SketchPoint {
  const rect = surface.value!.getBoundingClientRect()
  return { x: Math.round(Math.max(0, Math.min(900, (event.clientX - rect.left) / rect.width * 900))), y: Math.round(Math.max(0, Math.min(500, (event.clientY - rect.top) / rect.height * 500))) }
}
function push(value: SketchStroke[]): void { history.value = [...history.value.slice(0, index.value + 1), value].slice(-101); index.value = history.value.length - 1 }
function start(event: PointerEvent): void {
  if (pointer !== null || event.button !== 0 || strokes.value.length >= 100 || (mode.value === 'text' && !text.value.trim())) return
  pointer = event.pointerId
  surface.value?.setPointerCapture(pointer)
  pending.value = { kind: mode.value, color: color.value, points: [position(event)], ...(mode.value === 'text' ? { text: text.value.trim() } : {}) }
}
function move(event: PointerEvent): void {
  if (!pending.value || pointer !== event.pointerId || pending.value.kind === 'text') return
  const point = position(event)
  if (pending.value.kind === 'pen') {
    const last = pending.value.points.at(-1)!
    if (pending.value.points.length < 800 && Math.hypot(point.x - last.x, point.y - last.y) >= 3) pending.value.points.push(point)
  } else pending.value.points = [pending.value.points[0]!, point]
}
function finish(event: PointerEvent): void {
  if (pointer !== event.pointerId) return
  move(event)
  if (pending.value) push([...strokes.value, pending.value])
  pending.value = null; pointer = null
}
function cancel(): void { pending.value = null; pointer = null }
</script>
<template>
  <AppDialog :open="true" :title="t('add.draw')" :description="t('add.drawHelp')" :close-label="t('management.close')" @close="emit('close')">
    <div class="sketch-toolbar" role="toolbar" :aria-label="t('add.draw')">
      <button v-for="tool in tools" :key="tool.id" type="button" :aria-label="t(`add.${tool.id}`)" :title="t(`add.${tool.id}`)" :aria-pressed="mode === tool.id" @click="mode = tool.id"><component :is="tool.icon" :size="18" /></button>
      <input v-model="color" type="color" :aria-label="t('add.color')" />
      <button type="button" :disabled="index === 0" :aria-label="t('add.undo')" @click="index--"><Undo2 :size="18" /></button>
      <button type="button" :disabled="index === history.length - 1" :aria-label="t('add.redo')" @click="index++"><Redo2 :size="18" /></button>
      <button type="button" :disabled="!strokes.length" :aria-label="t('add.clear')" @click="push([])"><Trash2 :size="18" /></button>
    </div>
    <label v-if="mode === 'text'" class="sketch-label">{{ t('add.textHelp') }}<input v-model="text" maxlength="120" :aria-label="t('add.text')" /></label>
    <svg ref="surface" class="sketch-surface" viewBox="0 0 900 500" role="img" :aria-label="t('add.canvas')" @pointerdown.prevent="start" @pointermove.prevent="move" @pointerup="finish" @pointercancel="cancel" @lostpointercapture="cancel">
      <template v-for="(stroke, key) in preview" :key="key">
        <text v-if="stroke.kind === 'text'" :x="stroke.points[0]?.x ?? 0" :y="stroke.points[0]?.y ?? 0" :fill="stroke.color" font-size="24" font-family="sans-serif">{{ stroke.text }}</text>
        <path v-else :d="sketchPath(stroke)" fill="none" :stroke="stroke.color" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" />
      </template>
    </svg>
    <p v-if="tooLarge" role="alert">{{ t('add.attachmentLimit') }}</p>
    <p v-if="props.attachmentError" role="alert">{{ props.attachmentError }}</p>
    <template #footer><AppButton :disabled="!strokes.length || tooLarge || !!pending" @click="emit('attach', attachment)">{{ t('add.attachDrawing') }}</AppButton></template>
  </AppDialog>
</template>
<style scoped>
.sketch-toolbar { display: flex; align-items: center; flex-wrap: wrap; gap: 5px; margin-block-end: 12px; }
.sketch-toolbar button { display: grid; place-items: center; width: 34px; height: 34px; padding: 5px; border: 0; border-radius: 8px; color: var(--color-text-primary); background: transparent; cursor: pointer; }
.sketch-toolbar button:hover, .sketch-toolbar button[aria-pressed='true'] { background: var(--color-bg-element); }
.sketch-toolbar button:disabled { opacity: .3; cursor: default; }
.sketch-toolbar input { width: 32px; height: 30px; padding: 0; border: 0; background: transparent; }
.sketch-surface { display: block; width: 100%; background: white; border: 1px solid var(--color-border-base); border-radius: 10px; cursor: crosshair; touch-action: none; }
.sketch-label { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; font-size: 12px; margin-block-end: 12px; }
.sketch-label input { padding: 6px; border: 1px solid var(--color-border-control); border-radius: 6px; background: var(--color-bg-base); color: var(--color-text-primary); }
</style>
