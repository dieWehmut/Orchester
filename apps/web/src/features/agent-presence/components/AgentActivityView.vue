<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'
import { AppMenu, Spinner } from '@orchester/design'
import { ArrowDown, Ellipsis, Folder, MessageCircle, PanelLeft, RefreshCw } from '@lucide/vue'
import { activityNames, type ExternalActivityProvider } from '../../../api/agent-activity'
import type { AgentActivity } from '../use-agent-activity'
import AgentConversation from './AgentConversation.vue'
import { useI18n } from '../../../i18n'
import { runShellAction } from '../../../components/layout/shell-actions'

const props = defineProps<{ provider: ExternalActivityProvider; activity: AgentActivity }>()
const { t } = useI18n()
const { detail, sessions, selectedId, loading, refreshing, error, sourceStatus, refresh, isPinned, togglePin } = props.activity
const transcript = ref<HTMLElement | null>(null)
const title = ref<HTMLElement | null>(null)
const follow = ref(true)
function jumpToLatest(): void { if (transcript.value) transcript.value.scrollTop = transcript.value.scrollHeight; follow.value = true }
function onScroll(): void { const el = transcript.value; if (el) follow.value = el.scrollHeight - el.scrollTop - el.clientHeight < 48 }
function action(id: string): void {
  if (id === 'refresh') refresh()
  else if (id === 'pin' && detail.value) togglePin(detail.value)
}
watch(selectedId, () => { follow.value = true; void nextTick(() => title.value?.focus({ preventScroll: true })) })
watch(detail, async () => { await nextTick(); if (follow.value) jumpToLatest() })
</script>

<template>
  <section class="agent-activity" data-agent-activity-view :aria-label="t('activity.title')" :data-reader-provider="provider">
    <header class="agent-activity__header">
      <button type="button" class="agent-activity__icon" :aria-label="t('activity.sessions')" @click="runShellAction('rail.toggle')"><PanelLeft :size="18" /></button>
      <Folder v-if="detail?.project" class="agent-activity__folder" :size="18" aria-hidden="true" />
      <h1 ref="title" tabindex="-1" :title="detail?.title || ''" data-activity-title>{{ detail?.title || t('activity.title') }}</h1>
      <span class="agent-activity__live" :data-stale="error" :title="t(error ? 'activity.stale' : 'activity.live')" :aria-label="t(error ? 'activity.stale' : 'activity.live')" />
      <AppMenu :label="t('transcript.more')" :items="[...(detail ? [{ id: 'pin', label: t(isPinned(detail) ? 'sessions.unpinAction' : 'sessions.pinAction') }] : []), { id: 'refresh', label: t('management.refresh'), disabled: refreshing }]" align="end" @select="action"><template #trigger><Ellipsis :size="20" /></template></AppMenu>
    </header>
    <p v-if="error" class="agent-activity__notice" role="alert">{{ t('activity.error') }}<button type="button" @click="refresh"><RefreshCw :size="13" />{{ t('sessions.retry') }}</button></p>
    <p v-else-if="sourceStatus === 'unavailable' || sourceStatus === 'partial'" class="agent-activity__notice" role="status">{{ t('activity.storageUnavailable') }}</p>
    <div ref="transcript" class="agent-activity__transcript" data-activity-transcript @scroll="onScroll">
      <div v-if="loading && !detail" class="agent-activity__empty"><Spinner :label="t('management.loading')" /></div>
      <div v-else-if="!sessions.length" class="agent-activity__empty" data-activity-empty><MessageCircle :size="28" /><h2>{{ t('activity.emptyTitle') }}</h2><p>{{ t('activity.emptyDescription', { name: activityNames[provider] }) }}</p></div>
      <template v-else-if="detail">
        <p v-if="detail.truncated" class="agent-activity__hint">{{ t('activity.recentOnly') }}</p>
        <AgentConversation :key="detail.id" :detail="detail" />
        <p v-if="!detail.entries.length" class="agent-activity__hint">{{ t('activity.waitingContent') }}</p>
      </template>
    </div>
    <button v-if="!follow" type="button" class="agent-activity__latest" :aria-label="t('activity.latest')" @click="jumpToLatest"><ArrowDown :size="18" /></button>
    <footer class="agent-activity__hint">{{ t('activity.note') }}</footer>
  </section>
</template>

<style scoped>
.agent-activity { display: flex; position: relative; flex: 1; min-inline-size: 0; min-block-size: 0; flex-direction: column; color: var(--color-text-primary); background: #181818; }
:global([data-theme='light'] .agent-activity) { background: var(--color-bg-surface); }
.agent-activity__header { display: flex; align-items: center; gap: 12px; min-block-size: 52px; padding: 8px 20px; }
.agent-activity__header h1 { flex: 1; min-inline-size: 0; margin: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 15px; font-weight: 600; outline: none; }
.agent-activity__folder { flex-shrink: 0; color: var(--color-text-secondary); }
.agent-activity__icon { display: grid; place-items: center; inline-size: 28px; block-size: 28px; border: 0; border-radius: 6px; background: transparent; color: var(--color-text-secondary); cursor: pointer; }
.agent-activity__header :deep(.app-menu__trigger) { border: 0; padding: 4px; background: transparent; }
.agent-activity__live { inline-size: 5px; block-size: 5px; border-radius: 50%; background: var(--color-text-tertiary); }
.agent-activity__live[data-stale='true'] { background: var(--color-status-warning); }
.agent-activity__transcript { flex: 1; min-block-size: 0; overflow-y: auto; overflow-x: hidden; padding: 24px clamp(16px, 4vw, 48px); overflow-wrap: anywhere; scrollbar-width: thin; scrollbar-color: var(--color-border-base) transparent; }
.agent-activity__empty { display: flex; min-block-size: 240px; justify-content: center; align-items: center; flex-direction: column; gap: 12px; padding: 24px; text-align: center; color: var(--color-text-secondary); }
.agent-activity__empty h2 { font-size: 16px; font-weight: 500; margin: 0; }
.agent-activity__empty p { font-size: 14px; max-inline-size: 420px; line-height: 1.7; }
.agent-activity__notice { display: flex; align-items: center; justify-content: center; gap: 8px; margin: 4px 20px; color: var(--color-status-warning); font-size: 12px; }
.agent-activity__notice button { display: flex; gap: 4px; align-items: center; color: inherit; border: 0; background: none; font: inherit; cursor: pointer; }
.agent-activity__hint { margin: 0; color: var(--color-text-tertiary); font-size: 11px; text-align: center; }
footer.agent-activity__hint { padding: 8px 16px; }
.agent-activity__latest { position: absolute; inset-block-end: 44px; inset-inline-start: calc(50% - 17px); display: grid; place-items: center; inline-size: 34px; block-size: 34px; border: 1px solid var(--color-border-base); border-radius: 50%; background: var(--color-bg-element); color: var(--color-text-primary); cursor: pointer; }
button:focus-visible { outline: 2px solid var(--color-border-focus); outline-offset: 2px; }
@media (max-width: 640px) { .agent-activity__header { padding-inline: 12px; gap: 8px; } .agent-activity__folder { display: none; } }
</style>
