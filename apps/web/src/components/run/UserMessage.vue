<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from '../../i18n'
import MessageActions from './MessageActions.vue'

const props = defineProps<{ text: string; timestamp?: number | string | undefined }>()
const emit = defineEmits<{ edit: [text: string] }>()
const { t, locale } = useI18n()
const expanded = ref(false)
const long = computed(() => props.text.length > 700 || props.text.split('\n').length > 12)
watch(() => props.text, () => { expanded.value = false })
const date = computed(() => {
  if (props.timestamp === undefined) return null
  const value = new Date(props.timestamp)
  return Number.isFinite(value.getTime()) ? value : null
})
const time = computed(() => date.value ? new Intl.DateTimeFormat(locale.value, { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' }).format(date.value) : '')
</script>

<template>
  <div class="user-message" data-user-message>
    <div class="user-message__bubble">
      <p :class="{ collapsed: long && !expanded }" data-message-plain>{{ text }}</p>
      <button v-if="long" type="button" :aria-expanded="expanded" @click="expanded = !expanded">{{ expanded ? t('transcript.showLess') : t('transcript.showMore') }}</button>
    </div>
    <div class="user-message__meta">
      <time v-if="date" :datetime="date.toISOString()" :title="date.toLocaleString(locale)" data-message-time>{{ time }}</time>
      <MessageActions :text="text" :label="t('transcript.copyMessage')" :copied-label="t('transcript.copied')" :actions="[{ id: 'edit', label: t('transcript.edit'), icon: 'edit' }]" @action="emit('edit', text)" />
    </div>
  </div>
</template>

<style scoped>
/* A compact, right-aligned bubble keeps the reader's turn distinct from prose. */
.user-message { display: flex; flex-direction: column; align-items: flex-end; inline-size: 100%; min-inline-size: 0; }
.user-message__bubble { max-inline-size: min(85%, 40rem); margin: 0; padding: var(--space-3) calc(var(--space-1) * 4.5); border-radius: 22px; background: var(--color-bg-element); color: var(--color-text-primary); white-space: pre-wrap; overflow-wrap: anywhere; line-height: 1.65; }
.user-message__meta { display: flex; align-items: center; gap: var(--space-3); margin-block-start: calc(var(--space-1) * 1.75); color: var(--color-text-tertiary); font-size: 12px; font-variant-numeric: tabular-nums; }
.user-message__bubble p { margin: 0; }
.user-message__bubble p.collapsed { max-block-size: 18em; overflow: hidden; }
.user-message__bubble button { border: 0; background: transparent; color: inherit; padding: calc(var(--space-1) * 2.5) 0 0; cursor: pointer; font: inherit; font-weight: 600; }
.user-message__bubble button:focus-visible { outline: 2px solid var(--color-border-focus); outline-offset: 2px; }
:global([data-theme='dark'] .user-message__bubble) { background: color-mix(in srgb, var(--color-accent) 55%, var(--color-bg-element)); }
@media (max-width: 640px) { .user-message__bubble { max-inline-size: 92%; } }
</style>
