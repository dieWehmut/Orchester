import type {
  ModelCatalogDto,
  ModelChoiceDto,
  ModelSelectionRequestDto,
  ProviderChoiceDto,
  ModelProviderRequestDto,
  ProviderUpdateDto,
} from '@orchester/protokoll'
import { computed, ref, shallowRef } from 'vue'
import { defineStore } from 'pinia'

import { normalizeApiError, type ApiError } from '../api/errors'
import type { ModelsApi } from '../api/models'

export type ModelCatalogStoreStatus =
  | 'idle'
  | 'loading'
  | 'refreshing'
  | 'ready'
  | 'stale'
  | 'error'

export const useModelCatalogStore = defineStore('modelCatalog', () => {
  const status = ref<ModelCatalogStoreStatus>('idle')
  const catalog = shallowRef<ModelCatalogDto | null>(null)
  const error = shallowRef<ApiError | null>(null)
  const activeChoice = computed<ModelChoiceDto | null>(() =>
    catalog.value?.active.state === 'configured'
      ? catalog.value.active.choice
      : null,
  )
  const activeProvider = computed<ProviderChoiceDto | null>(() =>
    catalog.value?.providers.find((provider) => provider.active) ?? null,
  )

  let api: ModelsApi | null = null
  let generation = 0

  function configure(nextApi: ModelsApi): void {
    api = nextApi
  }

  async function load(): Promise<void> {
    const currentApi = api
    const currentGeneration = ++generation
    if (!currentApi) {
      error.value = normalizeApiError(new TypeError('model catalog API unavailable'))
      status.value = catalog.value ? 'stale' : 'error'
      return
    }

    status.value = catalog.value ? 'refreshing' : 'loading'
    error.value = null
    try {
      const next = await currentApi.catalog()
      if (currentGeneration !== generation) return
      catalog.value = next
      status.value = 'ready'
    } catch (cause) {
      if (currentGeneration !== generation) return
      error.value = normalizeApiError(cause)
      status.value = catalog.value ? 'stale' : 'error'
    }
  }

  async function discoverProvider(id: string) {
    if (!api) throw new Error('Model API unavailable')
    return api.discoverProvider(id)
  }
  async function providerDetails(id: string) {
    if (!api) throw new Error("Model API unavailable")
    return api.providerDetails(id)
  }
  async function editCatalog(action: (current: ModelsApi) => Promise<ModelCatalogDto>): Promise<boolean> {
    const current = api
    const requestGeneration = ++generation
    if (!current) return false
    status.value = catalog.value ? 'refreshing' : 'loading'
    error.value = null
    try {
      const next = await action(current)
      if (generation !== requestGeneration) return false
      catalog.value = next; status.value = 'ready'; return true
    } catch (cause) {
      if (generation !== requestGeneration) return false
      error.value = normalizeApiError(cause); status.value = catalog.value ? 'stale' : 'error'; return false
    }
  }
  function updateProvider(provider: ProviderUpdateDto) { return editCatalog(current => current.updateProvider(provider)) }
  function deleteProvider(id: string) { return editCatalog(current => current.deleteProvider(id)) }

  function reset(): void {
    generation += 1
    status.value = 'idle'
    catalog.value = null
    error.value = null
  }

  /**
   * Choose the model the following runs use.
   *
   * The runtime answers with the catalog its choice produces, so what the picker
   * draws comes from the runtime rather than from what the reader clicked: a
   * selection this workspace cannot apply leaves both unchanged, and says why.
   */
  async function select(selection: ModelSelectionRequestDto): Promise<boolean> {
    const currentApi = api
    const currentGeneration = ++generation
    if (!currentApi) {
      error.value = normalizeApiError(new TypeError('model catalog API unavailable'))
      status.value = catalog.value ? 'stale' : 'error'
      return false
    }

    status.value = catalog.value ? 'refreshing' : 'loading'
    error.value = null
    try {
      const next = await currentApi.select(selection)
      if (currentGeneration !== generation) return false
      catalog.value = next
      status.value = 'ready'
      return true
    } catch (cause) {
      if (currentGeneration !== generation) return false
      error.value = normalizeApiError(cause)
      status.value = catalog.value ? 'stale' : 'error'
      return false
    }
  }

  async function saveProvider(provider: ModelProviderRequestDto): Promise<boolean> {
    const currentApi = api
    const currentGeneration = ++generation
    if (!currentApi) {
      error.value = normalizeApiError(new TypeError('model catalog API unavailable'))
      status.value = catalog.value ? 'stale' : 'error'
      return false
    }
    status.value = catalog.value ? 'refreshing' : 'loading'
    error.value = null
    try {
      const next = await currentApi.saveProvider(provider)
      if (currentGeneration !== generation) return false
      catalog.value = next
      status.value = 'ready'
      return true
    } catch (cause) {
      if (currentGeneration !== generation) return false
      error.value = normalizeApiError(cause)
      status.value = catalog.value ? 'stale' : 'error'
      return false
    }
  }

  return {
    status,
    catalog,
    error,
    activeChoice,
    activeProvider,
    configure,
    load,
    select,
    saveProvider,
    providerDetails,
    discoverProvider,
    updateProvider,
    deleteProvider,
    reset,
  }
})
