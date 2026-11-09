<script setup lang="ts">
import { computed } from 'vue'
import { compactAge, spelledAge } from '../../../lib/relative-time'
import { CircleAlert, LoaderCircle, MessageCircle, Pin } from '@lucide/vue'
import type { ActivitySummary } from '../../../api/agent-activity'
import { useI18n } from '../../../i18n'
const props = defineProps<{ session: ActivitySummary; selected: boolean; pinned: boolean; unread: boolean; nested?: boolean }>()
defineEmits<{ select: [id: string]; pin: [] }>()
const { t, locale } = useI18n()
const age = computed(() => compactAge(props.session.updated_at / 1000, locale.value))
const fullAge = computed(() => spelledAge(props.session.updated_at / 1000, locale.value))
</script>

<template>
  <div class="activity-row" :data-selected="selected" :data-nested="nested" :data-activity-row="session.id">
    <button type="button" class="activity-row__open" :data-activity-session="session.id" :aria-pressed="selected" :title="`${session.title} · ${session.project} · ${t(`activity.states.${session.state}`)}`" @click="$emit('select', session.id)">
      <MessageCircle v-if="pinned && !nested" :size="18" class="activity-row__icon" aria-hidden="true" />
      <span class="activity-row__content"><span class="activity-row__title">{{ session.title || t('activity.untitled') }}</span><span class="activity-row__environment"><span v-if="session.environment && !nested" class="activity-row__origin">{{ session.environment }}</span><span class="activity-row__state" :data-state="session.state">{{ session.state === 'active' ? t('activity.working') : t('activity.inactive') }}</span><time class="activity-row__age" :datetime="new Date(session.updated_at).toISOString()" :title="fullAge">{{ age }}</time></span></span>
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
.activity-row__open { display: flex; align-items: center; gap: calc(var(--space-1) * 2.5); inline-size: 100%; min-inline-size: 0; min-block-size: 36px; padding: calc(var(--space-1) * 1.75) calc(var(--space-1) * 2.5); border: 0; border-radius: inherit; background: transparent; color: var(--color-text-primary); font: inherit; font-size: 14px; cursor: pointer; text-align: start; }
.activity-row[data-nested='true'] .activity-row__open { padding-inline-start: calc(var(--space-1) * 8.5); }
.activity-row__content { flex: 1; display: grid; gap: 2px; min-inline-size: 0; }
.activity-row__title, .activity-row__environment { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.activity-row__environment { display: flex; align-items: center; gap: 8px; color: var(--color-text-tertiary); font-size: 11px; }
.activity-row__origin { min-inline-size: 0; overflow: hidden; text-overflow: ellipsis; }
.activity-row__state, .activity-row__age { flex-shrink: 0; }
.activity-row__icon, .activity-row__running, .activity-row__interrupted, .activity-row__unread { flex-shrink: 0; }
.activity-row__running { color: var(--color-text-secondary); animation: activity-spin 1.2s linear infinite; }
.activity-row__interrupted { color: var(--color-status-warning); }
.activity-row__unread { inline-size: 7px; block-size: 7px; border-radius: 50%; background: var(--color-accent); }
.activity-row__pin { display: grid; place-items: center; position: absolute; inset-inline-end: var(--space-1); inline-size: 28px; block-size: 28px; border: 0; border-radius: 6px; background: var(--activity-selection, var(--color-bg-element)); color: var(--color-text-secondary); opacity: 0; pointer-events: none; cursor: pointer; }
.activity-row:hover .activity-row__pin, .activity-row:focus-within .activity-row__pin { opacity: 1; pointer-events: auto; }
.activity-row:hover .activity-row__open, .activity-row:focus-within .activity-row__open { padding-inline-end: calc(var(--space-1) * 8.5); }
button:focus-visible { outline: 2px solid var(--color-border-focus); outline-offset: -2px; }
@keyframes activity-spin { to { transform: rotate(360deg); } }
@media (prefers-reduced-motion: reduce) { .activity-row__running { animation: none; } }
.activity-row__age { margin-inline-start: auto; font-variant-numeric: tabular-nums; }
.activity-row__state[data-state="active"] { color: var(--color-status-success, var(--color-text-secondary)); }
</style>
