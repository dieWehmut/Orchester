import { mount, flushPromises } from '@vue/test-utils'
import { afterEach, describe, expect, it } from 'vitest'
import { createHttpClient } from '../src/api/http'
import { createAppStores } from '../src/stores/app'
import SchedulesView from '../src/views/SchedulesView.vue'
import PluginsView from '../src/views/PluginsView.vue'
import type { Schedule } from '../src/api/management'

const wrappers: ReturnType<typeof mount>[] = []
afterEach(() => { wrappers.splice(0).forEach(wrapper => wrapper.unmount()); document.body.innerHTML = '' })
const response = (body: unknown, status = 200) => new Response(status === 204 ? null : JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })

describe('local management views', () => {
  it('saves real form values, toggles a task and confirms deletion before requesting it', async () => {
    let tasks: Schedule[] = []
    const calls: { method: string; path: string; body: unknown }[] = []
    const http = createHttpClient({ fetch: async (url, init) => {
      const path = String(url); const method = init?.method ?? 'GET'
      const body = typeof init?.body === 'string' ? JSON.parse(init.body) : null
      calls.push({ method, path, body })
      if (method === 'GET') return response({ schema_version: 1, schedules: tasks })
      if (method === 'POST') { tasks = [{ ...body, id: 'task-1', next_run_at: body.first_run_at, created_at: 1, updated_at: 1, history: [] }]; return response(tasks[0], 201) }
      if (method === 'PUT') { tasks = [{ ...tasks[0], ...body }]; return response(tasks[0]) }
      tasks = []; return response(null, 204)
    } })
    const stores = createAppStores({ http, agentStatusStreamFactory: null }); stores.bootstrap.status.value = 'ready'
    const wrapper = mount(SchedulesView, { attachTo: document.body, global: { plugins: [stores], stubs: { RouterLink: true } } }); wrappers.push(wrapper)
    await flushPromises()
    await wrapper.findAll('button').find(button => button.text() === 'New schedule')!.trigger('click')
    const form = wrapper.get('form')
    await form.get('input[maxlength="80"]').setValue('Review tomorrow')
    await form.get('textarea').setValue('Check the pending changes')
    await form.get('input[type="datetime-local"]').setValue('2030-04-12T09:30')
    await form.get('select').setValue('0')
    await form.trigger('submit'); await flushPromises()
    expect(calls.find(call => call.method === 'POST')?.body).toMatchObject({ name: 'Review tomorrow', prompt: 'Check the pending changes', interval_seconds: null, enabled: true })
    expect(wrapper.get('[data-schedule-id="task-1"]').text()).toContain('Review tomorrow')
    await wrapper.get('[aria-label="Pause schedule"]').trigger('click'); await flushPromises()
    expect(tasks[0]?.enabled).toBe(false)
    await wrapper.get('[aria-label="Delete"]').trigger('click')
    expect(calls.filter(call => call.method === 'DELETE')).toHaveLength(0)
    await wrapper.get('[role="dialog"]').findAll('button').find(button => button.text() === 'Delete')!.trigger('click'); await flushPromises()
    expect(calls.filter(call => call.method === 'DELETE')).toHaveLength(1)
    expect(wrapper.find('[data-schedule-id]').exists()).toBe(false)
  })

  it('installs through the API and protects project-managed plugins from removal', async () => {
    const plugins = [
      { name: 'codex', display_name: 'Codex', package_name: '@orchester/codex', version: '0.1.2', installed: false, origin: null, removable: false },
      { name: 'claude', display_name: 'Claude', package_name: '@orchester/claude', version: '0.1.2', installed: true, origin: 'project', removable: false },
    ]
    const installed: unknown[] = []
    const http = createHttpClient({ fetch: async (url, init) => {
      if (init?.method === 'POST') { installed.push(JSON.parse(String(init.body))); plugins[0]!.installed = true; plugins[0]!.origin = 'managed'; plugins[0]!.removable = true }
      if (String(url).endsWith('/plugins')) return response({ schema_version: 1, plugins })
      return response({ code: 'unavailable' }, 503)
    } })
    const stores = createAppStores({ http, agentStatusStreamFactory: null }); stores.bootstrap.status.value = 'ready'
    const wrapper = mount(PluginsView, { global: { plugins: [stores] } }); wrappers.push(wrapper)
    await flushPromises()
    expect(wrapper.get('[data-plugin-name="claude"]').text()).toContain('Project managed')
    expect(wrapper.get('[data-plugin-name="claude"]').find('button').exists()).toBe(false)
    await wrapper.get('[data-plugin-name="codex"] button').trigger('click'); await flushPromises()
    expect(installed).toEqual([{ name: 'codex' }])
    expect(wrapper.get('[data-plugin-name="codex"]').text()).toContain('Installed')
  })

  it('keeps creation unavailable when the local service cannot load', async () => {
    const stores = createAppStores({ http: createHttpClient({ fetch: async () => response({ code: 'unavailable' }, 503) }) })
    stores.bootstrap.status.value = 'ready'
    const wrapper = mount(SchedulesView, { global: { plugins: [stores], stubs: { RouterLink: true } } }); wrappers.push(wrapper)
    await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toContain('local management')
    expect(wrapper.findAll('button').find(button => button.text() === 'New schedule')!.attributes('disabled')).toBeDefined()
  })
})
