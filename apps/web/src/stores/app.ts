import { inject, type App, type InjectionKey } from 'vue'

import { createHttpClient, type HttpClient } from '../api/http'
import { createRunsApi, type RunsApi } from '../api/runs'
import { createSessionsApi } from '../api/sessions'
import {
  createBootstrapStore,
  type BootstrapStore,
  type BootstrapStoreOptions,
} from './bootstrap'
import { createSessionsStore, type SessionsStore } from './sessions'
import { createRunStore, type RunStore } from './run'

export interface AppStores {
  http: HttpClient
  runs: RunsApi
  bootstrap: BootstrapStore
  sessions: SessionsStore
  run: RunStore
  getCsrfToken: () => string | null
  start: () => Promise<void>
  install: (app: App) => void
}

export interface AppStoresOptions {
  http?: HttpClient
  location?: BootstrapStoreOptions['location']
  history?: BootstrapStoreOptions['history']
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

  const stores: AppStores = {
    http,
    runs,
    bootstrap,
    sessions,
    run,
    getCsrfToken: () => csrfToken,
    async start(): Promise<void> {
      await bootstrap.load()
      if (bootstrap.status.value === 'ready' && bootstrap.context.value?.workspace.selected) {
        await sessions.load()
      }
    },
    install(app: App): void {
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
