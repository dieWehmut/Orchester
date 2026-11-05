import {
  parseModelCatalog,
  type ModelCatalogDto,
  type ModelSelectionRequestDto,
  type ModelProviderRequestDto,
  type ProviderDetailsDto,
  type ProviderUpdateDto,
} from '@orchester/protokoll'

import { ApiError } from './errors'
import type { HttpClient } from './http'

export interface ModelCatalogOptions {
  signal?: AbortSignal
}

export interface ModelsApi {
  discoverProvider: (id: string) => Promise<{ models: Array<{ model: string; display_name?: string }> }>
  providerDetails: (id: string) => Promise<ProviderDetailsDto>
  updateProvider: (provider: ProviderUpdateDto) => Promise<ModelCatalogDto>
  deleteProvider: (id: string) => Promise<ModelCatalogDto>
  saveProvider: (provider: ModelProviderRequestDto) => Promise<ModelCatalogDto>
  catalog: (options?: ModelCatalogOptions) => Promise<ModelCatalogDto>
  /**
   * Choose the model the following runs use.
   *
   * The runtime validates the choice against this workspace's configuration and
   * answers with the catalog it produces, so the caller has the new state
   * without a second request - and a choice that cannot be applied comes back as
   * an error rather than as a run that quietly used something else.
   */
  select: (
    selection: ModelSelectionRequestDto,
    options?: ModelCatalogOptions,
  ) => Promise<ModelCatalogDto>
}

export function createModelsApi(http: HttpClient): ModelsApi {
  function readCatalog(raw: unknown): ModelCatalogDto {
    const catalog = parseModelCatalog(raw)
    if (catalog === null) {
      throw new ApiError('Invalid model catalog response', {
        code: 'runtime_error',
        retryable: false,
      })
    }
    return catalog
  }

  return {
    discoverProvider: id => http.post(`/models/providers/${encodeURIComponent(id)}/discover`, {}),
    async providerDetails(id) {
      const raw = await http.get<unknown>(`/models/providers/${encodeURIComponent(id)}`)
      if (!raw || typeof raw !== 'object') throw new ApiError('Invalid provider details', { code: 'runtime_error', retryable: false })
      const detail = raw as ProviderDetailsDto
      if (detail.provider !== id || !['name', 'base_url', 'wire_api'].every(key => typeof (raw as Record<string, unknown>)[key] === 'string') || !Array.isArray(detail.models) || !detail.models.every(entry => entry && typeof entry.profile === 'string' && typeof entry.model === 'string')) {
        throw new ApiError('Invalid provider details', { code: 'runtime_error', retryable: false })
      }
      return detail
    },
    async updateProvider(provider) { return readCatalog(await http.put<unknown>(`/models/providers/${encodeURIComponent(provider.provider)}`, provider)) },
    async deleteProvider(id) { return readCatalog(await http.delete<unknown>(`/models/providers/${encodeURIComponent(id)}`)) },
    async saveProvider(provider): Promise<ModelCatalogDto> {
      return readCatalog(await http.post<unknown>('/models/providers', provider))
    },
    async catalog({ signal } = {}): Promise<ModelCatalogDto> {
      const raw = signal
        ? await http.get<unknown>('/models', { signal })
        : await http.get<unknown>('/models')
      return readCatalog(raw)
    },
    async select(selection, { signal } = {}): Promise<ModelCatalogDto> {
      const raw = signal
        ? await http.put<unknown>('/models/selection', selection, { signal })
        : await http.put<unknown>('/models/selection', selection)
      return readCatalog(raw)
    },
  }
}
