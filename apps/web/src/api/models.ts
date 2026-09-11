import {
  parseModelCatalog,
  type ModelCatalogDto,
} from '@orchester/protokoll'

import { ApiError } from './errors'
import type { HttpClient } from './http'

export interface ModelCatalogOptions {
  signal?: AbortSignal
}

export interface ModelsApi {
  catalog: (options?: ModelCatalogOptions) => Promise<ModelCatalogDto>
}

export function createModelsApi(http: HttpClient): ModelsApi {
  return {
    async catalog({ signal } = {}): Promise<ModelCatalogDto> {
      const raw = signal
        ? await http.get<unknown>('/models', { signal })
        : await http.get<unknown>('/models')
      const catalog = parseModelCatalog(raw)
      if (catalog === null) {
        throw new ApiError('Invalid model catalog response', {
          code: 'runtime_error',
          retryable: false,
        })
      }
      return catalog
    },
  }
}
