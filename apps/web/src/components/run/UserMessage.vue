<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from '../../i18n'
import MessageActions from './MessageActions.vue'

const props = defineProps<{ text: string; timestamp?: number | string | undefined }>()
const emit = defineEmits<{ edit: [text: string] }>()
const { t, locale } = useI18n()
const date = computed(() => {
  if (props.timestamp === undefined) return null
  const value = new Date(props.timestamp)
  return Number.isFinite(value.getTime()) ? value : null
})
const time = computed(() => date.value ? new Intl.DateTimeFormat(locale.value, { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' }).format(date.value) : '')
</script>

<template>
  <div class="user-message" data-user-message>
    <p class="user-message__bubble" data-message-plain>{{ text }}</p>
    <div class="user-message__meta">
      <time v-if="date" :datetime="date.toISOString()" :title="date.toLocaleString(locale)" data-message-time>{{ time }}</time>
      <MessageActions :text="text" :label="t('transcript.copyMessage')" :copied-label="t('transcript.copied')" :actions="[{ id: 'edit', label: t('transcript.edit'), icon: 'edit' }]" @action="emit('edit', text)" />
    </div>
  </div>
</template>

<style scoped>
.user-message { display: flex; flex-direction: column; align-items: flex-end; inline-size: 100%; min-inline-size: 0; }
.user-message__bubble { max-inline-size: min(85%, 40rem); margin: 0; padding: 12px 18px; border-radius: 22px; background: #843d63; color: #fff; white-space: pre-wrap; overflow-wrap: anywhere; line-height: 1.65; }
.user-message__meta { display: flex; align-items: center; gap: 12px; margin-block-start: 7px; color: var(--color-text-tertiary); font-size: 12px; font-variant-numeric: tabular-nums; }
@media (max-width: 640px) { .user-message__bubble { max-inline-size: 92%; } }
</style>
