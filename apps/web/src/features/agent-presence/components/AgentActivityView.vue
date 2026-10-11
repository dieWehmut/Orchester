<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { AppButton, Spinner } from '@orchester/design'
import { ArrowDown, ChevronDown, RefreshCw, ArrowLeft, MessageCircle } from '@lucide/vue'
import { createAgentActivityApi, activityProviders, type ActivityProvider, type ActivitySummary, type ActivityDetail, type ActivitySource } from '../../../api/agent-activity'
import { useAppStores } from '../../../stores/app'
import { useI18n } from '../../../i18n'

const props = defineProps<{ provider: ActivityProvider }>()
const emit = defineEmits<{ close: []; provider: [provider: ActivityProvider] }>()
const stores = useAppStores()
const { t, locale } = useI18n()
const api = createAgentActivityApi(stores.http)
const names: Record<ActivityProvider, string> = { codex: 'Codex', claude: 'Claude Code', opencode: 'OpenCode', orchester: 'Orchester' }
const sessions = ref<ActivitySummary[]>([])
const sources = ref<ActivitySource[]>([])
const selectedId = ref<string | null>(null)
const detail = ref<ActivityDetail | null>(null)
const loading = ref(true)
const refreshing = ref(false)
const error = ref(false)
const follow = ref(true)
const transcript = ref<HTMLElement | null>(null)
const sourceStatus = computed(() => sources.value.find(source => source.provider === props.provider)?.status)
let generation = 0
let controller: AbortController | null = null
let timer: ReturnType<typeof setTimeout> | null = null
let disposed = false

function clearRequest(): void {
  generation += 1
  controller?.abort()
  if (timer) clearTimeout(timer)
  timer = null
}
function queue(): void { if (!disposed) timer = setTimeout(() => { void load() }, 2000) }
async function load(): Promise<void> {
  if (disposed || refreshing.value || stores.bootstrap.status.value !== 'ready') return
  if (document.visibilityState === 'hidden') { queue(); return }
  if (timer) clearTimeout(timer)
  const current = generation
  controller = new AbortController()
  const signal = controller.signal
  refreshing.value = true
  try {
    const list = await api.list(props.provider, signal)
    if (current !== generation || signal.aborted) return
    sessions.value = list.sessions
    sources.value = list.sources
    if (!list.sessions.some(session => session.id === selectedId.value)) {
      selectedId.value = list.sessions.find(session => session.state === 'active')?.id ?? list.sessions[0]?.id ?? null
      detail.value = null
      follow.value = true
    }
    if (selectedId.value) {
      const next = await api.detail(selectedId.value, signal)
      if (current !== generation || signal.aborted) return
      detail.value = next
      await nextTick()
      if (follow.value) jumpToLatest()
    }
    error.value = false
  } catch {
    if (current === generation && !signal.aborted) error.value = true
  } finally {
    if (current === generation) { loading.value = false; refreshing.value = false; queue() }
  }
}
function refresh(): void { clearRequest(); refreshing.value = false; void load() }
function select(id: string): void {
  clearRequest(); refreshing.value = false; selectedId.value = id; detail.value = null; follow.value = true; loading.value = true; void load()
}
function jumpToLatest(): void { if (transcript.value) transcript.value.scrollTop = transcript.value.scrollHeight; follow.value = true }
function onScroll(): void { const el = transcript.value; if (el) follow.value = el.scrollHeight - el.scrollTop - el.clientHeight < 48 }
function date(value: number): string { return new Intl.DateTimeFormat(locale.value, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' }).format(new Date(value)) }
function visibilityChanged(): void { if (document.visibilityState === 'visible') refresh() }
watch(() => [props.provider, stores.bootstrap.status.value], () => {
  clearRequest(); sessions.value = []; sources.value = []; detail.value = null; selectedId.value = null; loading.value = true; refreshing.value = false; error.value = false; void load()
  if (stores.bootstrap.status.value === 'error') { loading.value = false; error.value = true }
}, { immediate: true })
onMounted(() => document.addEventListener('visibilitychange', visibilityChanged))
onUnmounted(() => { disposed = true; clearRequest(); document.removeEventListener('visibilitychange', visibilityChanged) })
</script>

<template>
  <section class="agent-activity" data-agent-activity-view :aria-label="t('activity.title')">
    <header class="agent-activity__header">
      <button type="button" class="agent-activity__back" :aria-label="t('activity.back')" @click="emit('close')"><ArrowLeft :size="18" /></button>
      <h1>{{ t('activity.title') }}</h1>
      <span class="agent-activity__live" :data-stale="error">{{ t(error ? 'activity.stale' : 'activity.live') }}</span>
      <AppButton variant="ghost" size="sm" :disabled="refreshing" @click="refresh"><RefreshCw :size="15" />{{ t('management.refresh') }}</AppButton>
    </header>
    <nav class="agent-activity__providers" :aria-label="t('agents.title')">
      <button v-for="provider in activityProviders" :key="provider" type="button" :aria-pressed="provider === props.provider" :data-activity-provider="provider" @click="emit('provider', provider)">{{ names[provider] }}</button>
    </nav>
    <p v-if="error" class="agent-activity__notice" role="alert">{{ t('activity.error') }}</p>
    <p v-else-if="sourceStatus === 'unavailable' || sourceStatus === 'partial'" class="agent-activity__notice" role="status">{{ t('activity.storageUnavailable') }}</p>
    <div v-if="loading && !sessions.length" class="agent-activity__empty"><Spinner :label="t('management.loading')" /></div>
    <div v-else-if="!sessions.length" class="agent-activity__empty" data-activity-empty><MessageCircle :size="28" /><h2>{{ t('activity.emptyTitle') }}</h2><p>{{ t('activity.emptyDescription', { name: names[props.provider] }) }}</p></div>
    <div v-else class="agent-activity__body">
      <aside class="agent-activity__sessions" :aria-label="t('activity.sessions')">
        <button v-for="session in sessions" :key="session.id" type="button" :data-activity-session="session.id" :aria-pressed="selectedId === session.id" @click="select(session.id)">
          <span class="agent-activity__session-title">{{ session.title || t('activity.untitled') }}</span>
          <span class="agent-activity__session-meta"><span>{{ session.project || names[session.provider] }}</span><span :data-activity-state="session.state">{{ t(`activity.states.${session.state}`) }}</span></span>
          <span class="agent-activity__preview">{{ session.preview }}</span>
          <time :datetime="new Date(session.updated_at).toISOString()">{{ date(session.updated_at) }}</time>
        </button>
      </aside>
      <section class="agent-activity__reader" :aria-label="t('activity.content')">
        <header v-if="detail" class="agent-activity__identity"><strong>{{ detail.title || t('activity.untitled') }}</strong><span>{{ t(detail.source === 'external' ? 'activity.external' : 'activity.managed') }} · {{ detail.project || names[detail.provider] }}</span></header>
        <div ref="transcript" class="agent-activity__transcript" data-activity-transcript @scroll="onScroll">
          <div v-if="!detail" class="agent-activity__empty"><Spinner v-if="loading || refreshing" :label="t('management.loading')" /><p v-else>{{ t('activity.error') }}</p></div>
          <template v-else>
            <p v-if="detail.truncated" class="agent-activity__hint">{{ t('activity.recentOnly') }}</p>
            <article v-for="entry in detail.entries" :key="entry.id" class="agent-activity__entry" :data-activity-entry="entry.kind">
              <details v-if="entry.kind === 'tool' || entry.kind === 'result'" class="agent-activity__tool"><summary><ChevronDown :size="14" />{{ entry.tool || t(`activity.kinds.${entry.kind}`) }}<span>{{ t(`activity.kinds.${entry.kind}`) }}</span></summary><pre>{{ entry.text }}</pre></details>
              <template v-else><span class="agent-activity__speaker">{{ entry.kind === 'assistant' ? names[props.provider] : t(`activity.kinds.${entry.kind}`) }}</span><p>{{ entry.text }}</p></template>
            </article>
            <p v-if="!detail.entries.length" class="agent-activity__hint">{{ t('activity.waitingContent') }}</p>
          </template>
        </div>
        <button v-if="!follow" type="button" class="agent-activity__latest" @click="jumpToLatest"><ArrowDown :size="15" />{{ t('activity.latest') }}</button>
        <footer class="agent-activity__hint">{{ t('activity.note') }}</footer>
      </section>
    </div>
  </section>
</template>

<style scoped>
.agent-activity { display: flex; flex: 1; min-block-size: 0; flex-direction: column; color: var(--color-text-primary); }
.agent-activity__header { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-4); }
.agent-activity__header h1 { margin: 0; font-size: var(--text-base); font-weight: var(--weight-medium); }
.agent-activity__back { display: grid; place-items: center; background: transparent; border: 0; color: var(--color-text-secondary); min-inline-size: 32px; min-block-size: 32px; cursor: pointer; border-radius: 8px; }
.agent-activity__live { margin-inline-start: auto; font-size: var(--text-xs); color: var(--color-text-tertiary); }
.agent-activity__live[data-stale='true'], .agent-activity__notice { color: var(--color-status-warning); }
.agent-activity__providers { display: flex; gap: var(--space-2); padding: 0 var(--space-4) var(--space-3); border-block-end: 1px solid var(--color-border-subtle); }
.agent-activity__providers button { padding: var(--space-2) var(--space-3); background: transparent; color: var(--color-text-secondary); border: 0; border-radius: 9px; font: inherit; font-size: var(--text-sm); cursor: pointer; }
.agent-activity__providers button[aria-pressed='true'] { background: var(--color-bg-element); color: var(--color-text-primary); }
.agent-activity__body { display: grid; grid-template-columns: minmax(180px, 240px) minmax(0, 1fr); flex: 1; min-block-size: 0; }
.agent-activity__sessions { min-block-size: 0; overflow: auto; padding: var(--space-2); border-inline-end: 1px solid var(--color-border-subtle); }
.agent-activity__sessions button { display: grid; gap: var(--space-2); inline-size: 100%; padding: var(--space-3); text-align: start; border: 0; border-radius: 10px; background: transparent; color: inherit; font: inherit; cursor: pointer; margin-block-end: var(--space-1); }
.agent-activity__sessions button:hover, .agent-activity__sessions button[aria-pressed='true'] { background: var(--color-bg-element); }
.agent-activity__session-title { font-size: var(--text-sm); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.agent-activity__session-meta { display: flex; justify-content: space-between; gap: var(--space-1); font-size: var(--text-xs); color: var(--color-text-tertiary); }
.agent-activity__session-meta > span { min-inline-size: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
[data-activity-state='active'] { color: var(--color-accent); }
.agent-activity__preview { color: var(--color-text-secondary); font-size: var(--text-xs); display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; overflow-wrap: anywhere; }
.agent-activity__sessions time { color: var(--color-text-tertiary); font-size: var(--text-xs); }
.agent-activity__reader { display: flex; min-inline-size: 0; min-block-size: 0; flex-direction: column; position: relative; }
.agent-activity__identity { display: grid; gap: var(--space-1); padding: var(--space-4) var(--space-5); border-block-end: 1px solid var(--color-border-subtle); }
.agent-activity__identity strong { font-size: var(--text-sm); font-weight: var(--weight-medium); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.agent-activity__identity span, .agent-activity__hint, .agent-activity__speaker { font-size: var(--text-xs); color: var(--color-text-tertiary); }
.agent-activity__transcript { flex: 1; min-block-size: 0; overflow: auto; padding: var(--space-5); overflow-wrap: anywhere; }
.agent-activity__entry { margin-block-end: var(--space-5); }
.agent-activity__entry p { white-space: pre-wrap; margin: var(--space-2) 0 0; font-size: var(--text-sm); line-height: 1.7; }
.agent-activity__entry[data-activity-entry='user'] { margin-inline-start: var(--space-5); padding: var(--space-3) var(--space-4); background: var(--color-bg-element); border-radius: 14px; }
.agent-activity__tool { border-inline-start: 2px solid var(--color-border-base); padding-inline-start: var(--space-3); }
.agent-activity__tool summary { display: flex; gap: var(--space-2); align-items: center; color: var(--color-text-secondary); font-size: var(--text-xs); cursor: pointer; min-block-size: 32px; }
.agent-activity__tool summary > span { margin-inline-start: auto; color: var(--color-text-tertiary); }
.agent-activity__tool pre { font-size: var(--text-xs); white-space: pre-wrap; overflow-wrap: anywhere; margin: var(--space-2) 0; line-height: 1.6; }
.agent-activity__empty { display: flex; flex: 1; justify-content: center; align-items: center; flex-direction: column; gap: var(--space-3); padding: var(--space-5); text-align: center; color: var(--color-text-secondary); }
.agent-activity__empty h2 { font-size: var(--text-base); font-weight: var(--weight-medium); margin: 0; }
.agent-activity__empty p { font-size: var(--text-sm); max-inline-size: 420px; line-height: 1.7; }
.agent-activity__notice { margin: var(--space-2) var(--space-4); font-size: var(--text-sm); }
footer.agent-activity__hint { padding: var(--space-2) var(--space-4); text-align: center; }
.agent-activity__latest { position: absolute; inset-block-end: 48px; inset-inline-end: 24px; display: flex; align-items: center; gap: var(--space-2); border: 1px solid var(--color-border-base); border-radius: 18px; background: var(--color-bg-element); color: var(--color-text-primary); padding: var(--space-2) var(--space-3); font: inherit; font-size: var(--text-xs); cursor: pointer; }
button:focus-visible, summary:focus-visible { outline: 2px solid var(--color-border-focus); outline-offset: 2px; }
@media (max-width: 900px) { .agent-activity__body { grid-template-columns: minmax(0, 1fr); grid-template-rows: 150px minmax(0, 1fr); } .agent-activity__sessions { display: flex; border-inline-end: 0; border-block-end: 1px solid var(--color-border-subtle); } .agent-activity__sessions button { min-inline-size: 200px; max-inline-size: 230px; } }
</style>
