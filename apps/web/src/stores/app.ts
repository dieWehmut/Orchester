import { inject, type App, type InjectionKey } from 'vue'
import type { Pinia } from 'pinia'

import { createHttpClient, type HttpClient } from '../api/http'
import { createRunsApi, type RunsApi } from '../api/runs'
import { createSessionsApi } from '../api/sessions'
import { createModelsApi } from '../api/models'
import {
  createBootstrapStore,
  type BootstrapStore,
  type BootstrapStoreOptions,
} from './bootstrap'
import { createSessionsStore, type SessionsStore } from './sessions'
import { createRunStore, type RunStore } from './run'
import { createAppPinia } from './pinia'
import { useAgentFleetStore } from './agent-fleet'
import type { AgentStatusStreamFactory } from './agent-fleet'
import { createAgentsApi } from '../api/agents'
import { createAgentStatusSocket } from '../transport/agent-status-socket'
import { useModelCatalogStore } from './model-catalog'

export interface AppStores {
  http: HttpClient
  runs: RunsApi
  bootstrap: BootstrapStore
  sessions: SessionsStore
  run: RunStore
  agents: ReturnType<typeof useAgentFleetStore>
  models: ReturnType<typeof useModelCatalogStore>
  pinia: Pinia
  getCsrfToken: () => string | null
  start: () => Promise<void>
  stop: () => void
  install: (app: App) => void
}

export interface AppStoresOptions {
  http?: HttpClient
  location?: BootstrapStoreOptions['location']
  history?: BootstrapStoreOptions['history']
  agentStatusStreamFactory?: AgentStatusStreamFactory | null
}

const APP_STORES_KEY: InjectionKey<AppStores> = Symbol('orchester-app-stores')

export function createAppStores(options: AppStoresOptions = {}): AppStores {
  let csrfToken: string | null = null
  const http =
    options.http ??
    createHttpClient({
      csrfToken: () => csrfToken,
    })
  const bootstrapOptions: BootstrapStoreOptions = {
    http,
    installCsrfToken: (token) => {
      csrfToken = token
    },
  }
  if (options.location) bootstrapOptions.location = options.location
  if (options.history) bootstrapOptions.history = options.history

  const bootstrap = createBootstrapStore(bootstrapOptions)
  const sessions = createSessionsStore(createSessionsApi(http))
  const runs = createRunsApi(http)
  const run = createRunStore(runs)
  const pinia = createAppPinia()
  const agents = useAgentFleetStore(pinia)
  const models = useModelCatalogStore(pinia)
  const agentStatusStreamFactory =
    options.agentStatusStreamFactory === undefined
      ? createAgentStatusSocket
      : options.agentStatusStreamFactory
  agents.configure(createAgentsApi(http), agentStatusStreamFactory ?? undefined)
  models.configure(createModelsApi(http))

  const stores: AppStores = {
    http,
    runs,
    bootstrap,
    sessions,
    run,
    agents,
    models,
    pinia,
    getCsrfToken: () => csrfToken,
    async start(): Promise<void> {
      await bootstrap.load()
      if (bootstrap.status.value === 'ready' && bootstrap.context.value?.workspace.selected) {
        await Promise.all([sessions.load(), agents.start(), models.load()])
      }
    },
    stop(): void {
      agents.stop()
    },
    install(app: App): void {
      app.use(pinia)
      app.provide(APP_STORES_KEY, stores)
    },
  }

  return stores
}

export function useAppStores(): AppStores {
  const stores = inject(APP_STORES_KEY)
  if (!stores) throw new Error('Orchester app stores are not installed')
  return stores
}
