import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { afterEach, describe, expect, it } from 'vitest'

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
  })
}

async function openPicker(wrapper: ReturnType<typeof mountPicker>) {
  await wrapper.get('[data-model-picker] [aria-haspopup="menu"]').trigger('click')
  await nextTick()
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
})

describe('ModelContextControl readout', () => {
  it('renders the active model, provider, and effort from the runtime catalog', () => {
    const wrapper = mountPicker()

    expect(wrapper.find('[data-model-context]').exists()).toBe(true)
    expect(wrapper.get('[data-model-context-model]').text()).toContain('gpt-5.6')
    expect(wrapper.get('[data-model-context-provider]').text()).toContain('OpenAI')
    expect(wrapper.get('[data-model-context-effort]').text()).toContain('high')
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

  it('shows an unavailable state when there is nothing to choose', () => {
    const wrapper = mountPicker({
      ...MODEL_CATALOG_FIXTURE,
      active: { state: 'not_configured' },
      selected_provider: null,
      providers: [],
      profiles: [],
    })

    expect(wrapper.get('[data-model-context-unavailable]').text()).toContain('Model unavailable')
    expect(wrapper.get('[data-model-context-unavailable]').attributes('aria-disabled')).toBe('true')
    wrapper.unmount()
  })
})

describe('ModelContextControl picker', () => {
  it('lists the providers, the profiles and the efforts, and marks what is in force', async () => {
    const wrapper = mountPicker()
    await openPicker(wrapper)

    const rows = items(wrapper).map((item) => item.text())

    expect(rows[0]).toContain('OpenAI')
    expect(rows[0]).toContain('gpt-5.6')
    expect(rows.some((row) => row.includes('review'))).toBe(true)
    expect(rows.some((row) => row.includes('Effort: high'))).toBe(true)

    // A menu that chooses has to say which one is in force.
    expect(wrapper.get('[data-model-picker] [role="menuitemradio"][aria-checked="true"]').text()).toContain(
      'OpenAI',
    )
    wrapper.unmount()
  })

  it('draws a provider the workspace cannot reach, with the reason it cannot', async () => {
    const wrapper = mountPicker()
    await openPicker(wrapper)

    const relay = itemByText(wrapper, 'Relay')

    expect(relay.attributes('disabled')).toBeDefined()
    expect(relay.text()).toContain('Provider endpoint is not configured')
    wrapper.unmount()
  })

  it('reports the whole intent when a provider is chosen, keeping the effort', async () => {
    const wrapper = mountPicker()
    await openPicker(wrapper)

    await itemByText(wrapper, 'OpenAI').trigger('click')

    // Picking a provider changes one axis: the effort in force comes with it,
    // because a request without it would reset the override to the default.
    expect(wrapper.emitted('select')).toEqual([
      [{ provider: 'openai', effort: 'high' }],
    ])
    wrapper.unmount()
  })

  it('names a profile without letting go of the axis', async () => {
    const wrapper = mountPicker()
    await openPicker(wrapper)

    await itemByText(wrapper, 'review').trigger('click')

    expect(wrapper.emitted('select')).toEqual([[{ profile: 'review', effort: 'high' }]])
    wrapper.unmount()
  })

  it('sends an effort for the axis the runtime reports', async () => {
    const plain = mountPicker()
    await openPicker(plain)

    await itemByText(plain, 'Effort: low').trigger('click')

    // This catalog reports no chosen provider: the active model is the
    // configuration's, so the request carries the effort and nothing else.
    expect(plain.emitted('select')).toEqual([[{ effort: 'low' }]])
    plain.unmount()

    const chosen = mountPicker({ ...MODEL_CATALOG_FIXTURE, selected_provider: 'openai' })
    await openPicker(chosen)

    await itemByText(chosen, 'Effort: low').trigger('click')

    // With a provider in force, the same click has to say so: a request that
    // only named the effort would drop the provider the reader chose.
    expect(chosen.emitted('select')).toEqual([[{ provider: 'openai', effort: 'low' }]])
    chosen.unmount()
  })

  it('clears the override when the provider default is chosen', async () => {
    const wrapper = mountPicker({ ...MODEL_CATALOG_FIXTURE, selected_provider: 'openai' })
    await openPicker(wrapper)

    await itemByText(wrapper, 'Effort: provider default').trigger('click')

    expect(wrapper.emitted('select')).toEqual([[{ provider: 'openai', effort: null }]])
    wrapper.unmount()
  })

  it('says how far the choice reaches, where the choice is made', async () => {
    const wrapper = mountPicker()
    await openPicker(wrapper)

    const scope = items(wrapper).at(-1)!

    // The runtime keeps this for as long as it runs and never writes it to the
    // configuration file, so the menu says so rather than reading as a setting
    // the reader has changed for good.
    expect(scope.text()).toContain('Applies to the runs that follow')
    expect(scope.attributes('disabled')).toBeDefined()
    wrapper.unmount()
  })

  it('keeps reporting a value it has no name for rather than hiding it', () => {
    const wrapper = mountPicker({
      ...MODEL_CATALOG_FIXTURE,
      active: {
        state: 'configured',
        choice: {
          profile: null,
          provider: 'openai',
          provider_name: 'OpenAI',
          model: 'gpt-5.6',
          reasoning_effort: 'xhigh',
          plan_reasoning_effort: null,
          service_tier: null,
        },
      },
    })

    // The runtime passes any effort through, so a value this surface cannot name
    // stays visible: forcing it into one of the offered names would report a
    // model setting the run is not using.
    expect(wrapper.get('[data-model-context-effort]').text()).toBe('xhigh')
    wrapper.unmount()
  })
})