# @orchester/protokoll

`@orchester/protokoll` is the source-only TypeScript mirror of Orchester's
public wire contract. The Rust source of truth lives in
`kisten/protokoll/src`. Browser applications import types, runtime parsers, API
DTOs, and deterministic fixtures from this package instead of declaring local
wire shapes.

## Event Surfaces

Orchester has three event surfaces with different trust and compatibility
requirements:

| Surface | Version | Consumer | Contract |
| --- | --- | --- | --- |
| Flat `Event` | unversioned (`LEGACY_EVENT_SCHEMA_VERSION = 0`) | existing CLI and adapters | Compatibility-only display events |
| `HarnessEvent` | durable v1/v2 | Rust runtime, audit, recovery | Internal journal with governance and tool data |
| `UiEventEnvelope` | browser v1 | WebUI, desktop WebView, website fixtures | Redacted projection with stable IDs and sequence |

Do not send `HarnessEvent` directly to a browser. It can contain workspace
identity, provider-derived data, observations, hashes, and other durable state
that the public UI contract intentionally omits.

## Browser Envelope

Every live or replayed browser event is a `UiEventEnvelope`:

```ts
import {
  parseUiEventEnvelopeJson,
  type UiEventEnvelope,
} from '@orchester/protokoll'

const event: UiEventEnvelope | null = parseUiEventEnvelopeJson(jsonLine)
if (event !== null) {
  render(event)
}
```

The parser rejects unsupported schema versions, zero or fractional sequences,
empty IDs, duplicate JSON keys, unknown fields, invalid approval bindings, and
tool payloads whose `call_id` differs from the envelope. Consumers must not cast
network JSON directly to `UiEventEnvelope`.

Within one run, `(run_id, sequence)` is the ordered replay cursor and `event_id`
is globally stable. Tool lifecycle events are correlated only by `call_id`;
tool names are not unique because concurrent invocations can use the same tool.

## Approvals

Approval requests expose only a redacted action summary and reason. Decisions
must include the current `row_version` and an idempotency key. A stale row is a
normal conflict result, not permission to retry against a newer approval state
without showing it to the user again.

Credential values, raw tool arguments, action hashes, provider payloads, and
workspace identities must never be added to browser DTOs or fixture data.

## Replay And Resync

Use `RunReplayRequestDto.after_sequence` to request events after the last
durably applied sequence. A gap, truncated retention window, or explicit
`ResyncRequiredDto` requires replacing local projection state with a
`RunSnapshotDto`; clients must not guess missing events.

## Fixtures

The package exports four deterministic scenarios:

```ts
import {
  FIXTURE_MANIFEST,
  fixtureScenarioEvents,
} from '@orchester/protokoll'

for (const scenario of FIXTURE_MANIFEST) {
  const events = fixtureScenarioEvents(scenario.id)
  replay(events)
}
```

`happy`, `approval`, `failure`, and `reconnect` are shared by projector tests,
the local WebUI, browser E2E, and the GitHub Pages demo. Each call returns a new
deterministic object graph, and every scenario has contiguous sequences starting
at one. Do not create website-only event shapes.

## Commands

Run from the repository root:

```sh
pnpm --dir apps --filter @orchester/protokoll typecheck
pnpm --dir apps --filter @orchester/protokoll test
cargo test -p orchester-protokoll
```

The Rust command requires the MSVC linker and ARM64 Windows SDK libraries when
the active Rust target is `aarch64-pc-windows-msvc`.
