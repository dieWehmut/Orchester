# Agent Process Presence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Report whether registered agent executables are currently running and expose a redaction-safe aggregate instance count to the local WebUI.

**Architecture:** Extend the shared Rust/TypeScript agent-status contract with an explicit `external_processes` count source. Add a process discovery module in `orchester-netz` that converts process names into provider counts without retaining PIDs, arguments, titles, or paths. Refresh the shared runtime store at request time first, then add a lifecycle-bound polling task for WebSocket updates.

**Tech Stack:** Rust 2021, sysinfo, Axum, Tokio, serde, TypeScript, Vitest, pnpm.

---

### Task 1: External process count contract

**Files:**
- Modify: `kisten/protokoll/src/agent_status.rs`
- Modify: `kisten/protokoll/tests/agent_status.rs`
- Modify: `apps/protokoll/src/agent-status.ts`
- Modify: `apps/protokoll/test/agent-status.test.ts`

- [ ] Add failing Rust and TypeScript tests accepting `external_processes` and rejecting unknown count sources.
- [ ] Run the focused tests and confirm the new source is rejected.
- [ ] Add the enum/union member and increment the agent-status schema version.
- [ ] Run focused Rust and TypeScript contract tests.
- [ ] Commit as `feat(protocol): model external agent processes`.

### Task 2: Pure provider process matching

**Files:**
- Create: `kisten/netz/src/agent_process.rs`
- Create: `kisten/netz/tests/agent_process.rs`
- Modify: `kisten/netz/src/lib.rs`

- [ ] Add failing tests for exact executable-name matching, helper exclusion, case normalization, and unknown process rejection.
- [ ] Run `cargo test -p orchester-netz --test agent_process` and confirm missing API failure.
- [ ] Implement provider rules and a pure aggregate snapshot containing only provider ids and counts.
- [ ] Run the focused test.
- [ ] Commit as `feat(netz): classify external agent processes`.

### Task 3: Operating-system process discovery

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `kisten/netz/Cargo.toml`
- Modify: `kisten/netz/src/agent_process.rs`
- Modify: `kisten/netz/tests/agent_process.rs`

- [ ] Add a failing test around an injected raw process-name source.
- [ ] Add the Rust-1.80-compatible `sysinfo` dependency with minimum features.
- [ ] Implement `SystemAgentProcessSource` so raw process metadata is discarded immediately after aggregation.
- [ ] Run focused tests and `cargo check -p orchester-netz`.
- [ ] Commit as `feat(netz): discover running agent executables`.

### Task 4: Runtime snapshot refresh

**Files:**
- Modify: `kisten/netz/src/agent_status.rs`
- Modify: `kisten/netz/src/bootstrap.rs`
- Modify: `kisten/netz/tests/agent_runtime_status.rs`
- Modify: `kisten/netz/tests/agent_status_routes.rs`

- [ ] Add failing tests proving an external count changes activity to `running`, sets `active_windows`, and preserves managed runtime fields.
- [ ] Implement store reconciliation with monotonic sequence updates and no broadcast for unchanged counts.
- [ ] Inject a process source into test contexts and refresh before HTTP snapshots.
- [ ] Run focused runtime and route tests.
- [ ] Commit as `feat(netz): reconcile process presence into agent status`.

### Task 5: Lifecycle-bound WebSocket polling

**Files:**
- Modify: `kisten/netz/src/agent_status.rs`
- Modify: `kisten/netz/src/bootstrap.rs`
- Modify: `kisten/netz/tests/agent_status_socket.rs`
- Create: `kisten/netz/tests/agent_process_monitor.rs`

- [ ] Add a failing paused-time test for periodic refresh, changed-count broadcast, and shutdown.
- [ ] Implement one cancellable monitor task per production `ServerContext`.
- [ ] Ensure test routers can opt out or inject deterministic sources.
- [ ] Run socket and monitor tests.
- [ ] Commit as `feat(netz): stream external agent presence changes`.

### Task 6: Cross-language and UI verification

**Files:**
- Modify: `apps/protokoll/src/fixtures/agents.ts`
- Modify: WebUI tests only where schema fixtures require version/source updates.

- [ ] Update deterministic fixtures to the new schema while retaining managed-session examples.
- [ ] Add an external-process fixture demonstrating Codex running with multiple instances.
- [ ] Run Rust protocol/netz tests, TypeScript protocol tests, WebUI tests/typecheck/build, and `git diff --check`.
- [ ] Commit as `test(agent-status): cover external process presence`.

