import { describe, expect, it, vi } from 'vitest'
import type { LibraryStateDto } from '@orchester/protokoll'
import { createLibraryStore, emptyLibrary } from '../src/stores/library'
import { HttpResponseError } from '../src/api/http'

const id = 's-11111111111111111111111111111111'
describe('durable conversation organization', () => {
  it('queues edits behind an in-flight refresh so old reads cannot erase a save', async () => {
    let finishRead!: (state: LibraryStateDto) => void
    const api = { load: vi.fn(() => new Promise<LibraryStateDto>(resolve => { finishRead = resolve })), save: vi.fn(async (state: LibraryStateDto) => ({ ...state, revision: state.revision + 1 })) }
    const store = createLibraryStore(api)
    const reading = store.load()
    await Promise.resolve()
    const writing = store.updateSession(id, row => { row.title = 'While loading' })
    await Promise.resolve()
    expect(api.load).toHaveBeenCalledTimes(1)
    expect(api.save).not.toHaveBeenCalled()
    finishRead(emptyLibrary())
    await Promise.all([reading, writing])
    expect(store.metadata.value[id]?.title).toBe('While loading')
    expect(store.state.value.revision).toBe(1)
  })
  it('serializes consecutive edits against the last acknowledged revision', async () => {
    let server = emptyLibrary()
    const revisions: number[] = []
    const store = createLibraryStore({
      load: async () => structuredClone(server),
      save: async state => {
        revisions.push(state.revision)
        expect(state.revision).toBe(server.revision)
        server = { ...state, revision: state.revision + 1 }
        return structuredClone(server)
      },
    })
    await Promise.all([
      store.updateSession(id, row => { row.title = 'Renamed chat' }),
      store.updateSession(id, row => { row.pinned = true }),
      store.updateSession(id, row => { row.archived = true }),
    ])
    expect(revisions).toEqual([0, 1, 2])
    const reopened = createLibraryStore({ load: async () => structuredClone(server), save: vi.fn() })
    await reopened.load()
    expect(reopened.metadata.value[id]).toMatchObject({ title: 'Renamed chat', pinned: true, archived: true })
    expect(store.busy.value).toBe(false)
  })

  it('rebases one conflicting edit without erasing another window’s rename', async () => {
    const concurrent: LibraryStateDto = { ...emptyLibrary(), revision: 1, sessions: [{ id, title: 'Other window', pinned: false, unread: false, archived: false, project_id: null, section_id: null }] }
    const api = {
      load: vi.fn().mockResolvedValueOnce(emptyLibrary()).mockResolvedValueOnce(concurrent),
      save: vi.fn().mockRejectedValueOnce(new HttpResponseError(new Response(null, { status: 409 }), null))
        .mockImplementationOnce(async (value: LibraryStateDto) => ({ ...value, revision: 2 })),
    }
    const store = createLibraryStore(api)
    expect(await store.updateSession(id, row => { row.pinned = !row.pinned })).toBe(true)
    expect(store.metadata.value[id]).toMatchObject({ title: 'Other window', pinned: true })
    expect(api.save).toHaveBeenCalledTimes(2)
  })

  it('keeps acknowledged data on a failed save and permits a retry', async () => {
    const api = { load: async () => emptyLibrary(), save: vi.fn().mockRejectedValueOnce(new TypeError('offline')).mockImplementationOnce(async (state: LibraryStateDto) => ({ ...state, revision: 1 })) }
    const store = createLibraryStore(api)
    expect(await store.updateSession(id, row => { row.unread = true })).toBe(false)
    expect(store.state.value.sessions).toEqual([])
    expect(store.error.value?.code).toBe('network')
    expect(await store.updateSession(id, row => { row.unread = true })).toBe(true)
    expect(store.metadata.value[id]?.unread).toBe(true)
    expect(store.error.value).toBeNull()
  })
})
