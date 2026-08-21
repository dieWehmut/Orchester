import type {
  BootstrapDto,
  FragmentTokenExchangeRequestDto,
  SessionBootstrapDto,
} from '@orchester/protokoll'
import { ref, shallowRef, type Ref } from 'vue'

import { normalizeApiError, type ApiError } from '../api/errors'
import type { HttpClient } from '../api/http'

export type BootstrapStatus = 'idle' | 'loading' | 'ready' | 'error'

interface BrowserLocation {
  hash: string
  pathname: string
  search: string
}

interface BrowserHistory {
  state: unknown
  replaceState: (state: unknown, unused: string, url?: string | URL | null) => void
}

export interface BootstrapStoreOptions {
  http: HttpClient
  location?: BrowserLocation
  history?: BrowserHistory
  installCsrfToken?: (token: string) => void
}

export interface BootstrapStore {
  status: Ref<BootstrapStatus>
  context: Ref<BootstrapDto | null>
  expiresAt: Ref<number | null>
  error: Ref<ApiError | null>
  load: () => Promise<void>
}

function browserLocation(): BrowserLocation {
  if (typeof window === 'undefined') return { hash: '', pathname: '/', search: '' }
  return window.location
}

function browserHistory(): BrowserHistory {
  if (typeof window === 'undefined') {
    return { state: null, replaceState: () => undefined }
  }
  return window.history
}

function readFragmentToken(hash: string): string | null {
  const raw = hash.startsWith('#') ? hash.slice(1) : hash
  if (!raw) return null
  const params = new URLSearchParams(raw)
  const named = params.get('fragment_token') ?? params.get('token')
  if (named) return named
  if (!raw.includes('=')) return decodeURIComponent(raw)
  return null
}

function clearFragment(location: BrowserLocation, history: BrowserHistory): void {
  history.replaceState(history.state, '', `${location.pathname}${location.search}`)
}

export function createBootstrapStore(options: BootstrapStoreOptions): BootstrapStore {
  const status = ref<BootstrapStatus>('idle')
  const context = shallowRef<BootstrapDto | null>(null)
  const expiresAt = ref<number | null>(null)
  const error = shallowRef<ApiError | null>(null)
  const location = options.location ?? browserLocation()
  const history = options.history ?? browserHistory()

  async function load(): Promise<void> {
    status.value = 'loading'
    error.value = null
    try {
      const bootstrap = await options.http.get<BootstrapDto>('/bootstrap')
      const fragmentToken = readFragmentToken(location.hash)
      let session: SessionBootstrapDto
      if (fragmentToken) {
        clearFragment(location, history)
        const request: FragmentTokenExchangeRequestDto = {
          schema_version: 1,
          fragment_token: fragmentToken,
        }
        session = await options.http.post<SessionBootstrapDto>('/auth/fragment', request)
      } else {
        session = await options.http.get<SessionBootstrapDto>('/session')
      }
      context.value = bootstrap
      expiresAt.value = session.expires_at
      options.installCsrfToken?.(session.csrf_token)
      status.value = 'ready'
    } catch (cause) {
      error.value = normalizeApiError(cause)
      status.value = 'error'
    }
  }

  return { status, context, expiresAt, error, load }
}
