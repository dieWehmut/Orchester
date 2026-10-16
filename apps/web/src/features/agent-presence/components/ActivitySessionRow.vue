<script setup lang="ts">
import { CircleAlert, LoaderCircle, MessageCircle, Pin } from '@lucide/vue'
import type { ActivitySummary } from '../../../api/agent-activity'
import { useI18n } from '../../../i18n'
const props = defineProps<{ session: ActivitySummary; selected: boolean; pinned: boolean; unread: boolean; nested?: boolean }>()
defineEmits<{ select: [id: string]; pin: [] }>()
const { t } = useI18n()
</script>

<template>
  <div class="activity-row" :data-selected="selected" :data-nested="nested" :data-activity-row="session.id">
    <button type="button" class="activity-row__open" :data-activity-session="session.id" :aria-pressed="selected" :title="`${session.title} · ${session.project} · ${t(`activity.states.${session.state}`)}`" @click="$emit('select', session.id)">
      <MessageCircle v-if="pinned && !nested" :size="18" class="activity-row__icon" aria-hidden="true" />
      <span class="activity-row__title">{{ session.title || t('activity.untitled') }}</span>
      <LoaderCircle v-if="session.state === 'active'" class="activity-row__running" :size="14" :aria-label="t('activity.states.active')" />
      <CircleAlert v-else-if="session.state === 'interrupted'" class="activity-row__interrupted" :size="14" :aria-label="t('activity.states.interrupted')" />
      <span v-else-if="unread" class="activity-row__unread" :aria-label="t('activity.unread')" />
    </button>
    <button type="button" class="activity-row__pin" :aria-label="pinned ? t('sessions.unpinAction') : t('sessions.pinAction')" :aria-pressed="pinned" :data-activity-pin="session.id" @click="$emit('pin')"><Pin :size="14" /></button>
  </div>
</template>

<style scoped>
.activity-row { position: relative; display: flex; align-items: center; min-inline-size: 0; border-radius: 10px; }
.activity-row:hover, .activity-row[data-selected='true'] { background: var(--activity-selection, var(--workspace-hover, var(--color-bg-element))); }
.activity-row__open { display: flex; align-items: center; gap: 10px; inline-size: 100%; min-inline-size: 0; min-block-size: 36px; padding: 7px 10px; border: 0; border-radius: inherit; background: transparent; color: var(--color-text-primary); font: inherit; font-size: 14px; cursor: pointer; text-align: start; }
.activity-row[data-nested='true'] .activity-row__open { padding-inline-start: 34px; }
.activity-row__title { flex: 1; min-inline-size: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.activity-row__icon, .activity-row__running, .activity-row__interrupted, .activity-row__unread { flex-shrink: 0; }
.activity-row__running { color: var(--color-text-secondary); animation: activity-spin 1.2s linear infinite; }
.activity-row__interrupted { color: var(--color-status-warning); }
.activity-row__unread { inline-size: 7px; block-size: 7px; border-radius: 50%; background: #ee78b1; }
.activity-row__pin { display: grid; place-items: center; position: absolute; inset-inline-end: 4px; inline-size: 28px; block-size: 28px; border: 0; border-radius: 6px; background: var(--activity-selection, var(--color-bg-element)); color: var(--color-text-secondary); opacity: 0; pointer-events: none; cursor: pointer; }
.activity-row:hover .activity-row__pin, .activity-row:focus-within .activity-row__pin { opacity: 1; pointer-events: auto; }
.activity-row:hover .activity-row__open, .activity-row:focus-within .activity-row__open { padding-inline-end: 34px; }
button:focus-visible { outline: 2px solid var(--color-border-focus); outline-offset: -2px; }
@keyframes activity-spin { to { transform: rotate(360deg); } }
@media (prefers-reduced-motion: reduce) { .activity-row__running { animation: none; } }
</style>
