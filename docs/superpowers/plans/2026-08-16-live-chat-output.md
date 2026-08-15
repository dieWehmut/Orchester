# Live Chat Output Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render live, inspectable model output in persistent TTY chat while accepting more input and giving slash commands visible feedback.

**Architecture:** The governed self-agent remains the only model/tool/approval owner. An optional event-aware model call has a final-response default; the Responses adapter supplies bounded SSE text deltas. The TTY owns transcript scrolling and queue status. Line mode keeps final-response semantics.

**Tech Stack:** Rust 2021, Tokio, crossterm, reqwest/futures streams, existing `LanguageModel` and `SelfAgentLoop` abstractions.

---

### Task 1: Visible TTY queue and scrollback

**Files:**
- Modify: `kisten/konsole/src/main.rs`
- Modify: `kisten/konsole/src/interactive.rs`
- Test: existing unit modules in those files

- [ ] **Step 1: Write failing tests**

Add a renderer test with input `/status` and `busy: Some("Creating ..")` that requires both palette and busy text. Add a transcript test with a long command result at two scroll offsets that requires first and last lines to be reachable. Add a state test requiring queued `Workspace(Status)` to create `Queued: /status` in the transcript.

- [ ] **Step 2: Verify RED**

Run:

```powershell
rustup run stable-x86_64-pc-windows-gnu cargo test --locked -p orchester-konsole --bin orchester busy_palette_and_transcript -- --nocapture
```

Expected: the tests fail because palette rendering replaces the busy frame, no scroll offset exists, and commands have no queued marker.

- [ ] **Step 3: Implement minimal state and renderer changes**

Add `scroll_offset` to TTY state/view. Handle PageUp/PageDown/Home/End outside palette selection. Compose palette lines, transcript lines, and the busy line in the same viewport. Append a sanitized queued marker for commands and reset scrolling when new output arrives.

- [ ] **Step 4: Verify GREEN and commit**

```powershell
rustup run stable-x86_64-pc-windows-gnu cargo test --locked -p orchester-konsole --quiet
git add kisten/konsole/src/main.rs kisten/konsole/src/interactive.rs
git commit -m "fix(tui): keep busy commands and transcript visible"
git push origin main
```

### Task 2: Retry transient provider responses

**Files:**
- Modify: `kisten/laufzeit/src/harness/provider/responses/model.rs`
- Test: `kisten/laufzeit/tests/responses_model.rs`

- [ ] **Step 1: Write failing sequence tests**

Make the fake transport return HTTP 503 then valid 200, and timeout then valid 200. Require exactly two sends and successful output. Add cancellation during retry delay requiring one send.

- [ ] **Step 2: Verify RED**

```powershell
rustup run stable-x86_64-pc-windows-gnu cargo test --locked -p orchester-laufzeit --test responses_model -- --nocapture
```

Expected: 503 and timeout return `ModelError::Transport` after one send.

- [ ] **Step 3: Implement bounded retry classification**

Retry only `Transport`, `Timeout`, and HTTP 408, 425, and 500..=599. Keep authentication, forbidden, rate-limit, protocol, and cancellation non-retryable. Recheck cancellation before each retry send.

- [ ] **Step 4: Verify GREEN and commit**

```powershell
rustup run stable-x86_64-pc-windows-gnu cargo test --locked -p orchester-laufzeit --test responses_model
rustup run stable-x86_64-pc-windows-gnu cargo clippy --locked -p orchester-laufzeit --all-targets -- -D warnings
git add kisten/laufzeit/src/harness/provider/responses/model.rs kisten/laufzeit/tests/responses_model.rs
git commit -m "fix(model): retry transient provider responses"
git push origin main
```

### Task 3: Model text event contract and Responses SSE adapter

**Files:**
- Modify: `kisten/modell/src/types.rs`, `kisten/modell/src/scripted.rs`
- Modify: `kisten/laufzeit/src/harness/provider/http/{mod.rs,client.rs}`
- Modify: `kisten/laufzeit/src/harness/provider/responses/{request.rs,response.rs,model.rs}`
- Test: model and Responses provider tests

- [ ] **Step 1: Write failing event tests**

Define a text-delta sink. Require the scripted default to emit final assistant text once. Add loopback SSE data with two `response.output_text.delta` records then `response.completed`; require ordered deltas and a bounded final response.

- [ ] **Step 2: Verify RED, then implement**

Keep `complete` compatible. Add event-aware default behavior that invokes `complete` and emits final text. Override the Responses path to request SSE, validate bounded events, handle cancellation, and leave the existing non-streaming path unchanged.

- [ ] **Step 3: Verify and commit**

```powershell
rustup run stable-x86_64-pc-windows-gnu cargo test --locked -p orchester-modell
rustup run stable-x86_64-pc-windows-gnu cargo test --locked -p orchester-laufzeit --test responses_model --test model_http_reqwest
git add kisten/modell kisten/laufzeit/src/harness/provider
git commit -m "feat(model): expose bounded response text events"
git push origin main
```

### Task 4: Governed runtime forwarding and live TTY deltas

**Files:**
- Modify: `kisten/laufzeit/src/harness/agent_loop.rs` and service runtime entry points
- Modify: `kisten/konsole/src/self_agent.rs`, `kisten/konsole/src/main.rs`
- Test: runtime event tests and CLI tests

- [ ] **Step 1: Write failing forwarding test**

Use a model that emits two text deltas and one final response. Run the governed loop and require ordered deltas without changed tool-call or approval behavior.

- [ ] **Step 2: Implement and render**

Thread an optional sender through model calls. Add a TTY-only submit method. Create one provisional assistant transcript entry, append sanitized deltas while key polling and busy animation continue, then replace it with final rendered outcome data.

- [ ] **Step 3: Verify and commit**

```powershell
rustup run stable-x86_64-pc-windows-gnu cargo test --locked -p orchester-laufzeit --lib
rustup run stable-x86_64-pc-windows-gnu cargo test --locked -p orchester-konsole --quiet
rustup run stable-x86_64-pc-windows-gnu cargo clippy --locked -p orchester-konsole -p orchester-laufzeit --all-targets -- -D warnings
git add kisten/laufzeit/src/harness kisten/konsole/src
git commit -m "feat(tui): render governed model deltas live"
git push origin main
```

### Task 5: Install and verify the artifact

**Files:** none expected.

- [ ] **Step 1: Run final checks**

```powershell
rustup run stable-x86_64-pc-windows-gnu cargo fmt --all -- --check
git diff --check
rustup run stable-x86_64-pc-windows-gnu cargo test --locked -p orchester-konsole --quiet
rustup run stable-x86_64-pc-windows-gnu cargo test --locked -p orchester-laufzeit --test responses_model
```

- [ ] **Step 2: Reinstall and inspect**

```powershell
rustup run stable-x86_64-pc-windows-gnu cargo install --path kisten/konsole --locked --force
Get-Command orchester
git status --short --branch
git rev-parse HEAD
git rev-parse origin/main
```

Run each requested slash command followed by `/quit`; require visible output with no credential-like text. A real 2xx provider response remains required before claiming live model text; provider 401/403 is an external account or configuration block.
