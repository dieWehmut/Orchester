# Orchester Frontend Platform Implementation Plan

> This is the living execution index for the frontend platform. Every checkbox is
> an atomic task. A task is implemented with a failing test first, verified
> green, committed independently, and pushed to the remote feature branch.

**Goal:** Deliver a local WebUI, a shared event-rendering package, a GitHub Pages
site, and a desktop shell on top of the existing Rust runtime without changing
the CLI contract or leaking governed runtime data.

**Architecture:** Rust remains the source of truth. `kisten/netz` owns loopback
HTTP/WebSocket serving and maps safe DTOs from `laufzeit`/`anwendung`; the
browser receives a versioned, redacted UI event envelope with replayable
sequence numbers. `apps/protokoll` owns wire types, `apps/ereignis` owns pure
deterministic projections and Vue run components, `apps/web` owns transport,
auth, routing, and stores, `apps/website` owns a static fixture-driven demo,
and `apps/desktop` embeds the same web bundle.

**Tech Stack:** Rust 2021, Tokio, SQLite, Axum (server), Vue 3, TypeScript,
Vite, Vitest, Playwright, pnpm, Tauri 2, GitHub Actions Pages deployment.

## Non-negotiable execution rules

1. Do not expose `HarnessEvent` internals, credentials, raw provider payloads,
   workspace absolute paths, or unredacted action arguments to the browser.
2. Browser streams use `UiEventEnvelope` with `schema_version`, `run_id`,
   `sequence`, stable `event_id`, and stable `call_id` for tool concurrency.
3. A reconnect starts with a one-time socket ticket and `after_sequence`; the
   server replays the journal or returns an explicit resync-required response.
4. Approval decisions are idempotent, bound to `row_version`, and never inferred
   from a legacy `stopped` event.
5. A full snapshot replaces a collection. Client code never incrementally merges
   a directory snapshot or silently drops a failed response.
6. `apps/ereignis` has no fetch, WebSocket, Pinia, router, or application i18n.
7. `apps/website` never contacts localhost. Its demo replays the same fixtures
   used by `apps/web` tests.
8. GitHub Pages is deployed only by a pinned GitHub Actions workflow from
   `apps/website/dist`; no checked-in generated output is used.
9. Every task below ends with a focused test command, a workspace check, an
   atomic commit, and `git push`.

## Current baseline

- [x] F0-001: Commit the existing `apps/design` source closure and the related
  lockfile hunk (`10f528f`).
- [x] F0-002: Add the shared no-flash appearance bootstrap script and tests
  (`efbc420`).
- [x] F0-003: Add operational layout, breakpoint, z-index, and radius tokens
  (`5e77db2`).
- [x] F0-004: Make busy buttons retain focus and suppress duplicate actions
  (`51f19cf`).
- [x] F0-005: Add roving keyboard navigation to the color scheme picker
  (`7707ac3`).

## Wave F0: repository and design-system foundations

- [x] F0-006: Add `apps/design/src/components/AppField.vue` with label,
  hint, error, `aria-describedby`, and an isolated component test.
- [x] F0-007: Add `AppInput.vue` and its controlled-value/disabled/error tests.
- [x] F0-008: Add `AppTextarea.vue` with deterministic row sizing and tests.
- [x] F0-009: Add `AppSelect.vue` with native keyboard semantics and tests.
- [x] F0-010: Add `AppCheckbox.vue` with indeterminate and label wiring tests.
- [x] F0-011: Add `AppSwitch.vue` with `role=switch` and keyboard tests.
- [x] F0-012: Export all form primitives from `apps/design/src/index.ts`.
- [x] F0-013: Add `apps/design/test/forms.test.ts` for cross-primitive ARIA
  invariants and run the complete design suite.
- [x] F0-014: Add `AppTabs.vue` with roving focus and manual activation tests.
- [x] F0-015: Add `AppSegmentedControl.vue` with selected-state tests.
- [x] F0-016: Add `AppMenu.vue` with Escape, arrow, and outside-click tests.
- [x] F0-017: Add `AppTooltip.vue` with hover/focus and reduced-motion tests.
- [x] F0-018: Add `VisuallyHidden.vue` and its DOM visibility test.
- [x] F0-019: Export navigation primitives and add `navigation.test.ts`.
- [x] F0-020: Add `AppDialog.vue` with focus trap, restore-focus, and Escape
  tests.
- [x] F0-021: Add `AppDrawer.vue` with side placement and body-scroll-lock
  tests.
- [x] F0-022: Add `AppPopover.vue` with anchor placement and dismiss tests.
- [x] F0-023: Export overlay primitives and add `overlays.test.ts`.
- [x] F0-024: Add `InlineAlert.vue` and live-region tests.
- [x] F0-025: Add `EmptyState.vue` and action-slot tests.
- [x] F0-026: Add `SkeletonBlock.vue` and reduced-motion tests.
- [x] F0-027: Add `ProgressBar.vue` with bounded-value tests.
- [x] F0-028: Add `ToastRegion.vue` with queue, timeout, and dismiss tests.
- [x] F0-029: Export feedback primitives and add `feedback.test.ts`.
- [x] F0-030: Add `@lucide/vue` as the icon dependency and update only
  the lockfile entries required by the design package.
- [x] F0-031: Replace the hand-drawn theme toggle glyph with Lucide icons and
  add an icon accessibility test.
- [x] F0-032: Add design package CSS custom-property snapshot tests for dark,
  light, and all four color schemes.
- [x] F0-033: Add `apps/design/src/index.css` reset contract and verify it does
  not style consumer layout elements.
- [x] F0-034: Add design package README with import examples and supported
  keyboard contracts.
- [x] F0-035: Run package typecheck/test and publish the design wave commit
  checkpoint.

## Wave P1: browser protocol and fixtures

- [x] P1-001: Add Rust `kisten/protokoll/src/ui.rs` envelope types with
  `UiEventEnvelope`, `UiEventKind`, and strict unknown-field rejection.
- [x] P1-002: Add Rust `UiEventEnvelope` sequence and run-binding validation.
- [x] P1-003: Add Rust redaction tests for paths, credentials, provider data,
  and action arguments.
- [x] P1-004: Add stable `call_id` to the browser-facing tool lifecycle type.
- [x] P1-005: Add explicit tool states queued/running/succeeded/failed/cancelled.
- [x] P1-006: Add approval request/resolution DTOs with `approval_id`,
  `row_version`, risk summary, and redacted action.
- [x] P1-007: Add Rust round-trip tests for every UI event variant.
- [x] P1-008: Export the UI protocol from `kisten/protokoll/src/lib.rs`.
- [x] P1-009: Add TypeScript `UiEventEnvelope` and discriminated UI event types.
- [x] P1-010: Add TypeScript guards for envelope version, sequence, and IDs.
- [x] P1-011: Add TypeScript `ApiErrorDto`, `RunSnapshotDto`, and resync DTOs.
- [x] P1-012: Add TypeScript approval queue/decision DTOs matching Rust fields.
- [x] P1-013: Add TypeScript tool invocation DTOs keyed only by `call_id`.
- [x] P1-014: Add JSON fixture builders for happy, approval, failure, and
  reconnect paths.
- [x] P1-015: Add protocol tests for duplicate, missing, and unknown fields.
- [x] P1-016: Add protocol schema-version compatibility tests for legacy v1
  and UI v1.
- [x] P1-017: Add a generated fixture manifest consumed by website and web.
- [x] P1-018: Add protocol documentation in `apps/protokoll/README.md`.
- [ ] P1-019: Run TypeScript and Rust protocol checks with the available MSVC
  linker and record the toolchain requirement. Local ARM64 validation is
  blocked until Visual Studio Build Tools are installed; see
  `docs/BUILD-TOOLCHAIN.md`.
- [x] P1-020: Commit and push the protocol wave as separate Rust and TypeScript
  commits.

## Wave N1: Rust `kisten/netz` service

- [x] N1-001: Create `kisten/netz/Cargo.toml` with axum, tower, tokio, and
  static-file feature boundaries.
- [x] N1-002: Add `kisten/netz/src/lib.rs` and a typed `ServerConfig`.
- [x] N1-003: Add loopback bind validation and port `0` allocation tests.
  - [x] N1-003a: Validate that configured bind addresses are loopback-only.
  - [x] N1-003b: Allocate and expose an ephemeral port-`0` listener.
- [x] N1-004: Add workspace selection validation using `OrchesterPaths`.
  - [x] N1-004a: Wire the application-layer path dependency into `netz`.
  - [x] N1-004b: Validate and canonicalize selected workspace directories.
- [x] N1-005: Add server lifecycle start/stop tests without a real socket.
  - [x] N1-005a: Define and test the ordered server lifecycle state machine.
  - [x] N1-005b: Add observable shutdown control without opening a socket.
- [x] N1-006: Add `GET /api/v1/health` and typed health tests.
  - [x] N1-006a: Define the redaction-safe `HealthDto` wire contract.
  - [x] N1-006b: Add the Rust health handler and router.
  - [x] N1-006c: Verify status, JSON content type, and unknown-route behavior.
- [x] N1-007: Add `GET /api/v1/bootstrap` with safe workspace/status data.
  - [x] N1-007a: Define the path-free bootstrap wire contract.
  - [x] N1-007b: Project safe workspace and lifecycle state in Rust.
  - [x] N1-007c: Route and verify the bootstrap response over HTTP.
- [x] N1-008: Add request ID middleware and response header tests.
  - [x] N1-008a: Generate UUID request IDs when clients omit the header.
  - [x] N1-008b: Propagate client/generated IDs on every routed response.
- [x] N1-009: Add cookie session bootstrap and CSRF token validation.
  - [x] N1-009a: Define the session bootstrap and CSRF response contract.
  - [x] N1-009b: Store only hashed expiring session/CSRF tokens.
  - [x] N1-009c: Enforce cookie and CSRF headers on state-changing routes.
- [x] N1-010: Add fragment-token exchange that never persists tokens.
  - [x] N1-010a: Define one-time fragment exchange request/response DTOs.
  - [x] N1-010b: Consume registered fragment token hashes exactly once.
  - [x] N1-010c: Add POST exchange route with URL-fragment-safe semantics.
- [x] N1-011: Add typed API error mapping and JSON content-type tests.
  - [x] N1-011a: Define stable machine-readable API error codes.
  - [x] N1-011b: Map server failures to redacted JSON error responses.
  - [x] N1-011c: Verify error status/content type across route failures.
- [x] N1-012: Add agent catalog route backed by `Registry`.
  - [x] N1-012a: Define a versioned, path-free agent catalog contract.
  - [x] N1-012b: Project registry capabilities and availability safely.
  - [x] N1-012c: Bind the workspace registry into server context.
  - [x] N1-012d: Route and verify the agent catalog over HTTP.
- [ ] N1-013: Add model catalog route backed by `SelfAgentHost`.
  - [x] N1-013a: Define the versioned, endpoint-free model catalog contract.
  - [x] N1-013b: Project `SelfAgentHost` model choices into safe DTOs.
  - [x] N1-013c: Bind one read-only model host to server context.
  - [ ] N1-013d: Route and verify the model catalog over HTTP.
- [ ] N1-014: Add session list/detail routes with pagination cursors.
- [ ] N1-015: Add run snapshot route backed by `RunStore`.
- [ ] N1-016: Add run submit route with client request idempotency.
- [ ] N1-017: Add run cancel route with explicit cancellation result.
- [ ] N1-018: Add resume catalog and resume route with workspace binding.
- [ ] N1-019: Add approval queue route with redacted summaries.
- [ ] N1-020: Add approve route with row-version and idempotency checks.
- [ ] N1-021: Add deny route with the same conflict semantics.
- [ ] N1-022: Add files tree route that returns a complete snapshot.
- [ ] N1-023: Add read-only file preview route with byte and path limits.
- [ ] N1-024: Add settings/provider routes without credential echo.
- [ ] N1-025: Add permissions/status route and safe failure responses.
- [ ] N1-026: Expose a runtime event adapter that maps durable events to UI
  envelopes without leaking `HarnessEvent` payloads.
- [ ] N1-027: Add per-workspace actor locking for mutable `SelfAgentHost`.
- [ ] N1-028: Add bounded event channels and backpressure tests.
- [ ] N1-029: Add one-time WebSocket ticket issuance route.
- [ ] N1-030: Add WebSocket handshake validation and heartbeat handling.
- [ ] N1-031: Add sequence replay from `RunStore` and gap detection.
- [ ] N1-032: Add resync-required response when replay retention is exceeded.
- [ ] N1-033: Add terminal stream closure for completed/failed/cancelled runs.
- [ ] N1-034: Add CORS and origin policy tests for loopback clients.
- [ ] N1-035: Add static-file fallback and SPA index resolution.
- [ ] N1-036: Add `orchester web` CLI command with bind/port options.
- [ ] N1-037: Add CLI JSON output for the allocated server URL.
- [ ] N1-038: Add server integration tests using a scripted model and temp
  `OrchesterPaths`.
- [ ] N1-039: Add Rust documentation for local threat model and limitations.
- [ ] N1-040: Run Rust fmt, clippy, unit, and integration gates; commit/push.

## Wave E1: `apps/ereignis` pure projection package

- [ ] E1-001: Scaffold package metadata, tsconfig, Vitest setup, and source
  index with only Vue/protokoll/design dependencies.
- [ ] E1-002: Define `RunView`, `TurnView`, `TimelineItem`, and connection-
  independent labels in `src/model/run-view.ts`.
- [ ] E1-003: Define event-key helpers and exhaustive event-kind tests.
- [ ] E1-004: Implement sequence deduplication and duplicate-event tests.
- [ ] E1-005: Implement out-of-order buffering and gap markers.
- [ ] E1-006: Implement snapshot replacement semantics.
- [ ] E1-007: Implement user/assistant content projection with delta batching.
- [ ] E1-008: Implement cumulative usage replacement without double counting.
- [ ] E1-009: Implement tool invocation aggregation by `call_id`.
- [ ] E1-010: Test two concurrent same-name tool calls.
- [ ] E1-011: Implement validator feedback and todo projection.
- [ ] E1-012: Implement approval requested/resolved/expired/stale projection.
- [ ] E1-013: Implement interrupted-unknown-outcome stop state.
- [ ] E1-014: Expose `useRunProjection` with reset/snapshot/apply only.
- [ ] E1-015: Add happy-path, reconnect, approval, and failure fixtures.
- [ ] E1-016: Build `RunTimeline.vue` and bottom-aware autoscroll tests.
- [ ] E1-017: Build `TimelineItem.vue`, user, assistant, and reasoning blocks.
- [ ] E1-018: Build `ToolInvocationCard.vue` with safe collapsed details.
- [ ] E1-019: Build `ValidationSummary.vue` and `TodoList.vue`.
- [ ] E1-020: Build `ApprovalSummaryCard.vue` emitting decision intents only.
- [ ] E1-021: Build `RunStopBanner.vue` for unknown and terminal outcomes.
- [ ] E1-022: Build `UsageSummary.vue`, `FileChangeList.vue`, and status badge.
- [ ] E1-023: Add component keyboard/focus/aria tests.
- [ ] E1-024: Export the public package surface and forbid app imports by
  dependency tests.
- [ ] E1-025: Run typecheck/test and commit/push the shared event package.

## Wave W1: local WebUI application

- [ ] W1-001: Scaffold Vite app, entrypoint, app shell, and no-flash bootstrap.
- [ ] W1-002: Add router with workspace/settings/not-found lazy routes.
- [ ] W1-003: Add English, Simplified Chinese, and Traditional Chinese locale
  dictionaries with stable message keys.
- [ ] W1-004: Add typed HTTP client with cookies, CSRF, request IDs, and abort.
- [ ] W1-005: Add API error normalization and retry classification.
- [ ] W1-006: Add bootstrap store and fragment-token URL cleanup.
- [ ] W1-007: Add session, model, workspace, settings, and layout stores.
- [ ] W1-008: Add run store backed by the pure ereignis projector.
- [ ] W1-009: Add optimistic user row deduplication by client request ID.
- [ ] W1-010: Add one-time-ticket WebSocket transport with bounded backoff.
- [ ] W1-011: Add heartbeat, terminal close, replay, and resync callbacks.
- [ ] W1-012: Build desktop three-column `WorkspaceShell`.
- [ ] W1-013: Build `WorkspaceHeader`, session rail, and inspector dock.
- [ ] W1-014: Build responsive drawers for 900-1279px layouts.
- [ ] W1-015: Build single-column mobile header and full-height drawers.
- [ ] W1-016: Build session list, loading, empty, error, and unavailable states.
- [ ] W1-017: Build agent/model/effort pickers with unavailable handling.
- [ ] W1-018: Build composer Enter/Shift+Enter and size-limit behavior.
- [ ] W1-019: Build submit, cancel, and busy double-submit protection.
- [ ] W1-020: Integrate run panel, connection banner, footer, and resync UI.
- [ ] W1-021: Build approval queue and decision dialog with stale handling.
- [ ] W1-022: Build read-only file tree, preview, and diff views.
- [ ] W1-023: Build settings/provider/credential/appearance sections with
  independent save errors.
- [ ] W1-024: Add keyboard navigation, focus restoration, and body-lock tests.
- [ ] W1-025: Add API mock server fixtures for deterministic unit tests.
- [ ] W1-026: Add Playwright real-server workspace smoke test.
- [ ] W1-027: Add Playwright approval, cancel, resume, and reconnect flows.
- [ ] W1-028: Add 1440x900, 1024x768, and 390x844 visual/no-overlap checks.
- [ ] W1-029: Add browser accessibility assertions for dialogs, drawers, and
  live regions.
- [ ] W1-030: Add production build manifest and embedded-asset verification.
- [ ] W1-031: Run full app typecheck/test/build/e2e and commit/push.

## Wave S1: GitHub Pages website

- [ ] S1-001: Scaffold the independent Vite website package and base-path helper.
- [ ] S1-002: Copy `index.html` to `404.html` at closeBundle for deep links.
- [ ] S1-003: Add lazy router, three locales, and not-found view.
- [ ] S1-004: Build site shell with keyboard-safe mobile navigation.
- [ ] S1-005: Build product hero using a real WebUI screenshot asset.
- [ ] S1-006: Build capability, adapter, and governance content sections.
- [ ] S1-007: Build architecture and install views from typed content modules.
- [ ] S1-008: Add fixture-driven `DemoRuntime` with no network access.
- [ ] S1-009: Reuse ereignis timeline and tool/approval components in the demo.
- [ ] S1-010: Add explicit simulated-demo state and deterministic replay tests.
- [ ] S1-011: Add optional Giscus comments gated by complete environment config.
- [ ] S1-012: Add metadata, favicon, screenshot, and social preview assets.
- [ ] S1-013: Add Pages build workflow with SHA-pinned actions, frozen pnpm
  install, typecheck, tests, build, artifact upload, and deployment.
- [ ] S1-014: Add workflow preview smoke for `/Orchester/` base path and 404.
- [ ] S1-015: Add Pages environment permissions and concurrency cancellation.
- [ ] S1-016: Run website unit/build/base-path checks and commit/push.

## Wave T1: desktop shell

- [ ] T1-001: Scaffold Tauri 2 package with an isolated Rust workspace.
- [ ] T1-002: Add embedded netz server startup on an ephemeral loopback port.
- [ ] T1-003: Create the window hidden and reveal only after web readiness.
- [ ] T1-004: Add strict CSP, navigation allowlist, and minimal capabilities.
- [ ] T1-005: Add menu actions for reload, logs, workspace folder, and close.
- [ ] T1-006: Add graceful server shutdown and duplicate-start protection.
- [ ] T1-007: Add platform build scripts that build web before Tauri.
- [ ] T1-008: Add Windows/macOS/Linux CI artifact smoke workflow.
- [ ] T1-009: Add desktop security and lifecycle tests.
- [ ] T1-010: Run supported local compile checks and commit/push.

## Wave Q1: release and maintenance gates

- [ ] Q1-001: Add root `pnpm build` and `pnpm check` scripts.
- [ ] Q1-002: Add a frontend CI workflow for frozen install/typecheck/test/build.
- [ ] Q1-003: Add Rust plus frontend protocol compatibility CI.
- [ ] Q1-004: Add Playwright artifact upload on failure.
- [ ] Q1-005: Add dependency audit and lockfile-drift check.
- [ ] Q1-006: Add release checklist documenting MSVC linker prerequisites.
- [ ] Q1-007: Add changelog entries for protocol, web, Pages, and desktop.
- [ ] Q1-008: Run a clean clone verification from the pushed branch.
- [ ] Q1-009: Tag the first integrated frontend-platform release candidate.
- [ ] Q1-010: Record residual risks and accepted limitations in the decision log.

## Checkpoint commands

```text
pnpm --dir apps install --frozen-lockfile
pnpm --dir apps typecheck
pnpm --dir apps test
pnpm --dir apps --filter @orchester/web build
pnpm --dir apps --filter @orchester/website build
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
git diff --check
git push
```

The Rust commands must run from an MSVC developer shell whose `link.exe` is
the MSVC linker; the current MSYS `link.exe` on the ARM64 machine is not a
valid replacement.
