import type {
  ModelCatalogDto,
  ModelChoiceDto,
  ProviderChoiceDto,
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

  function reset(): void {
    generation += 1
    status.value = 'idle'
    catalog.value = null
    error.value = null
  }

  return {
    status,
    catalog,
    error,
    activeChoice,
    activeProvider,
    configure,
    load,
    reset,
  }
})
