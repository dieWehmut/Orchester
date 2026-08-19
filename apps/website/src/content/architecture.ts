export interface ArchitectureStage {
  readonly id: 'browser' | 'service' | 'runtime' | 'provider'
  readonly index: string
  readonly label: string
  readonly title: string
  readonly description: string
  readonly contract: string
  readonly details: readonly string[]
}

export const architectureStages: readonly ArchitectureStage[] = Object.freeze([
  {
    id: 'browser',
    index: '01',
    label: 'Surface',
    title: 'Browser and desktop surfaces',
    description: 'The WebUI and Tauri shell render the same redacted state without owning runtime decisions.',
    contract: 'Vue views + typed API envelopes',
    details: ['Session history and run views', 'Keyboard-safe approvals', 'No provider credentials in state'],
  },
  {
    id: 'service',
    index: '02',
    label: 'Boundary',
    title: 'Loopback HTTP and WebSocket service',
    description: 'Netz exposes a small local API, validates sessions, and translates durable events into UI envelopes.',
    contract: 'Axum routes + UiEventEnvelope',
    details: ['Loopback-only bind validation', 'Cookie session and CSRF checks', 'Replay and resync semantics'],
  },
  {
    id: 'runtime',
    index: '03',
    label: 'Authority',
    title: 'Rust application runtime',
    description: 'Anwendung, Laufzeit, and RunStore keep workspace state, approvals, and durable history authoritative.',
    contract: 'Typed domain services + durable events',
    details: ['Workspace-bound paths', 'Bounded action execution', 'Explicit terminal outcomes'],
  },
  {
    id: 'provider',
    index: '04',
    label: 'Adapter',
    title: 'Model and tool adapters',
    description: 'Provider-specific behavior stays behind a host boundary so the rest of the system can remain stable.',
    contract: 'Registry + SelfAgentHost',
    details: ['Capability catalog', 'Safe model choices', 'Provider errors mapped to API codes'],
  },
])

export const architectureBoundary = Object.freeze({
  title: 'Boundary rules that stay visible',
  description: 'The browser can ask for a state transition, but only the local runtime can authorize and persist it.',
  bullets: [
    'Loopback service first: no public bind or implicit CORS opening.',
    'Redact before serialization: browser DTOs never mirror HarnessEvent internals.',
    'Replace snapshots atomically: a failed response is visible instead of silently merged away.',
  ] as const,
})
