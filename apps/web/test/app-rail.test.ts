import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import AppRail from '../src/components/layout/AppRail.vue'

const props = { productName: 'Orchester', workspaceName: 'default', accountName: 'Orchester', accountHint: 'Local runtime', newSessionLabel: 'New chat', projectsLabel: 'Pinned', sessionsLabel: 'Recent chats', fleetLabel: 'Agents', searchLabel: 'Search chats', attentionLabel: 'Approvals' }
const slots = { projects: '<p>Pinned list</p>', sessions: '<p>Recent list</p>', fleet: '<p>Agent list</p>' }
describe('AppRail', () => {
  it('starts with new chat and removes both branding areas', () => {
    const wrapper = mount(AppRail, { props, slots })
    expect(wrapper.findAll('[data-rail-section]').map(node => node.attributes('data-rail-section'))).toEqual(['primary', 'projects', 'fleet', 'sessions'])
    expect(wrapper.text()).not.toContain('Orchester')
    expect(wrapper.text()).not.toContain('Local runtime')
    expect(wrapper.find('[data-rail-account-menu]').exists()).toBe(false)
    expect(wrapper.find('[data-rail-product]').exists()).toBe(false)
    wrapper.unmount()
  })
  it('starts a new chat', async () => {
    const wrapper = mount(AppRail, { props })
    await wrapper.get('[data-rail-action="new-session"]').trigger('click')
    expect(wrapper.emitted('newSession')).toHaveLength(1)
    wrapper.unmount()
  })
  it('folds lists independently', async () => {
    const wrapper = mount(AppRail, { props, slots })
    await wrapper.get('[data-rail-disclosure="pinned"]').trigger('click')
    expect(wrapper.get('[data-rail-disclosure="pinned"]').attributes('aria-expanded')).toBe('false')
    expect(wrapper.text()).not.toContain('Pinned list')
    expect(wrapper.text()).toContain('Recent list')
    expect(wrapper.text()).toContain('Agent list')
    await wrapper.get('[data-rail-disclosure="pinned"]').trigger('click')
    expect(wrapper.text()).toContain('Pinned list')
    wrapper.unmount()
  })
  it('focuses search and restores focus and unfiltered results on Escape', async () => {
    const wrapper = mount(AppRail, { props, attachTo: document.body })
    const button = wrapper.get('[data-rail-action="search"]')
    await button.trigger('click')
    const input = wrapper.get('input')
    expect(document.activeElement).toBe(input.element)
    await input.setValue('runtime')
    expect(wrapper.emitted('search')?.at(-1)).toEqual(['runtime'])
    await input.trigger('keydown', { key: 'Escape' })
    expect(wrapper.find('input').exists()).toBe(false)
    expect(wrapper.emitted('search')?.at(-1)).toEqual([''])
    expect(document.activeElement).toBe(button.element)
    wrapper.unmount()
  })
  it('shows waiting approvals only when present', async () => {
    const wrapper = mount(AppRail, { props })
    expect(wrapper.find('[data-rail-action="attention"]').exists()).toBe(false)
    await wrapper.setProps({ attentionCount: 2 })
    await wrapper.get('[aria-label="Approvals (2)"]').trigger('click')
    expect(wrapper.emitted('openAttention')).toHaveLength(1)
    wrapper.unmount()
  })
})
