import { mount, flushPromises } from '@vue/test-utils'
import { afterEach, describe, expect, it } from 'vitest'
import SessionContextMenu from '../src/components/sessions/SessionContextMenu.vue'

const mounted: ReturnType<typeof mount>[] = []
afterEach(() => { mounted.forEach(wrapper => wrapper.unmount()); mounted.length = 0; document.body.innerHTML = '' })
describe('session actions menu', () => {
  it('opens from the keyboard and emits a rename action without selecting a session', async () => {
    const wrapper = mount(SessionContextMenu, { attachTo: document.body, props: { id: 'session-1' } })
    mounted.push(wrapper)
    const trigger = wrapper.get('button')
    await trigger.trigger('keydown', { key: 'Enter' })
    await flushPromises()
    const rename = Array.from(document.querySelectorAll<HTMLElement>('[role="menuitem"]')).find(item => item.textContent?.includes('Rename'))
    expect(rename).toBeDefined()
    rename!.click()
    await flushPromises()
    expect(wrapper.emitted('action')).toEqual([[{ id: 'session-1', action: 'rename' }]])
    expect(document.querySelector('[role="menu"]')).toBeNull()
  })
  it('keeps destinations with the same name independently selectable', async () => {
    const wrapper = mount(SessionContextMenu, { attachTo: document.body, props: { id: 'session-1', projects: [{ id: 'project-1', name: 'Same name' }, { id: 'project-2', name: 'Same name' }] } })
    mounted.push(wrapper)
    await wrapper.get('button').trigger('keydown', { key: 'Enter' })
    await flushPromises()
    const project = Array.from(document.querySelectorAll<HTMLElement>('[role="menuitem"]')).find(item => item.textContent === 'Project')!
    project.focus()
    project.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true, cancelable: true }))
    await flushPromises()
    const destinations = Array.from(document.querySelectorAll<HTMLElement>('[role="menuitem"]')).filter(item => item.textContent === 'Same name')
    expect(destinations).toHaveLength(2)
    destinations[1]!.click()
    await flushPromises()
    expect(wrapper.emitted('action')).toEqual([[{ id: 'session-1', action: 'project', value: 'project-2' }]])
  })
})
