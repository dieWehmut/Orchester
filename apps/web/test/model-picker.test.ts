import { flushPromises, mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { afterEach, describe, expect, it, vi } from 'vitest'

import ModelContextControl from '../src/components/run/ModelContextControl.vue'
import type { ModelCatalogStoreStatus } from '../src/stores/model-catalog'
import { MODEL_CATALOG_FIXTURE } from './fixtures/model-catalog'

/**
 * The model picker, which is the readout made real.
 *
 * It was a readout for as long as the runtime could not be asked to choose; now
 * that a selection route exists, the same control offers the providers the
 * workspace can reach, the named profiles, and the reasoning effort. Each click
 * reports the whole intent, because the runtime takes three axes and a request
 * that omitted one would silently reset it.
 */

function mountPicker(
  catalog = MODEL_CATALOG_FIXTURE,
  status: ModelCatalogStoreStatus = 'ready',
) {
  return mount(ModelContextControl, {
    props: { catalog, status },
    attachTo: document.body,
    global: { stubs: { teleport: true } },
  })
}

async function openPicker(wrapper: ReturnType<typeof mountPicker>) {
  await wrapper.get('[data-model-picker] [aria-haspopup="menu"]').trigger('click')
  await nextTick()
  await flushPromises()
}

function items(wrapper: ReturnType<typeof mountPicker>) {
  return wrapper.findAll('[data-model-picker] [role="menuitemradio"], [data-model-picker] [role="menuitem"]')
}

function itemByText(wrapper: ReturnType<typeof mountPicker>, text: string) {
  const found = items(wrapper).find((item) => item.text().includes(text))
  if (!found) throw new Error(`no menu item containing ${text}`)
  return found
}

afterEach(() => {
  document.body.replaceChildren()
  vi.restoreAllMocks()
})

describe('ModelContextControl readout', () => {
  it('renders the active model, provider, and effort from the runtime catalog', () => {
    const wrapper = mountPicker()

    expect(wrapper.find('[data-model-context]').exists()).toBe(true)
    expect(wrapper.get('[data-model-context-model]').text()).toContain('gpt-5.6')
    expect(wrapper.get('.model-context__readout').attributes('title')).toContain('OpenAI')
    expect(wrapper.get('[data-model-context-effort]').text()).toContain('High')
    expect(wrapper.find('[data-model-context-status]').exists()).toBe(false)
    wrapper.unmount()
  })

  it('keeps the last active context visible while the catalog is stale', () => {
    const wrapper = mountPicker(MODEL_CATALOG_FIXTURE, 'stale')

    expect(wrapper.get('[data-model-context-model]').text()).toContain('gpt-5.6')
    expect(wrapper.get('[data-model-context-status]').text()).toContain('stale')
    wrapper.unmount()
  })

  it('offers the choice when nothing is active but providers exist', () => {
    const wrapper = mountPicker({
      ...MODEL_CATALOG_FIXTURE,
      active: { state: 'not_configured' },
      selected_provider: null,
    })

    // A workspace whose configured model cannot be resolved is exactly the one
    // where choosing another provider is worth offering: a disabled readout
    // there would be a dead end.
    expect(wrapper.find('[data-model-picker]').exists()).toBe(true)
    expect(wrapper.find('[data-model-context-unavailable]').exists()).toBe(false)
    wrapper.unmount()
  })

  it('opens setup when no model service is configured', async () => {
    const wrapper = mountPicker({
      ...MODEL_CATALOG_FIXTURE,
      active: { state: 'not_configured' },
      selected_provider: null,
      providers: [],
      profiles: [],
    })

    const button = wrapper.get('[data-model-context-unavailable]')
    expect(button.text()).toContain('Set up model')
    await button.trigger('click')
    expect(wrapper.emitted('configure')).toEqual([[]])
    expect(wrapper.emitted('retry')).toBeUndefined()
    wrapper.unmount()
  })

  it('shows loading before offering setup, and retries failed catalog loads', async () => {
    const wrapper = mount(ModelContextControl, { props: { catalog: null, status: 'loading' } })
    expect(wrapper.text()).toContain('Loading models')
    expect(wrapper.find('[data-model-context-unavailable]').exists()).toBe(false)
    await wrapper.setProps({ status: 'error' })
    await wrapper.get('[data-model-context-unavailable]').trigger('click')
    expect(wrapper.emitted('retry')).toEqual([[]])
    expect(wrapper.emitted('configure')).toBeUndefined()
    wrapper.unmount()
  })
})

async function openSection(wrapper: ReturnType<typeof mountPicker>, section = 'models') {
  await openPicker(wrapper)
  await wrapper.get(`[data-model-section="${section}"]`).trigger('click')
  await nextTick()
  await flushPromises()
}

function choice(wrapper: ReturnType<typeof mountPicker>, id: string) {
  return wrapper.get(`[data-model-choice="${id}"]`)
}

describe('ModelContextControl picker', () => {
  it('opens only Model and Reasoning level before showing grouped choices', async () => {
    const wrapper = mountPicker()
    await openPicker(wrapper)
    expect(items(wrapper).map(item => item.text())).toEqual(['Modelgpt-5.6', 'Reasoning levelHigh'])
    expect(wrapper.find('[data-model-submenu]').exists()).toBe(false)
    await wrapper.get('[data-model-section="models"]').trigger('click')
    expect(wrapper.findAll('.model-selection__heading').map(item => item.text())).toEqual(['OpenAI', 'Relay'])
    expect(choice(wrapper, 'profile:review').text()).toContain('gpt-5.6review')
    expect(choice(wrapper, 'provider:openai').attributes('aria-checked')).toBe('true')
    expect(wrapper.findAll('[role="menuitemradio"][aria-checked="true"]')).toHaveLength(1)
    wrapper.unmount()
  })

  it('offers settings and retry inside the model submenu', async () => {
    const wrapper = mountPicker(MODEL_CATALOG_FIXTURE, 'stale')
    await openSection(wrapper)
    await itemByText(wrapper, 'Retry models').trigger('click')
    expect(wrapper.emitted('retry')).toEqual([[]])
    await openSection(wrapper)
    await itemByText(wrapper, 'Model settings').trigger('click')
    expect(wrapper.emitted('configure')).toEqual([[]])
    expect(wrapper.get('[data-model-context-model]').text()).toContain('gpt-5.6')
    wrapper.unmount()
  })

  it('shows an unavailable provider with a reason and prevents selecting it', async () => {
    const wrapper = mountPicker()
    await openSection(wrapper)
    const relay = choice(wrapper, 'provider:relay')
    expect(relay.attributes('disabled')).toBeDefined()
    expect(relay.text()).toContain('Provider endpoint is not configured')
    await relay.trigger('click')
    expect(wrapper.emitted('select')).toBeUndefined()
    wrapper.unmount()
  })

  it.each([
    ['provider:openai', { provider: 'openai', effort: 'high' }],
    ['profile:review', { profile: 'review', effort: 'high' }],
  ])('selects %s while preserving the effort and returns focus', async (id, selection) => {
    const wrapper = mountPicker()
    await openSection(wrapper)
    await choice(wrapper, id).trigger('click')
    expect(wrapper.emitted('select')).toEqual([[selection]])
    expect(wrapper.find('[data-model-root]').exists()).toBe(false)
    expect(document.activeElement).toBe(wrapper.get('[data-model-picker] > button').element)
    wrapper.unmount()
  })

  it('changes effort independently for configured, provider, and profile selections', async () => {
    for (const [catalog, axis] of [
      [MODEL_CATALOG_FIXTURE, {}],
      [{ ...MODEL_CATALOG_FIXTURE, selected_provider: 'openai' }, { provider: 'openai' }],
      [{ ...MODEL_CATALOG_FIXTURE, active: { state: 'configured', choice: MODEL_CATALOG_FIXTURE.profiles[0]! } }, { profile: 'review' }],
    ] as const) {
      const wrapper = mountPicker(catalog)
      await openSection(wrapper, 'efforts')
      await choice(wrapper, 'effort:max').trigger('click')
      expect(wrapper.emitted('select')).toEqual([[{ ...axis, effort: 'max' }]])
      await openSection(wrapper, 'efforts')
      await choice(wrapper, 'effort:').trigger('click')
      expect(wrapper.emitted('select')?.[1]).toEqual([{ ...axis, effort: null }])
      wrapper.unmount()
    }
  })

  it('checks the profile without also checking its provider', async () => {
    const wrapper = mountPicker({ ...MODEL_CATALOG_FIXTURE, active: { state: 'configured', choice: MODEL_CATALOG_FIXTURE.profiles[0]! } })
    await openSection(wrapper)
    const checked = wrapper.findAll('[aria-checked="true"]')
    expect(checked).toHaveLength(1)
    expect(checked[0]?.attributes('data-model-choice')).toBe('profile:review')
    wrapper.unmount()
  })

  it('preserves an unknown effort as a checked, selectable option', async () => {
    const wrapper = mountPicker({ ...MODEL_CATALOG_FIXTURE, active: { state: 'configured', choice: { ...MODEL_CATALOG_FIXTURE.profiles[0]!, reasoning_effort: 'custom-budget' } } })
    expect(wrapper.get('[data-model-context-effort]').text()).toBe('custom-budget')
    await openSection(wrapper, 'efforts')
    expect(choice(wrapper, 'effort:custom-budget').attributes('aria-checked')).toBe('true')
    await choice(wrapper, 'effort:custom-budget').trigger('click')
    expect(wrapper.emitted('select')).toEqual([[{ profile: 'review', effort: 'custom-budget' }]])
    wrapper.unmount()
  })

  it('keeps keyboard navigation, submenu Back, Escape, and outside focus independent', async () => {
    const wrapper = mountPicker()
    await openPicker(wrapper)
    const modelRow = wrapper.get('[data-model-section="models"]')
    expect(document.activeElement).toBe(modelRow.element)
    await modelRow.trigger('keydown', { key: 'ArrowRight' })
    expect(document.activeElement).toBe(choice(wrapper, 'provider:openai').element)
    await choice(wrapper, 'provider:openai').trigger('keydown', { key: 'End' })
    expect(document.activeElement?.textContent).toContain('Model settings')
    await wrapper.get('[data-model-submenu]').trigger('keydown', { key: 'Escape' })
    expect(wrapper.find('[data-model-submenu]').exists()).toBe(false)
    expect(document.activeElement).toBe(wrapper.get('[data-model-section="models"]').element)
    await wrapper.get('[data-model-section="models"]').trigger('keydown', { key: 'ArrowDown' })
    expect(document.activeElement).toBe(wrapper.get('[data-model-section="efforts"]').element)
    await wrapper.get('[data-model-root]').trigger('keydown', { key: 'Escape' })
    expect(document.activeElement).toBe(wrapper.get('[data-model-picker] > button').element)
    await openPicker(wrapper)
    const outside = document.createElement('button')
    document.body.append(outside)
    outside.focus()
    outside.dispatchEvent(new Event('pointerdown', { bubbles: true }))
    await nextTick()
    expect(wrapper.find('[data-model-root]').exists()).toBe(false)
    expect(document.activeElement).toBe(outside)
    wrapper.unmount()
  })

  it('replaces the root on narrow screens and keeps both panels within the viewport', async () => {
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({ left: 150, right: 355, top: 700, bottom: 734, width: 205, height: 34, x: 150, y: 700, toJSON() {} })
    vi.stubGlobal('innerWidth', 375)
    const wrapper = mountPicker()
    await openSection(wrapper)
    expect(wrapper.get('[data-model-submenu]').attributes('data-compact')).toBe('true')
    expect(wrapper.get('[data-model-root]').isVisible()).toBe(false)
    const menu = wrapper.get('[data-model-submenu]').element as HTMLElement
    expect(parseFloat(menu.style.left)).toBeGreaterThanOrEqual(8)
    expect(parseFloat(menu.style.left) + parseFloat(menu.style.width)).toBeLessThanOrEqual(367)
    await wrapper.get('[data-model-back]').trigger('click')
    expect(wrapper.get('[data-model-root]').isVisible()).toBe(true)
    expect(document.activeElement).toBe(wrapper.get('[data-model-section="models"]').element)
    wrapper.unmount()
    vi.unstubAllGlobals()
  })
})
