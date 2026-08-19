import type { SessionDetailDto, SessionSummaryDto } from '@orchester/protokoll'
import { ref, shallowRef, type Ref } from 'vue'

import { normalizeApiError, type ApiError } from '../api/errors'
import type { SessionsApi } from '../api/sessions'

export type SessionsStatus = 'idle' | 'loading' | 'refreshing' | 'loading_more' | 'ready' | 'error'
export type DetailStatus = 'idle' | 'loading' | 'ready' | 'error'

export interface SessionsStore {
  status: Ref<SessionsStatus>
  detailStatus: Ref<DetailStatus>
  items: Ref<SessionSummaryDto[]>
  nextCursor: Ref<string | null>
  selectedId: Ref<string | null>
  selected: Ref<SessionDetailDto | null>
  error: Ref<ApiError | null>
  detailError: Ref<ApiError | null>
  load: () => Promise<void>
  loadMore: () => Promise<void>
  select: (id: string | null) => Promise<void>
  reset: () => void
}

function mergeUnique(
  current: SessionSummaryDto[],
  incoming: readonly SessionSummaryDto[],
): SessionSummaryDto[] {
  const byId = new Map(current.map((item) => [item.id, item]))
  for (const item of incoming) byId.set(item.id, item)
  return [...byId.values()]
}

export function createSessionsStore(api: SessionsApi): SessionsStore {
  const status = ref<SessionsStatus>('idle')
  const detailStatus = ref<DetailStatus>('idle')
  const items = ref<SessionSummaryDto[]>([])
  const nextCursor = ref<string | null>(null)
  const selectedId = ref<string | null>(null)
  const selected = shallowRef<SessionDetailDto | null>(null)
  const error = shallowRef<ApiError | null>(null)
  const detailError = shallowRef<ApiError | null>(null)
  let listGeneration = 0
  let detailGeneration = 0

  async function load(): Promise<void> {
    const generation = ++listGeneration
    status.value = items.value.length > 0 ? 'refreshing' : 'loading'
    error.value = null
    try {
      const page = await api.list()
      if (generation !== listGeneration) return
      items.value = mergeUnique([], page.items)
      nextCursor.value = page.next_cursor
      status.value = 'ready'
    } catch (cause) {
      if (generation !== listGeneration) return
      error.value = normalizeApiError(cause)
      status.value = 'error'
    }
  }

  async function loadMore(): Promise<void> {
    if (!nextCursor.value || status.value === 'loading_more') return
    const generation = ++listGeneration
    const cursor = nextCursor.value
    status.value = 'loading_more'
    error.value = null
    try {
      const page = await api.list({ cursor })
      if (generation !== listGeneration) return
      items.value = mergeUnique(items.value, page.items)
      nextCursor.value = page.next_cursor
      status.value = 'ready'
    } catch (cause) {
      if (generation !== listGeneration) return
      error.value = normalizeApiError(cause)
      status.value = 'error'
    }
  }

  async function select(id: string | null): Promise<void> {
    const generation = ++detailGeneration
    selectedId.value = id
    detailError.value = null
    if (!id) {
      selected.value = null
      detailStatus.value = 'idle'
      return
    }
    detailStatus.value = 'loading'
    try {
      const detail = await api.detail(id)
      if (generation !== detailGeneration) return
      selected.value = detail
      detailStatus.value = 'ready'
    } catch (cause) {
      if (generation !== detailGeneration) return
      detailError.value = normalizeApiError(cause)
      detailStatus.value = 'error'
    }
  }

  function reset(): void {
    listGeneration += 1
    detailGeneration += 1
    status.value = 'idle'
    detailStatus.value = 'idle'
    items.value = []
    nextCursor.value = null
    selectedId.value = null
    selected.value = null
    error.value = null
    detailError.value = null
  }

  return {
    status,
    detailStatus,
    items,
    nextCursor,
    selectedId,
    selected,
    error,
    detailError,
    load,
    loadMore,
    select,
    reset,
  }
}
