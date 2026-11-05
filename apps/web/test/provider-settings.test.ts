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
  const remove = vi.fn(async () => empty)
  const stores = createAppStores({ http: { get, post, put: post, delete: remove } as unknown as HttpClient, agentStatusStreamFactory: null })
  const wrapper = mount(ProviderSettings, { global: { plugins: [stores] } })
  wrappers.push(wrapper)
  return { wrapper, stores, get, post, remove }
}

async function fillForm(wrapper: ReturnType<typeof setup>['wrapper']) {
  await wrapper.findAll('button')[0]!.trigger('click')
  await wrapper.get('#provider-name').setValue('Test relay')
  await wrapper.get('#provider-endpoint').setValue('https://example.com/v1')
  await wrapper.get('.model-editor input').setValue('test-model')
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
    expect(post).toHaveBeenCalledWith(expect.stringMatching(/^\/models\/providers\/provider-/), expect.objectContaining({ name: 'Test relay', api_key: 'fake-test-key', wire_api: 'responses' }))
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
  it('edits an existing model catalog without resending a saved credential', async () => {
    const { wrapper, stores, get, post } = setup()
    await flushPromises()
    stores.models.catalog = MODEL_CATALOG_FIXTURE
    await flushPromises()
    const provider = MODEL_CATALOG_FIXTURE.providers[0]!
    get.mockResolvedValueOnce({ provider: provider.id, name: provider.name, base_url: 'https://example.com/v1', wire_api: 'responses', models: [{ profile: 'one', model: 'original', input_types: ['text'] }] } as never)
    await wrapper.findAll('button').find(button => button.text() === 'Edit')!.trigger('click')
    await flushPromises()
    expect((wrapper.get('#provider-key').element as HTMLInputElement).value).toBe('')
    await wrapper.get('.model-editor input').setValue('edited-model')
    await wrapper.get('[data-provider-form]').trigger('submit')
    await flushPromises()
    const request = (post.mock.calls as unknown as Array<[string, Record<string, unknown>]>)[0]![1]
    expect(request).not.toHaveProperty('api_key')
    expect(request.models).toEqual([expect.objectContaining({ model: 'edited-model', profile: 'one' })])
  })

  it('requires confirmation before deleting an existing service', async () => {
    const { wrapper, stores, remove } = setup()
    await flushPromises()
    stores.models.catalog = MODEL_CATALOG_FIXTURE
    await flushPromises()
    await wrapper.findAll('button').find(button => button.text() === 'Delete')!.trigger('click')
    expect(remove).not.toHaveBeenCalled()
    await wrapper.get('[role="alertdialog"]').findAll('button').find(button => button.text() === 'Confirm delete')!.trigger('click')
    await flushPromises()
    expect(remove).toHaveBeenCalledWith(`/models/providers/${MODEL_CATALOG_FIXTURE.providers[0]!.id}`)
    expect(stores.models.catalog).toEqual(empty)
  })

  it('opens a newly added model and reports its missing ID by index', async () => {
    const { wrapper, post } = setup()
    await flushPromises()
    await fillForm(wrapper)
    await wrapper.findAll('button').find(button => button.text() === 'Add model')!.trigger('click')
    expect(wrapper.findAll('.model-editor')[1]!.attributes('open')).toBeDefined()
    await wrapper.get('[data-provider-form]').trigger('submit')
    await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toBe('Model 2: model ID is required.')
    expect(post).not.toHaveBeenCalled()
  })

  it('shows inherited reasoning as enabled and requires a key for a new service', async () => {
    const { wrapper, post } = setup()
    await flushPromises()
    await fillForm(wrapper)
    expect(wrapper.get('[role="switch"]').attributes('aria-checked')).toBe('true')
    expect(wrapper.text()).toContain('Using default reasoning levels')
    await wrapper.get('[role="switch"]').trigger('click')
    expect(wrapper.text()).not.toContain('Using default reasoning levels')
    await wrapper.get('#provider-key').setValue('')
    expect(wrapper.get('#provider-key').attributes('required')).toBeDefined()
    await wrapper.get('[data-provider-form]').trigger('submit')
    expect(wrapper.get('[role="alert"]').text()).toContain('API key is required')
    expect(post).not.toHaveBeenCalled()
  })

  it('preserves distinct aliases pointing to the same model ID', async () => {
    const { wrapper, stores, get, post } = setup()
    await flushPromises()
    stores.models.catalog = MODEL_CATALOG_FIXTURE
    await flushPromises()
    const provider = MODEL_CATALOG_FIXTURE.providers[0]!
    get.mockResolvedValueOnce({ provider: provider.id, name: provider.name, base_url: 'https://example.com/v1', wire_api: 'responses', models: [
      { profile: 'one', model: 'shared-id', display_name: 'Fast', input_types: ['text'] },
      { profile: 'two', model: 'shared-id', display_name: 'Careful', input_types: ['text'] },
    ] } as never)
    await wrapper.findAll('button').find(button => button.text() === 'Edit')!.trigger('click')
    await flushPromises()
    await wrapper.get('[data-provider-form]').trigger('submit')
    await flushPromises()
    expect(post).toHaveBeenCalledWith(expect.any(String), expect.objectContaining({ models: [
      expect.objectContaining({ profile: 'one', model: 'shared-id', display_name: 'Fast' }),
      expect.objectContaining({ profile: 'two', model: 'shared-id', display_name: 'Careful' }),
    ] }))
    expect(wrapper.find('[data-provider-form]').exists()).toBe(false)
  })

})
