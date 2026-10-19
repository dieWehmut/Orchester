<script setup lang="ts">
import { LoaderCircle, Pencil, X } from '@lucide/vue'
import type { QueuedPrompt } from '../../stores/run-queue'
import { useI18n } from '../../i18n'
defineProps<{ items: readonly QueuedPrompt[]; paused: boolean }>()
const emit = defineEmits<{ remove: [id: string]; edit: [id: string]; resume: [] }>()
const { t } = useI18n()
</script>

<template>
  <section v-if="items.length" class="queued-prompts" data-queued-prompts :aria-label="t('queue.title')">
    <header><span role="status">{{ t(paused ? 'queue.paused' : 'queue.count', { count: String(items.length) }) }}</span><button v-if="paused" type="button" data-queue-resume @click="emit('resume')">{{ t('queue.resume') }}</button></header>
    <ol>
      <li v-for="item in items" :key="item.id" :data-queued-id="item.id" :data-queue-status="item.status">
        <LoaderCircle v-if="item.status === 'sending'" :size="14" class="queued-prompts__sending" :aria-label="t('run.starting')" />
        <span class="queued-prompts__text" :title="item.prompt">{{ item.prompt }}</span>
        <button type="button" :disabled="item.status === 'sending'" :aria-label="t('queue.edit')" :title="t('queue.edit')" @click="emit('edit', item.id)"><Pencil :size="14" /></button>
        <button type="button" :disabled="item.status === 'sending'" :aria-label="t('queue.remove')" :title="t('queue.remove')" @click="emit('remove', item.id)"><X :size="15" /></button>
      </li>
    </ol>
  </section>
</template>

<style scoped>
.queued-prompts { box-sizing: border-box; inline-size: min(calc(100% - var(--space-12)), var(--composer-max-width, 54rem)); margin-inline: auto; padding: 10px 14px; border: 1px solid var(--color-border-subtle); border-radius: 16px; background: var(--color-bg-surface); font-size: 13px; }
header { display: flex; align-items: center; justify-content: space-between; color: var(--color-text-tertiary); font-size: 12px; }
ol { list-style: none; padding: 0; margin: 4px 0 0; max-block-size: 144px; overflow-y: auto; }
li { display: flex; align-items: center; gap: 8px; min-block-size: 30px; }
.queued-prompts__text { flex: 1; min-inline-size: 0; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
button { display: inline-flex; align-items: center; justify-content: center; min-inline-size: 30px; min-block-size: 30px; border: 0; border-radius: 6px; background: transparent; color: var(--color-text-secondary); cursor: pointer; font: inherit; }
button:hover { background: var(--color-bg-element); }
button:disabled { opacity: .35; cursor: default; }
button:focus-visible { outline: 2px solid var(--color-border-focus); outline-offset: 2px; }
.queued-prompts__sending { animation: queue-spin 1.2s linear infinite; }
@keyframes queue-spin { to { transform: rotate(360deg); } }
@media (prefers-reduced-motion: reduce) { .queued-prompts__sending { animation: none; } }
@media (max-width: 640px) { .queued-prompts { inline-size: calc(100% - var(--space-4)); } }
</style>
