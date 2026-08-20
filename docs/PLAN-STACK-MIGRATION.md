# Frontend Stack Migration Plan

This is the execution index for the requested frontend stack. It supplements
`docs/PLAN-FRONTENDS.md`; existing completed work remains valid unless a task
below explicitly replaces its implementation.

## Required Stack

- Rust core and local HTTP/WebSocket API remain the source of truth.
- Local WebUI: Vue 3, TypeScript, Vite, Tailwind CSS, shadcn-vue source
  components, Pinia, Vue Router.
- WebUI code editor: Monaco. Web terminal: xterm.js.
- Desktop: Tauri 2 hosting the production WebUI bundle.
- Project site: Astro with Vue islands and Tailwind CSS.
- Comments: Giscus, injected only when the complete configuration exists.
- Package manager: pnpm. Pages deployment: GitHub Actions only.

## Branch and Commit Rules

The integration branch is `feat/frontend-platform`. Each wave starts from the
latest integration head and uses a dedicated branch. Every checkbox is one
small reviewable commit: write a focused test or contract first, run the
focused gate, commit, and push. No generated `dist` output is committed.

Branch order:

1. `feat/web-stack-foundation` -> `feat/frontend-platform`
2. `feat/web-pinia-migration` -> `feat/frontend-platform`
3. `feat/web-editor-terminal` -> `feat/frontend-platform`
4. `feat/website-astro` -> `feat/frontend-platform`
5. `feat/desktop-runtime` -> `feat/frontend-platform`
6. `feat/platform-e2e` -> `feat/frontend-platform`

Each branch may contain stacked pull requests, but a later branch must be
rebased or recreated from the merged integration head before opening its PR.

## Commands and Entrypoints

| Surface | Source | Development command | Local URL/output |
| --- | --- | --- | --- |
| WebUI | `apps/web` | `pnpm --dir apps --filter @orchester/web dev` | `http://127.0.0.1:4173/` |
| Site | `apps/website` | `pnpm --dir apps --filter @orchester/website dev` | Astro dev URL |
| Pages | `.github/workflows/pages.yml` | push/Actions workflow | `https://dieWehmut.github.io/Orchester/` |
| Desktop | `apps/desktop` | `pnpm --dir apps --filter @orchester/desktop dev` | Tauri window |

WebUI production build is the only bundle consumed by Tauri and `kisten/netz`.
The website is a separate Astro app and never calls localhost.

## W2: WebUI Foundation (`feat/web-stack-foundation`)

- [x] W2-001 Add pinned Pinia, Tailwind CSS, Tailwind Vite plugin, shadcn
  utility, and Lucide dependencies to `apps/web/package.json`.
- [x] W2-002 Regenerate only the required `apps/pnpm-lock.yaml` importer and
  package entries with a frozen-install check.
- [x] W2-003 Add the Tailwind Vite plugin without changing the dev port.
- [x] W2-004 Add a dedicated Tailwind entry stylesheet and preserve the shared
  appearance bootstrap order.
- [x] W2-005 Add a CSS-variable bridge from `@orchester/design` tokens to the
  Tailwind theme; do not duplicate color values in components.
- [x] W2-006 Add `components.json` for shadcn-vue source generation and record
  the alias contract.
- [x] W2-007 Add the `cn()` utility using `clsx` and `tailwind-merge`.
- [x] W2-008 Test `cn()` deterministic merge precedence and arbitrary values.
- [ ] W2-009 Add the first shadcn-vue Button source component with keyboard and
  disabled semantics.
- [ ] W2-010 Add Button variants for primary, quiet, destructive, and icon-only
  actions using the existing operational palette.
- [ ] W2-011 Add Button tests for focus retention, busy state, and aria-disabled.
- [ ] W2-012 Add shadcn-vue Badge source component and status variants.
- [ ] W2-013 Add Badge tests for text overflow and contrast classes.
- [ ] W2-014 Add shadcn-vue Card source component with radius <= 8px.
- [ ] W2-015 Add Card tests proving no nested layout assumptions.
- [ ] W2-016 Add Input, Textarea, Select, Checkbox, and Switch source wrappers.
- [ ] W2-017 Add form wrapper tests for labels, descriptions, and errors.
- [ ] W2-018 Add Dialog and Sheet source wrappers with focus restoration.
- [ ] W2-019 Add Dialog/Sheet tests for Escape, overlay click, and body lock.
- [ ] W2-020 Add Tabs and Dropdown source wrappers with keyboard navigation.
- [ ] W2-021 Add Tabs/Dropdown tests for roving focus and selected semantics.
- [ ] W2-022 Add Tooltip source wrapper for unfamiliar icon buttons.
- [ ] W2-023 Add Tooltip tests for focus, hover, and reduced motion.
- [ ] W2-024 Define a single icon import boundary using the maintained
  `@lucide/vue` package.
- [ ] W2-025 Replace new hand-drawn icons in WebUI files with Lucide icons.
- [ ] W2-026 Add Tailwind content/source scanning for Vue and TS files.
- [ ] W2-027 Add a CSS snapshot test for dark/light and four color schemes.
- [ ] W2-028 Add a responsive utility contract for 1280, 1024, and 390 widths.
- [ ] W2-029 Add a lint-like test rejecting gradient-orb and giant hero classes.
- [ ] W2-030 Run WebUI typecheck, unit tests, and production build.

## W3: Pinia Migration (`feat/web-pinia-migration`)

- [x] W3-001 Add a test-only Pinia factory that creates an isolated store per
  test and never shares active Pinia state.
- [x] W3-002 Add the application Pinia plugin to `main.ts`.
- [ ] W3-003 Define the bootstrap store with typed state and actions.
- [ ] W3-004 Move fragment-token exchange and URL cleanup into bootstrap actions.
- [ ] W3-005 Test bootstrap loading, success, and error states in Pinia.
- [ ] W3-006 Define the session store with list cursor and selected session state.
- [ ] W3-007 Move session pagination and detail loading into Pinia actions.
- [ ] W3-008 Test session refresh, load-more, empty, and partial failure states.
- [ ] W3-009 Define the run store with lifecycle and connection state.
- [ ] W3-010 Move projector application behind Pinia actions.
- [ ] W3-011 Preserve optimistic request IDs and server echo deduplication.
- [ ] W3-012 Test run snapshot replacement and sequence gap handling.
- [ ] W3-013 Define approval, model, workspace, settings, and layout stores.
- [ ] W3-014 Test each store's local failure isolation.
- [ ] W3-015 Add typed store selectors for header, rail, transcript, and dock.
- [ ] W3-016 Replace `createAppStores` injection with Pinia `use*Store` calls.
- [ ] W3-017 Keep a compatibility adapter for isolated non-component tests.
- [ ] W3-018 Remove the old injection key after all consumers migrate.
- [ ] W3-019 Add a Pinia reset helper for logout and workspace changes.
- [ ] W3-020 Test reset does not leak events, credentials, or selected sessions.
- [ ] W3-021 Add dev-only store inspection names without payload logging.
- [ ] W3-022 Run full WebUI typecheck, tests, and build after migration.

## W4: WebUI Workspace Completion (`feat/web-pinia-migration`)

- [ ] W4-001 Add agent picker API client and typed unavailable state.
- [ ] W4-002 Add model catalog store and loading/error/empty views.
- [ ] W4-003 Add effort picker with accessible segmented control semantics.
- [ ] W4-004 Connect run socket ticket URL to the Pinia run store.
- [ ] W4-005 Apply event, resync, and liveness status callbacks to the store.
- [ ] W4-006 Add snapshot fetch after `resync_required`.
- [ ] W4-007 Add approval queue API and Pinia approval store.
- [ ] W4-008 Add approval dialog with row-version and idempotency handling.
- [ ] W4-009 Add stale/expired/already-applied decision rendering.
- [ ] W4-010 Add read-only file tree snapshot store.
- [ ] W4-011 Add file preview dialog with size and path policy limits.
- [ ] W4-012 Add diff preview with safe text rendering and truncation.
- [ ] W4-013 Add provider settings form with section-local save errors.
- [ ] W4-014 Add credential-present state without echoing secret values.
- [ ] W4-015 Add appearance settings backed by shared design storage.
- [ ] W4-016 Add responsive drawer tests at all three target viewports.
- [ ] W4-017 Add keyboard and focus restoration tests for every overlay.
- [ ] W4-018 Add browser no-overlap screenshot assertions.
- [ ] W4-019 Add API mock server fixtures for deterministic component tests.
- [ ] W4-020 Run the WebUI vertical-slice gate and push a PR.

## W5: Monaco and xterm (`feat/web-editor-terminal`)

- [ ] W5-001 Add `monaco-editor` and its Vite worker strategy.
- [ ] W5-002 Add Monaco lazy loader with cancellation and one-instance caching.
- [ ] W5-003 Add typed editor options limited to read-only workspace files first.
- [ ] W5-004 Add `CodeEditor.vue` shell with loading and unsupported fallbacks.
- [ ] W5-005 Add editor model disposal on file/session change.
- [ ] W5-006 Add editor tests for loading, content, language, and disposal.
- [ ] W5-007 Add path redaction and maximum file-size guards before Monaco load.
- [ ] W5-008 Add diff editor mode for read-only change previews.
- [ ] W5-009 Add diff editor tests for truncation and binary refusal.
- [ ] W5-010 Add `xterm` and `xterm-addon-fit` dependencies.
- [ ] W5-011 Add terminal transport interface independent of WebSocket creation.
- [ ] W5-012 Add terminal session Pinia store with connection lifecycle.
- [ ] W5-013 Add `TerminalPanel.vue` with fit-on-resize behavior.
- [ ] W5-014 Add terminal output batching and bounded scrollback.
- [ ] W5-015 Add terminal input/cancel/close controls with icon tooltips.
- [ ] W5-016 Add terminal tests using an injected xterm adapter.
- [ ] W5-017 Refuse terminal startup when the server lacks an approved session.
- [ ] W5-018 Add reconnect and stale-session recovery tests.
- [ ] W5-019 Add browser smoke test for editor and terminal drawers.
- [ ] W5-020 Run editor/terminal typecheck, tests, and build.

## W6: Rust Web Transport Contract (`feat/netz-run-api`)

- [ ] W6-001 Define a per-workspace run manager ownership boundary.
- [ ] W6-002 Define typed start response and one-time socket ticket issuance.
- [ ] W6-003 Persist run event sequence and bounded replay metadata.
- [ ] W6-004 Add snapshot projection that never exposes HarnessEvent internals.
- [ ] W6-005 Add replay endpoint with `after_sequence` and bounded `limit`.
- [ ] W6-006 Add resync-required response for retention gaps.
- [ ] W6-007 Add WebSocket event endpoint with ticket validation.
- [ ] W6-008 Add heartbeat/liveness server policy and close codes.
- [ ] W6-009 Add cancel endpoint with idempotent terminal response.
- [ ] W6-010 Add approval list and row-version decision endpoints.
- [ ] W6-011 Add HTTP error DTO coverage for auth, conflict, unavailable, and
  resync cases.
- [ ] W6-012 Add workspace scope and loopback authentication tests.
- [ ] W6-013 Add backpressure and slow-client disconnect tests.
- [ ] W6-014 Add server integration fixtures for happy, approval, and reconnect.
- [ ] W6-015 Run GNU Rust netz tests and clippy with documented baseline.

## W7: Astro Site (`feat/website-astro`)

- [ ] W7-001 Replace the website Vite scaffold with Astro 7 and Vue integration.
- [ ] W7-002 Add Astro Tailwind integration and shared design token import.
- [ ] W7-003 Preserve pnpm workspace scripts and frozen lockfile installs.
- [ ] W7-004 Add `normalizeBasePath` for GitHub Pages `/Orchester/` hosting.
- [ ] W7-005 Copy built `index.html` to `404.html` during the Astro build.
- [ ] W7-006 Add typed site route/content definitions.
- [ ] W7-007 Add Home, Architecture, Install, and NotFound Astro pages.
- [ ] W7-008 Add Vue island SiteHeader with mobile disclosure navigation.
- [ ] W7-009 Add keyboard Escape and focus restoration for mobile navigation.
- [ ] W7-010 Add SiteFooter and external-link security attributes.
- [ ] W7-011 Port the real WebUI screenshot hero without gradient/SVG hero art.
- [ ] W7-012 Add capability, adapter, and governance content sections.
- [ ] W7-013 Add architecture flow and boundary content from typed modules.
- [ ] W7-014 Add install commands from repository metadata.
- [ ] W7-015 Add deterministic simulated RunDemo Vue island.
- [ ] W7-016 Reuse `@orchester/ereignis` fixtures and components in the demo.
- [ ] W7-017 Assert demo never calls fetch, WebSocket, or localhost.
- [ ] W7-018 Add explicit simulated state and reduced-motion replay controls.
- [ ] W7-019 Add Giscus component gated by complete env configuration.
- [ ] W7-020 Add Giscus theme synchronization without login UI.
- [ ] W7-021 Add metadata, favicon, canonical, and social image assets.
- [ ] W7-022 Add responsive and accessibility tests for all pages.
- [ ] W7-023 Add Pages workflow Astro build and artifact path checks.
- [ ] W7-024 Add base-path preview smoke and deep-link 404 checks.
- [ ] W7-025 Add workflow permissions, environment, concurrency, and pinned SHAs.
- [ ] W7-026 Run Astro typecheck, tests, build, and Pages smoke.

## W8: Tauri Runtime (`feat/desktop-runtime`)

- [ ] W8-001 Make desktop depend on the WebUI production build artifact.
- [ ] W8-002 Add embedded netz server startup on an ephemeral loopback port.
- [ ] W8-003 Add validated workspace selection before server startup.
- [ ] W8-004 Create the window hidden until the local page is ready.
- [ ] W8-005 Navigate only to exact Tauri local origins and approved loopback.
- [ ] W8-006 Keep strict production/dev CSP and minimal capabilities.
- [ ] W8-007 Add fragment-token bootstrap for the desktop window.
- [ ] W8-008 Add reload, open logs, and open workspace menu commands.
- [ ] W8-009 Add close confirmation while a run or terminal is active.
- [ ] W8-010 Add graceful server and WebSocket shutdown.
- [ ] W8-011 Add duplicate-start protection and restart recovery.
- [ ] W8-012 Add platform-specific path and port tests.
- [ ] W8-013 Add Win/macOS/Linux build scripts that build web first.
- [ ] W8-014 Add CI artifact smoke workflow for all supported platforms.
- [ ] W8-015 Verify no broad shell, opener, or remote navigation capability.
- [ ] W8-016 Run supported local metadata/compile checks and package smoke.

## W9: Cross-Surface Release Gate (`feat/platform-e2e`)

- [ ] W9-001 Add Playwright server fixture for real `orchester web`.
- [ ] W9-002 Add workspace submit/live event/reconnect test.
- [ ] W9-003 Add approval approve/deny/stale/expired test.
- [ ] W9-004 Add cancel/resume/unknown-outcome test.
- [ ] W9-005 Add Monaco file preview and diff test.
- [ ] W9-006 Add xterm terminal lifecycle test.
- [ ] W9-007 Add 1440x900 no-overlap screenshot test.
- [ ] W9-008 Add 1024x768 drawer screenshot test.
- [ ] W9-009 Add 390x844 single-column screenshot test.
- [ ] W9-010 Add keyboard focus and reduced-motion browser assertions.
- [ ] W9-011 Add Astro demo isolation and base-path browser test.
- [ ] W9-012 Add Tauri bundled-web smoke against a fake loopback server.
- [ ] W9-013 Add root `pnpm check` script for typecheck/test/build.
- [ ] W9-014 Add CI frontend matrix for WebUI, Astro, and desktop metadata.
- [ ] W9-015 Run Rust workspace checks and all frontend release gates.
- [ ] W9-016 Publish a versioned protocol compatibility note.
- [ ] W9-017 Merge only after all required branch PRs are green.

## W10: Agent Fleet and Codex-Style Workspace (`feat/web-pinia-migration`)

This wave defines the visible agent fleet before provider-specific process
integration. Availability (installed/configured/authenticated) is separate
from activity (idle/running/waiting approval). A browser cannot inspect
arbitrary operating-system windows, so `active_windows` means Orchester-owned
workspace views until a Tauri window registry supplies a stronger source.

- [x] W10-001 Add `AgentRuntimeSummaryDto` and a versioned fleet snapshot to
  `apps/protokoll`; keep provider IDs, display labels, and icon keys safe for
  rendering and reject unknown fields at the boundary.
- [x] W10-002 Add availability and activity enums with independent guards;
  never derive `running` from process existence alone.
- [x] W10-003 Add active window/session/run/subagent counts and an explicit
  `window_count_source` (`managed_sessions` or `tauri_windows`).
- [x] W10-004 Add redacted heartbeat and last-error fields; prohibit absolute
  paths, credentials, command lines, and transcript contents in the DTO.
- [x] W10-005 Add deterministic fleet fixtures for Codex, Claude Code,
  DeepSeek, unavailable, and auth-required states.
- [x] W10-006 Add `/api/v1/agents/status` client and snapshot refresh policy.
- [ ] W10-007 Add a WebSocket status-frame contract with sequence and resync
  handling, independent from run event frames.
- [x] W10-008 Add a Pinia agent store with isolated loading/error/stale state.
- [x] W10-009 Add provider metadata and Lucide icon mapping without hand-drawn
  SVG or provider brand asset copying.
- [x] W10-010 Add `AgentFleetPanel` with one row per known agent, icon, status
  dot, accessible label, and active counts.
- [ ] W10-011 Add compact running-subagent badges to the owning session/project
  row; parent idle plus child running must remain distinguishable.
- [ ] W10-012 Add an agent detail drawer showing sessions, runs, subagents,
  heartbeat age, and redacted error state.
- [ ] W10-013 Add Codex thread status projection for `not_loaded`, `idle`,
  `active`, and `system_error`, plus running-turn count updates.
- [ ] W10-014 Add Claude hook bridge projection for SessionStart/End,
  SubagentStart/Stop, Stop, and Notification with parent-agent linkage.
- [ ] W10-015 Add DeepSeek descendant indexing projection and explicit teardown
  semantics for child handles.
- [ ] W10-016 Add the Codex-style left rail sections: brand/new chat,
  projects, sessions, and agent fleet; preserve keyboard navigation.
- [x] W10-017 Add a dedicated `OrchesterMark` component for the center empty
  state; it must be an original local mark and disappear after first submit or
  first run event.
- [x] W10-018 Add empty-state transition tests proving the large mark is absent
  once a conversation has started, even while the first event is pending.
- [x] W10-019 Add compact connecting/running state for a started run with no
  events; do not re-show the marketing-style empty state.
- [ ] W10-020 Add bottom composer behavior matching the reference interaction:
  multiline input, Enter submit, Shift+Enter newline, cancel while active.
- [ ] W10-021 Add project/session selection and a visible selected-row state;
  loading failures must retain the prior selection.
- [ ] W10-022 Add responsive drawers at tablet/mobile widths without hiding
  projects, sessions, agents, approvals, or settings.
- [ ] W10-023 Add no-overlap visual checks at 1440x900, 1024x768, and 390x844.
- [ ] W10-024 Add keyboard/focus tests for rail disclosure, agent drawer,
  composer, and empty-state controls.
- [ ] W10-025 Add a Rust redaction test for fleet snapshots and status frames.
- [ ] W10-026 Add Rust registry/provider status aggregation with unavailable
  and auth-required outcomes that do not fail the whole snapshot.
- [ ] W10-027 Add Codex app-server status adapter integration tests using
  thread/status/changed and turn start/complete fixtures.
- [ ] W10-028 Add Claude hook ingestion tests for parent/child lifecycle and
  redacted transcript paths.
- [ ] W10-029 Add DeepSeek subagent-count integration fixtures and stale-child
  cleanup tests.
- [ ] W10-030 Add Tauri managed-window registry source and map its count to the
  same fleet DTO without changing browser semantics.
- [ ] W10-031 Add browser E2E coverage for multiple active sessions and a
  parent with a running subagent.
- [ ] W10-032 Add browser E2E coverage for unavailable/auth-required agents and
  reconnecting fleet status.
- [ ] W10-033 Add a release screenshot fixture generated from the WebUI build;
  website may consume the asset but must not call localhost.
- [ ] W10-034 Run the fleet/workspace wave gate before merging its branch.

## Acceptance Gates

Every WebUI commit must pass the narrow package command plus `git diff --check`.
Wave gates are:

```powershell
pnpm --dir apps install --frozen-lockfile
pnpm --dir apps --filter @orchester/protokoll typecheck
pnpm --dir apps --filter @orchester/protokoll test
pnpm --dir apps --filter @orchester/web typecheck
pnpm --dir apps --filter @orchester/web test
pnpm --dir apps --filter @orchester/web build
```

The site gate additionally runs Astro typecheck/test/build with
`BASE_PATH=/Orchester/`; the desktop gate runs Tauri metadata and platform CI
artifact smoke. A task is not marked complete from a local screenshot alone.
