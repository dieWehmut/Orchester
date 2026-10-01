import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { ModelCatalogDto } from '@orchester/protokoll'

import { ApiError } from '../src/api/errors'
import type { HttpClient } from '../src/api/http'
import ProviderSettings from '../src/components/settings/ProviderSettings.vue'
import { createAppStores } from '../src/stores/app'
import { MODEL_CATALOG_FIXTURE } from './fixtures/model-catalog'

const empty: ModelCatalogDto = { schema_version: 1, active: { state: 'not_configured' }, selected_provider: null, providers: [], profiles: [] }
const wrappers: ReturnType<typeof mount>[] = []

function setup(post = vi.fn(async () => MODEL_CATALOG_FIXTURE)) {
  const get = vi.fn(async () => empty)
  const stores = createAppStores({ http: { get, post } as unknown as HttpClient, agentStatusStreamFactory: null })
  const wrapper = mount(ProviderSettings, { global: { plugins: [stores] } })
  wrappers.push(wrapper)
  return { wrapper, stores, get, post }
}

async function fillForm(wrapper: ReturnType<typeof setup>['wrapper']) {
  await wrapper.findAll('button')[0]!.trigger('click')
  await wrapper.get('#provider-name').setValue('Test relay')
  await wrapper.get('#provider-endpoint').setValue('https://example.com/v1')
  await wrapper.get('#provider-model').setValue('test-model')
  await wrapper.get('#provider-key').setValue('fake-test-key')
}

afterEach(() => { wrappers.splice(0).forEach((wrapper) => wrapper.unmount()) })

describe('model service settings', () => {
  it('loads an empty catalog and offers adding a service', async () => {
    const { wrapper, get } = setup()
    await flushPromises()
    expect(get).toHaveBeenCalledWith('/models')
    expect(wrapper.get('[data-provider-empty]').text()).toContain('No model service')
    expect(wrapper.find('[data-provider-form]').exists()).toBe(false)
  })

  it('clears the password as soon as a save begins and adopts only the confirmed catalog', async () => {
    let resolve!: (catalog: ModelCatalogDto) => void
    const post = vi.fn(() => new Promise<ModelCatalogDto>((done) => { resolve = done }))
    const { wrapper, stores } = setup(post)
    await flushPromises()
    await fillForm(wrapper)
    await wrapper.get('[data-provider-form]').trigger('submit')
    expect(post).toHaveBeenCalledWith('/models/providers', expect.objectContaining({ name: 'Test relay', api_key: 'fake-test-key', wire_api: 'responses' }))
    expect((wrapper.get('#provider-key').element as HTMLInputElement).value).toBe('')
    expect(wrapper.get('button[type="submit"]').attributes('disabled')).toBeDefined()
    expect(stores.models.catalog).toEqual(empty)
    resolve(MODEL_CATALOG_FIXTURE)
    await flushPromises()
    expect(wrapper.find('[data-provider-form]').exists()).toBe(false)
    expect(wrapper.text()).toContain('Model settings saved')
    expect(wrapper.text()).toContain('OpenAI')
    expect(stores.models.catalog).toEqual(MODEL_CATALOG_FIXTURE)
    expect(JSON.stringify(stores.models.$state)).not.toContain('fake-test-key')
  })

  it('keeps editable metadata and clears the key after a failed save', async () => {
    const post = vi.fn(async () => { throw new ApiError('private-error-fake-test-key', { code: 'validation_failed', retryable: false }) })
    const { wrapper, stores } = setup(post)
    await flushPromises()
    await fillForm(wrapper)
    await wrapper.get('[data-provider-form]').trigger('submit')
    await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toContain('Check the service URL')
    expect(wrapper.text()).not.toContain('private-error-fake-test-key')
    expect((wrapper.get('#provider-name').element as HTMLInputElement).value).toBe('Test relay')
    expect((wrapper.get('#provider-key').element as HTMLInputElement).value).toBe('')
    expect(stores.models.catalog).toEqual(empty)
    expect(wrapper.findAll('input').every((input) => input.attributes('disabled') === undefined)).toBe(true)
  })

  it('discards a canceled key when reopening the form', async () => {
    const { wrapper, post } = setup()
    await flushPromises()
    await fillForm(wrapper)
    await wrapper.findAll('button').find((button) => button.text() === 'Cancel')!.trigger('click')
    await wrapper.findAll('button')[0]!.trigger('click')
    expect((wrapper.get('#provider-key').element as HTMLInputElement).value).toBe('')
    expect(wrapper.get('#provider-key').attributes('type')).toBe('password')
    expect(wrapper.get('#provider-key').attributes('autocomplete')).toBe('off')
    expect(post).not.toHaveBeenCalled()
  })

  it('retries a failed load using refresh', async () => {
    const { wrapper, get } = setup()
    await flushPromises()
    get.mockRejectedValueOnce(new TypeError('offline'))
    await wrapper.findAll('button').find((button) => button.text() === 'Refresh')!.trigger('click')
    await flushPromises()
    expect(wrapper.find('[role="alert"]').exists()).toBe(true)
    expect(wrapper.get('[role="alert"]').text()).toContain('Unable to load')
    expect(wrapper.find('[data-provider-empty]').exists()).toBe(false)
    await wrapper.findAll('button').find((button) => button.text() === 'Refresh')!.trigger('click')
    await flushPromises()
    expect(wrapper.find('[role="alert"]').exists()).toBe(false)
    expect(get).toHaveBeenCalledTimes(3)
  })
})
