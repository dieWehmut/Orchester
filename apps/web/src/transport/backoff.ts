export interface ReconnectBackoffOptions {
  /** Delay before the first retry. */
  initialDelayMs?: number
  /** Exponential multiplier applied for each retry. */
  factor?: number
  /** Upper bound for an individual delay. */
  maxDelayMs?: number
  /** Maximum number of retry delays before exhaustion. */
  maxAttempts?: number
  /** Symmetric random jitter as a fraction of the base delay. */
  jitterRatio?: number
  /** Injectable source for deterministic tests. */
  random?: () => number
}

export interface ReconnectBackoff {
  readonly attempt: number
  readonly exhausted: boolean
  next: () => number | null
  reset: () => void
}

export const DEFAULT_RECONNECT_BACKOFF: Required<
  Omit<ReconnectBackoffOptions, 'random'>
> = {
  initialDelayMs: 250,
  factor: 2,
  maxDelayMs: 10_000,
  maxAttempts: 6,
  jitterRatio: 0.2,
}

function positiveFinite(value: number, fallback: number): number {
  return Number.isFinite(value) && value > 0 ? value : fallback
}

function nonNegativeFinite(value: number, fallback: number): number {
  return Number.isFinite(value) && value >= 0 ? value : fallback
}

/**
 * Creates a bounded retry schedule. A successful socket calls `reset`; a
 * caller must explicitly decide what to do after `next()` returns `null`.
 */
export function createReconnectBackoff(
  options: ReconnectBackoffOptions = {},
): ReconnectBackoff {
  const initialDelayMs = positiveFinite(
    options.initialDelayMs ?? DEFAULT_RECONNECT_BACKOFF.initialDelayMs,
    DEFAULT_RECONNECT_BACKOFF.initialDelayMs,
  )
  const factor = positiveFinite(
    options.factor ?? DEFAULT_RECONNECT_BACKOFF.factor,
    DEFAULT_RECONNECT_BACKOFF.factor,
  )
  const maxDelayMs = Math.max(
    initialDelayMs,
    positiveFinite(options.maxDelayMs ?? DEFAULT_RECONNECT_BACKOFF.maxDelayMs, initialDelayMs),
  )
  const maxAttempts = Math.max(
    1,
    Math.floor(
      positiveFinite(options.maxAttempts ?? DEFAULT_RECONNECT_BACKOFF.maxAttempts, 1),
    ),
  )
  const jitterRatio = Math.min(
    1,
    nonNegativeFinite(options.jitterRatio ?? DEFAULT_RECONNECT_BACKOFF.jitterRatio, 0),
  )
  const random = options.random ?? Math.random
  let attempt = 0

  return {
    get attempt() {
      return attempt
    },
    get exhausted() {
      return attempt >= maxAttempts
    },
    next() {
      if (attempt >= maxAttempts) return null
      const base = Math.min(maxDelayMs, initialDelayMs * factor ** attempt)
      attempt += 1
      if (jitterRatio === 0) return Math.round(base)

      const jitter = (random() * 2 - 1) * jitterRatio
      return Math.max(0, Math.min(maxDelayMs, Math.round(base * (1 + jitter))))
    },
    reset() {
      attempt = 0
    },
  }
}
