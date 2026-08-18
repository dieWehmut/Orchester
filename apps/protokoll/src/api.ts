/**
 * The REST contract between the frontends and `orchester web`.
 *
 * This file is written before the server that serves it, deliberately: the Rust
 * handlers are implemented against these shapes, so the contract lives in one
 * place instead of being inferred from whatever a handler happened to return.
 * Field names stay `snake_case` because they cross serde.
 */

import type { ApprovalId, Usage } from './event'

/** An adapter the registry discovered, and whether its binary is on PATH. */
export interface AgentSummary {
  id: string
  name: string
  description: string
  capabilities: string[]
  /** False when the adapter is known but its executable is not installed. */
  available: boolean
}

/** The model that turns will actually run on. */
export type ActiveModelDto =
  | {
      state: 'configured'
      model: string
      provider: string
      reasoning_effort: string | null
    }
  | {
      /** Configuration named a model but something it depends on is missing. */
      state: 'unresolved'
      /** The configuration field at fault, so the UI can point at it. */
      field: string
      reason: string
    }
  | { state: 'not_configured' }

export interface ProviderChoiceDto {
  name: string
  wire_api: string
  base_url: string
  model: string
  /**
   * Whether a key is present in the OS keyring. Never the key itself, and never
   * a prefix of it: a "credential hint" is a credential.
   */
  credential_present: boolean
  active: boolean
}

export interface ModelProfileDto {
  name: string
  model: string
  provider: string
  reasoning_effort: string | null
}

export interface ModelCatalogDto {
  active: ActiveModelDto
  providers: ProviderChoiceDto[]
  profiles: ModelProfileDto[]
}

export interface SessionSummaryDto {
  /** The opaque handle `orchester --resume` accepts. */
  handle: string
  started_at: string
  title: string
  stage: string
  resumable: boolean
}

export interface ConfigViewDto {
  config_path: string
  workspace: string
  resolution: 'loaded' | 'missing' | 'invalid'
  /** Why the config could not be used, when `resolution` is not `loaded`. */
  message: string | null
}

export interface StatusDto {
  workspace: string
  model: string
  actor: string
  approvals_pending: number
  audit_ok: boolean
}

export interface StartRunRequest {
  prompt: string
  /** A handle from {@link SessionSummaryDto} to continue instead of starting fresh. */
  resume?: string
}

export interface StartRunResponse {
  run_id: string
  /**
   * Where to open the WebSocket for this run. The server decides, so the client
   * never has to reconstruct a URL from a port it guessed.
   */
  events_url: string
}

export interface ApprovalDecision {
  approval_id: ApprovalId
  approve: boolean
}

/** What a finished run cost, for a footer that outlives the stream. */
export interface RunSummaryDto {
  run_id: string
  usage: Usage
  stopped: boolean
}

/** The shape every failing endpoint returns. */
export interface ApiErrorDto {
  error: string
  /** A stable machine-readable code; the prose in `error` may be reworded. */
  code: string
}
