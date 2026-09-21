# Backend run contract plan

## Goal

Expose the existing self-agent runtime through the loopback API contract already
consumed by the WebUI, while preserving durable run identity, redacted UI events,
request correlation, and explicit cancellation semantics.

## Boundaries

- Keep the TypeScript run shapes in `apps/protokoll/src/api.ts` authoritative.
- Reuse `SelfAgentHost`, `RunEventSink`, and `SqliteRunStore`; do not create a
  second execution engine in the HTTP layer.
- Store only redaction-safe `UiEventEnvelope` values in the network run registry;
  durable harness events remain in the existing SQLite store.
- Require the existing loopback session/CSRF boundary for mutating endpoints.
- Keep this batch focused on start, snapshot, replay, WebSocket streaming, and
  cancellation. Approval decision endpoints remain a follow-up slice.

## Tasks

1. Define Rust DTOs and validation helpers for start, snapshot, replay, stream,
   and cancellation payloads.
2. Add a context-owned run registry with bounded event retention, broadcast
   delivery, cancellation tokens, and monotonic sequence assignment.
3. Bridge `SelfAgentHost` runtime events into the registry using the existing
   vendor-neutral event vocabulary and redaction rules.
4. Add REST routes for start, snapshot, replay, and cancel plus a WebSocket route
   using the server-issued absolute `events_url`.
5. Add focused route and registry tests for validation, replay boundaries,
   cancellation, stale clients, and request IDs.
6. Run focused Rust tests, workspace checks, and the existing WebUI test/build
   suite before merging this branch into `main`.

## Follow-up

- Approval queue/decision routes and durable approval projection.
- Multi-run scheduling and per-agent active-window accounting.
- Persistent network registry recovery after process restart.
