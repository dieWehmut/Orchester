/**
 * The REST contract between the frontends and `orchester web`.
 *
 * This file is written before the server that serves it, deliberately: the Rust
 * handlers are implemented against these shapes, so the contract lives in one
 * place instead of being inferred from whatever a handler happened to return.
 * Field names stay `snake_case` because they cross serde.
 */

import type { ApprovalId, Usage } from './event'
import type { RunId, UiApprovalRequest, UiEventEnvelope } from './ui'

export const HEALTH_SCHEMA_VERSION = 1 as const

export interface HealthDto {
  status: 'ok'
  service: 'orchester'
  version: string
  schema_version: typeof HEALTH_SCHEMA_VERSION
}

export const BOOTSTRAP_SCHEMA_VERSION = 1 as const

export type BootstrapServerState = 'starting' | 'running' | 'stopping' | 'stopped'

export interface BootstrapWorkspaceDto {
  selected: boolean
  /** A display-only basename; never an absolute or relative path. */
  name: string | null
}

export interface BootstrapDto {
  schema_version: typeof BOOTSTRAP_SCHEMA_VERSION
  service_version: string
  server_state: BootstrapServerState
  workspace: BootstrapWorkspaceDto
}

export const SESSION_SCHEMA_VERSION = 1 as const

export interface SessionBootstrapDto {
  schema_version: typeof SESSION_SCHEMA_VERSION
  /** Returned once; never persisted by the browser. */
  csrf_token: string
  /** Unix seconds when the browser must bootstrap a new session. */
  expires_at: number
}

export const FRAGMENT_AUTH_SCHEMA_VERSION = 1 as const

export interface FragmentTokenExchangeRequestDto {
  schema_version: typeof FRAGMENT_AUTH_SCHEMA_VERSION
  /** Read from `location.hash`, posted once, then removed from browser history. */
  fragment_token: string
}

export interface FragmentTokenExchangeResponseDto extends SessionBootstrapDto {
  schema_version: typeof FRAGMENT_AUTH_SCHEMA_VERSION
}

export const AGENT_CATALOG_SCHEMA_VERSION = 1 as const

export type AgentAvailabilityDto = 'available' | 'missing' | 'unknown'

/** An adapter projected from the registry without executable paths or commands. */
export interface AgentSummary {
  id: string
  name: string
  task_kinds: string[]
  supports_resume: boolean
  streaming: boolean
  availability: AgentAvailabilityDto
}

export interface AgentCatalogDto {
  schema_version: typeof AGENT_CATALOG_SCHEMA_VERSION
  agents: AgentSummary[]
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

export type ApprovalQueueState = 'pending' | 'approved' | 'denied' | 'expired' | 'stale'

export interface ApprovalQueueItemDto {
  approval_id: ApprovalId
  run_id: RunId
  row_version: number
  risk: string
  action: string
  reason: string
  state: ApprovalQueueState
  created_at: string
  expires_at: string | null
}

export interface ApprovalQueueDto {
  run_id: RunId
  items: ApprovalQueueItemDto[]
}

export type ApprovalDecisionKind = 'approved' | 'denied'

export interface ApprovalDecisionRequestDto {
  approval_id: ApprovalId
  row_version: number
  decision: ApprovalDecisionKind
  idempotency_key: string
}

export type ApprovalDecisionStatus = 'applied' | 'already_applied' | 'stale' | 'expired'

export interface ApprovalDecisionResponseDto {
  status: ApprovalDecisionStatus
  approval_id: ApprovalId
  row_version: number
  decision: ApprovalDecisionKind | 'stale' | 'expired'
}

/** What a finished run cost, for a footer that outlives the stream. */
export interface RunSummaryDto {
  run_id: string
  usage: Usage
  stopped: boolean
}

/** Durable run states exposed by the snapshot endpoint. */
export type RunStateDto =
  | 'created'
  | 'running'
  | 'awaiting_approval'
  | 'validating'
  | 'succeeded'
  | 'failed'
  | 'cancelled'
  | 'paused'

/** A bounded replay window and the approvals needed to render it safely. */
export interface RunSnapshotDto {
  run_id: RunId
  state: RunStateDto
  events: UiEventEnvelope[]
  pending_approvals: UiApprovalRequest[]
  oldest_sequence: number
  latest_sequence: number
  next_sequence: number
  updated_at: string
}

/** Request a replay after a sequence already held by the browser. */
export interface RunReplayRequestDto {
  after_sequence: number
  limit?: number
}

export interface RunReplayResponseDto {
  run_id: RunId
  events: UiEventEnvelope[]
  first_sequence: number | null
  last_sequence: number | null
  has_more: boolean
}

export type ResyncReason = 'retention_exceeded' | 'sequence_gap' | 'schema_mismatch'

/** Explicitly tells a client to fetch a fresh snapshot instead of guessing. */
export interface ResyncRequiredDto {
  type: 'resync_required'
  run_id: RunId
  requested_after_sequence: number
  oldest_sequence: number
  latest_sequence: number
  reason: ResyncReason
}

export type RunStreamFrameDto =
  | { type: 'event'; event: UiEventEnvelope }
  | ResyncRequiredDto

/** The shape every failing endpoint returns. */
export type ApiErrorCode =
  | 'bad_request'
  | 'method_not_allowed'
  | 'not_found'
  | 'unauthorized'
  | 'forbidden'
  | 'conflict'
  | 'resync_required'
  | 'validation_failed'
  | 'runtime_error'
  | 'unavailable'
  | 'internal'

export interface ApiErrorDto {
  error: string
  /** A stable machine-readable code; the prose in `error` may be reworded. */
  code: ApiErrorCode
  /** Correlates a browser error with server logs without exposing internals. */
  request_id?: string
  /** False for validation/auth/conflict errors that should not be retried. */
  retryable?: boolean
}
