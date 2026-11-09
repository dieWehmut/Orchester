import { mount, flushPromises } from '@vue/test-utils'
import { describe, expect, it, afterEach, vi } from 'vitest'
import { createAppStores } from '../src/stores/app'
import { emptyLibrary, sessionPreference } from '../src/stores/library'
import type { HttpClient } from '../src/api/http'
import WorkspaceSidebar from '../src/components/layout/WorkspaceSidebar.vue'
import SessionListItem from '../src/components/sessions/SessionListItem.vue'
import { PINNED_SESSIONS_STORAGE_KEY, resetPinnedSessionsForTests } from '../src/composables/use-pinned-sessions'
const cleanups: (() => void)[] = []
afterEach(() => { cleanups.splice(0).forEach(clean => clean()); document.body.innerHTML = ''; localStorage.clear(); resetPinnedSessionsForTests() })
function setup(fail = false, initial = emptyLibrary()) {
 const http = { get: vi.fn(async path => { if (path.startsWith('/sessions/')) throw new Error('Unknown conversation'); return emptyLibrary() }), put: vi.fn(async (_path, state) => { if (fail) throw new Error('offline'); return { ...state, revision: state.revision + 1 } }) } as unknown as HttpClient
 const stores = createAppStores({ http, agentStatusStreamFactory: null })
 const row = { id: 's-11111111111111111111111111111111', source: 'delegate' as const, title: 'Original', agent: 'codex', model: null, outcome: 'success' as const, resumable: true, recorded_at_unix: 1700000000 }
 stores.sessions.items.value = [row]; stores.library.state.value = initial; stores.library.ready.value = true
 const wrapper = mount(WorkspaceSidebar, { attachTo: document.body, global: { plugins: [stores] }, props: { productName: 'Orchester', workspaceName: 'test', sessionStatus: 'ready', sessions: [row], selectedSessionId: null, nextCursor: null, sessionError: null, agentStatus: 'idle', agentStreamStatus: 'idle', agentSnapshot: null, agentError: null, selectedAgentId: null } })
 cleanups.push(() => { wrapper.unmount(); stores.stop() })
 return { wrapper, stores, row }
}
async function command(wrapper: ReturnType<typeof setup>['wrapper'], id: string, action: string) {
 wrapper.findComponent(SessionListItem).vm.$emit('action', { id, action }); await flushPromises()
}
describe('sidebar library integration', () => {
 it('migrates legacy pins even when project preferences are already saved', async () => {
  const id = 's-11111111111111111111111111111111'
  const unknown = 's-99999999999999999999999999999999'
  localStorage.setItem(PINNED_SESSIONS_STORAGE_KEY, JSON.stringify([id, unknown])); resetPinnedSessionsForTests()
  const state = emptyLibrary(); state.revision = 1; state.projects.push({ id: 'p1', name: 'Existing project', folders: ['/existing'] })
  const { stores, wrapper } = setup(false, state)
  await flushPromises()
  expect(stores.library.metadata.value[id]?.pinned).toBe(true)
  expect(stores.library.state.value.projects).toEqual(state.projects)
  expect(wrapper.findComponent(SessionListItem).props('pinned')).toBe(true)
  expect(stores.library.metadata.value[unknown]).toBeUndefined()
  expect(JSON.parse(localStorage.getItem(PINNED_SESSIONS_STORAGE_KEY)!)).toEqual([id, unknown])
 })
 it('saves a renamed row through the dialog', async () => {
  const { wrapper, stores, row } = setup()
  await command(wrapper, row.id, 'rename')
  const input = document.querySelector<HTMLInputElement>('#library-name')!
  expect(input.value).toBe('Original'); input.value = 'Renamed'; input.dispatchEvent(new Event('input', { bubbles: true }))
  input.closest('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); await flushPromises()
  expect(stores.library.metadata.value[row.id]?.title).toBe('Renamed')
  expect(document.querySelector('#library-name')).toBeNull()
 })
 it('keeps the rename form open on a failed write', async () => {
  const { wrapper, row } = setup(true); await command(wrapper, row.id, 'rename')
  document.querySelector('#library-name')!.closest('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); await flushPromises()
  expect(document.querySelector('#library-name')).not.toBeNull()
  expect(document.querySelector('[role="alert"]')).not.toBeNull()
 })
 it('archives and restores a row while retaining its project and pin', async () => {
  const { wrapper, stores, row } = setup()
  const state = emptyLibrary(); state.projects.push({ id: 'p1', name: 'Project', folders: [] }); Object.assign(sessionPreference(state, row.id), { pinned: true, project_id: 'p1' }); stores.library.state.value = state; await flushPromises()
  expect(wrapper.findAllComponents(SessionListItem)).toHaveLength(1)
  await command(wrapper, row.id, 'archive'); expect(wrapper.findAllComponents(SessionListItem)).toHaveLength(0)
  await wrapper.get('.archive-toggle').trigger('click'); await flushPromises()
  expect(wrapper.findAllComponents(SessionListItem)).toHaveLength(1)
  expect(wrapper.findComponent(SessionListItem).props('pinned')).toBe(true)
  await command(wrapper, row.id, 'archive')
  expect(stores.library.metadata.value[row.id]).toMatchObject({ archived: false, pinned: true, project_id: 'p1' })
  expect(wrapper.findAllComponents(SessionListItem)).toHaveLength(1)
 })
 it('can load older history when the current category has no rows', async () => {
  const { wrapper, stores, row } = setup()
  const state = emptyLibrary(); sessionPreference(state, row.id).archived = true
  stores.library.state.value = state
  await wrapper.setProps({ nextCursor: 'older-page' }); await flushPromises()
  expect(wrapper.findAllComponents(SessionListItem)).toHaveLength(0)
  await wrapper.get('.session-rail__more').trigger('click')
  expect(wrapper.emitted('loadMoreSessions')).toEqual([[]])
 })
 it('explains a search with no matches and keeps older pages reachable', async () => {
  const { wrapper } = setup()
  await wrapper.setProps({ nextCursor: 'older-page' })
  await wrapper.get('[data-rail-action="search"]').trigger('click')
  await wrapper.get('[data-rail-search] input').setValue('Older chat')
  expect(wrapper.findAllComponents(SessionListItem)).toHaveLength(0)
  expect(wrapper.get('[data-session-filter-empty]').text()).toContain('No sessions match')
  expect(wrapper.find('.session-rail__more').exists()).toBe(true)
 })
 it('explains an unmatched search when every loaded chat belongs to another group', async () => {
  const { wrapper, stores, row } = setup()
  const state = emptyLibrary(); sessionPreference(state, row.id).pinned = true
  stores.library.state.value = state
  await wrapper.setProps({ nextCursor: 'older-page' })
  await wrapper.get('[data-rail-action="search"]').trigger('click')
  await wrapper.get('[data-rail-search] input').setValue('Older chat')
  expect(wrapper.findAllComponents(SessionListItem)).toHaveLength(0)
  expect(wrapper.get('[data-session-filter-empty]').text()).toContain('No sessions match')
  await wrapper.get('.session-rail__more').trigger('click')
  expect(wrapper.emitted('loadMoreSessions')).toEqual([[]])
 })
})
