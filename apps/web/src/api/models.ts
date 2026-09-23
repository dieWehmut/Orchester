import {
  parseModelCatalog,
  type ModelCatalogDto,
  type ModelSelectionRequestDto,
} from '@orchester/protokoll'

import { ApiError } from './errors'
import type { HttpClient } from './http'

export interface ModelCatalogOptions {
  signal?: AbortSignal
}

export interface ModelsApi {
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