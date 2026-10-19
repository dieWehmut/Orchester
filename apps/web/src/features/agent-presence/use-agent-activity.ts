import { computed, onMounted, onUnmounted, ref, watch, type Ref } from 'vue'
import { readStored, writeStored } from '@orchester/design/storage'
import { createAgentActivityApi, type ActivityDetail, type ActivitySource, type ActivitySummary, type ExternalActivityProvider } from '../../api/agent-activity'
import { useAppStores } from '../../stores/app'

interface Preferences { pins: Record<string, boolean>; seen: Record<string, number>; projects: string[] }
function preferences(provider: string): Preferences {
  try {
    const value = JSON.parse(readStored(`orchester:activity:${provider}`) ?? '{}')
    return {
      pins: Object.fromEntries(Object.entries(value.pins ?? {}).filter(([, v]) => typeof v === 'boolean')) as Record<string, boolean>,
      seen: Object.fromEntries(Object.entries(value.seen ?? {}).filter(([, v]) => typeof v === 'number' && Number.isFinite(v))) as Record<string, number>,
      projects: Array.isArray(value.projects) ? value.projects.filter((v: unknown) => typeof v === 'string') : [],
    }
  } catch { return { pins: {}, seen: {}, projects: [] } }
}

export function useAgentActivity(provider: Ref<ExternalActivityProvider | null>) {
  const stores = useAppStores()
  const api = createAgentActivityApi(stores.http)
  const sessions = ref<ActivitySummary[]>([])
  const sources = ref<ActivitySource[]>([])
  const selectedId = ref<string | null>(null)
  const detail = ref<ActivityDetail | null>(null)
  const loading = ref(true)
  const refreshing = ref(false)
  const error = ref(false)
  const loadingOlder = ref(false)
  const olderError = ref(false)
  let olderController: AbortController | null = null
  let historyLoaded = false
  const prefs = ref<Preferences>(preferences(provider.value ?? 'codex'))
  const sourceStatus = computed(() => sources.value.find(source => source.provider === provider.value)?.status)
  let generation = 0
  let controller: AbortController | null = null
  let timer: ReturnType<typeof setTimeout> | null = null
  let disposed = false
  let baseline = Object.keys(prefs.value.seen).length === 0
  let detailSnapshot = ''

  function persist(): void { if (provider.value) writeStored(`orchester:activity:${provider.value}`, JSON.stringify(prefs.value)) }
  function isPinned(row: ActivitySummary): boolean { return prefs.value.pins[row.id] ?? row.pinned ?? false }
  function togglePin(row: ActivitySummary): void { prefs.value.pins[row.id] = !isPinned(row); persist() }
  function isUnread(row: ActivitySummary): boolean {
    return selectedId.value !== row.id && prefs.value.seen[row.id] !== undefined && row.updated_at > prefs.value.seen[row.id]!
  }
  function projectKey(row: ActivitySummary): string { return row.project_id ?? `${row.provider}:${row.project}` }
  function toggleProject(key: string): void {
    prefs.value.projects = prefs.value.projects.includes(key) ? prefs.value.projects.filter(value => value !== key) : [...prefs.value.projects, key]
    persist()
  }
  function clearRequest(): void {
    generation += 1; controller?.abort(); olderController?.abort(); loadingOlder.value = false
    if (timer) clearTimeout(timer)
    timer = null
  }
  function queue(): void { if (!disposed && provider.value) timer = setTimeout(() => { void load() }, 2000) }
  async function load(): Promise<void> {
    if (disposed || refreshing.value || !provider.value || stores.bootstrap.status.value !== 'ready') return
    if (document.visibilityState === 'hidden') { queue(); return }
    if (timer) clearTimeout(timer)
    const current = generation
    controller = new AbortController()
    const signal = controller.signal
    refreshing.value = true
    try {
      const list = await api.list(provider.value, signal)
      if (current !== generation || signal.aborted) return
      sessions.value = list.sessions.filter(row => row.provider === provider.value && row.source === 'external')
        .sort((a, b) => b.updated_at - a.updated_at || a.id.localeCompare(b.id))
      sources.value = list.sources
      for (const row of sessions.value) if (prefs.value.seen[row.id] === undefined) prefs.value.seen[row.id] = baseline ? row.updated_at : 0
      baseline = false
      if (!sessions.value.some(row => row.id === selectedId.value)) {
        selectedId.value = sessions.value.find(row => row.state === 'active')?.id ?? sessions.value[0]?.id ?? null
        detail.value = null
        detailSnapshot = ''
        historyLoaded = false
      }
      if (selectedId.value) {
        const next = await api.detail(selectedId.value, signal)
        if (current !== generation || signal.aborted) return
        // A poll is not new output. Avoid rebuilding Markdown and disturbing
        // selections/scroll position until the source content actually changes.
        const snapshot = JSON.stringify(next)
        if (snapshot !== detailSnapshot) {
          if (historyLoaded && detail.value?.id === next.id) {
            // A hidden tab may miss more than one tail window. Bridge back to
            // the visible history before merging so no middle messages vanish.
            const known = new Set(detail.value.entries.map(entry => entry.id))
            const pages = [next]
            let page = next
            while (page.next_before && !page.entries.some(entry => known.has(entry.id))) {
              const before = page.next_before
              page = await api.detail(next.id, signal, before)
              if (current !== generation || signal.aborted) return
              if (page.next_before && page.next_before >= before) throw new Error('History cursor did not advance')
              pages.unshift(page)
            }
            const existing = detail.value
            const entries = new Map(existing.entries.map(entry => [entry.id, entry]))
            const turns = new Map(existing.turns?.map(turn => [turn.id, turn]))
            for (const page of pages) {
              const overlap = page.entries.findIndex(entry => known.has(entry.id))
              for (const entry of page.entries.slice(Math.max(0, overlap))) entries.set(entry.id, entry)
              for (const turn of page.turns ?? []) turns.set(turn.id, turn)
            }
            detail.value = { ...next, entries: [...entries.values()], turns: [...turns.values()], next_before: existing.next_before }
          } else detail.value = next
          detailSnapshot = snapshot
        }
        prefs.value.seen[next.id] = next.updated_at
      }
      persist()
      error.value = false
    } catch { if (current === generation && !signal.aborted) error.value = true }
    finally { if (current === generation) { loading.value = false; refreshing.value = false; queue() } }
  }
  function refresh(): void { clearRequest(); refreshing.value = false; void load() }
  async function loadOlder(): Promise<void> {
    const currentDetail = detail.value
    if (!currentDetail?.next_before || loadingOlder.value) return
    const current = generation
    olderController = new AbortController()
    const signal = olderController.signal
    loadingOlder.value = true; olderError.value = false
    try {
      const previous = await api.detail(currentDetail.id, signal, currentDetail.next_before)
      if (signal.aborted || current !== generation || detail.value?.id !== currentDetail.id) return
      // A simultaneous live refresh wins for overlapping entries and timings.
      const latest = detail.value
      const entries = new Map(previous.entries.map(entry => [entry.id, entry]))
      for (const entry of latest.entries) entries.set(entry.id, entry)
      const turns = new Map(previous.turns?.map(turn => [turn.id, turn]))
      for (const turn of latest.turns ?? []) turns.set(turn.id, turn)
      detail.value = { ...latest, truncated: latest.truncated || previous.truncated, entries: [...entries.values()], turns: [...turns.values()], next_before: previous.next_before }
      historyLoaded = true
    } catch { if (!signal.aborted && current === generation) olderError.value = true }
    finally { if (current === generation) loadingOlder.value = false }
  }
  function select(id: string): void {
    if (id === selectedId.value) return
    clearRequest(); refreshing.value = false; selectedId.value = id; detail.value = null; detailSnapshot = ''; historyLoaded = false; olderError.value = false; loading.value = true; void load()
  }
  function visibilityChanged(): void { if (document.visibilityState === 'visible') refresh() }
  watch(() => [provider.value, stores.bootstrap.status.value], () => {
    clearRequest(); sessions.value = []; sources.value = []; detail.value = null; selectedId.value = null
    detailSnapshot = ''
    historyLoaded = false; olderError.value = false
    loading.value = true; refreshing.value = false; error.value = false
    prefs.value = preferences(provider.value ?? 'codex')
    baseline = Object.keys(prefs.value.seen).length === 0
    if (provider.value) void load()
    if (stores.bootstrap.status.value === 'error') { loading.value = false; error.value = true }
  }, { immediate: true })
  onMounted(() => document.addEventListener('visibilitychange', visibilityChanged))
  onUnmounted(() => { disposed = true; clearRequest(); document.removeEventListener('visibilitychange', visibilityChanged) })
  return { sessions, sources, selectedId, detail, loading, refreshing, error, loadingOlder, olderError, loadOlder, sourceStatus, prefs, isPinned, togglePin, isUnread, projectKey, toggleProject, refresh, select }
}
export type AgentActivity = ReturnType<typeof useAgentActivity>
