import type { LibraryStateDto } from '@orchester/protokoll'
import { computed, ref, shallowRef } from 'vue'
import { normalizeApiError, type ApiError } from '../api/errors'
import type { LibraryApi, LibrarySession } from '../api/library'

export function emptyLibrary(): LibraryStateDto {
  return { revision: 0, projects: [], sections: [], sessions: [] }
}
export function sessionPreference(state: LibraryStateDto, id: string): LibrarySession {
  let row = state.sessions.find(item => item.id === id)
  if (!row) {
    row = { id, title: null, pinned: false, unread: false, archived: false, project_id: null, section_id: null }
    state.sessions.push(row)
  }
  return row
}

/** Writes are serialized; a conflict reapplies the user's intent to fresh state. */
export function createLibraryStore(api: LibraryApi) {
  const state = shallowRef<LibraryStateDto>(emptyLibrary())
  const ready = ref(false)
  const pending = ref(0)
  const error = shallowRef<ApiError | null>(null)
  let chain: Promise<unknown> = Promise.resolve()
  const metadata = computed(() => Object.fromEntries(state.value.sessions.map(row => [row.id, row])))
  async function read(): Promise<void> {
    const value = await api.load()
    if (!Number.isSafeInteger(value?.revision) || !Array.isArray(value.projects)
      || !Array.isArray(value.sections) || !Array.isArray(value.sessions)) throw new Error('Invalid library response')
    state.value = value
    ready.value = true
  }
  function load(): Promise<void> {
    const work = chain.then(async () => {
      try { await read(); error.value = null } catch (cause) { error.value = normalizeApiError(cause) }
    })
    chain = work
    return work
  }
  function mutate(change: (draft: LibraryStateDto) => void): Promise<boolean> {
    pending.value++
    const work = chain.then(async () => {
      error.value = null
      try {
        if (!ready.value) await read()
        for (let attempt = 0; attempt < 2; attempt++) {
          const draft = structuredClone(state.value)
          change(draft)
          try { state.value = await api.save(draft); return true } catch (cause) {
            if (attempt === 0 && normalizeApiError(cause).status === 409) { await read(); continue }
            throw cause
          }
        }
        return false
      } catch (cause) { error.value = normalizeApiError(cause); return false }
      finally { pending.value-- }
    })
    chain = work
    return work
  }
  function updateSession(id: string, change: (row: LibrarySession) => void): Promise<boolean> {
    return mutate(state => change(sessionPreference(state, id)))
  }
  return { state, ready, error, metadata, busy: computed(() => pending.value > 0), load, mutate, updateSession }
}
