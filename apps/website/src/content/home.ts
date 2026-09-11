export type HomeIcon = 'observe' | 'govern' | 'compose' | 'model' | 'runtime' | 'adapter'

export interface HomeContentItem {
  readonly id: string
  readonly icon: HomeIcon
  readonly title: string
  readonly summary: string
  readonly detail: string
}

export const homeCapabilities: readonly HomeContentItem[] = Object.freeze([
  {
    id: 'observable',
    icon: 'observe',
    title: 'Observable by default',
    summary: 'Every turn, tool call, and approval has a durable place in the run timeline.',
    detail: 'Reconnect from a sequence number without asking the model to reconstruct what happened.',
  },
  {
    id: 'governed',
    icon: 'govern',
    title: 'Governed at the edge',
    summary: 'Risky actions stop at an explicit approval boundary before they reach a workspace.',
    detail: 'The browser sees redacted intent and decisions, never provider credentials or raw payloads.',
  },
  {
    id: 'composable',
    icon: 'compose',
    title: 'Composable in layers',
    summary: 'The runtime, loopback service, browser UI, and desktop shell share typed contracts.',
    detail: 'Choose the surface that fits the job while the Rust runtime remains the source of truth.',
  },
])

export const homeAdapters: readonly HomeContentItem[] = Object.freeze([
  {
    id: 'models',
    icon: 'model',
    title: 'Model adapters',
    summary: 'Keep provider configuration behind a local catalog and a bounded host.',
    detail: 'Switch models without moving secrets into browser state.',
  },
  {
    id: 'runtime',
    icon: 'runtime',
    title: 'Runtime primitives',
    summary: 'Sessions, files, approvals, and runs are exposed as safe versioned DTOs.',
    detail: 'A stable envelope makes reconnect and replay explicit rather than implicit.',
  },
  {
    id: 'surfaces',
    icon: 'adapter',
    title: 'Multiple surfaces',
    summary: 'Use the local WebUI, static demo, or Tauri shell without changing the core.',
    detail: 'The same interaction language scales from a browser tab to a desktop window.',
  },
])

export const governancePrinciples: readonly HomeContentItem[] = Object.freeze([
  {
    id: 'local-boundary',
    icon: 'govern',
    title: 'Loopback boundary',
    summary: 'The local service binds to a loopback address and rejects public interfaces.',
    detail: 'A browser session is a client of the runtime, not a second source of authority.',
  },
  {
    id: 'redaction',
    icon: 'observe',
    title: 'Redaction before transport',
    summary: 'Paths, credentials, provider payloads, and unredacted arguments stay in Rust.',
    detail: 'UI events carry the minimum context required to render a useful state.',
  },
  {
    id: 'idempotency',
    icon: 'compose',
    title: 'Idempotent decisions',
    summary: 'Approval decisions bind to a row version and a client request id.',
    detail: 'Retries remain safe when a tab reconnects or a desktop window resumes.',
  },
])
