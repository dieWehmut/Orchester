# Persistent Full-Screen Chat Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep the self-agent TTY conversation in one full-screen terminal frame with a visible transcript and fixed bottom composer.

**Architecture:** `interactive::ChatSession` owns one `TerminalSession` and `FramePresenter`. `ChatHomeView` receives transcript entries and busy state; the main async loop owns `SelfAgentHost`, updates transcript state, presents the busy frame before awaiting the model, captures the sanitized outcome into the transcript, and continues until quit. Non-TTY mode keeps its existing line loop.

**Tech Stack:** Rust 2021, crossterm 0.29, existing `FramePresenter`, Tokio async runtime, Rust integration/unit tests.

---

### Task 1: Add the transcript view model and RED renderer test

**Files:**
- Modify: `kisten/konsole/src/interactive.rs:66-86` for crate-local transcript types and view fields.
- Modify: `kisten/konsole/src/interactive.rs` test helpers and one new renderer test.

- [ ] **Step 1: Add the failing test**

Add `chat_home_renders_transcript_busy_state_and_fixed_composer` in the
existing `interactive` test module. Render an 80x12 view containing an old
user turn, a newest assistant turn, and `Creating...`; assert the plain frame
contains the newest text and `Creating...`, contains a `> next task` composer,
and has no more than 12 lines. The test must use the real frame renderer, not a
mock.

- [ ] **Step 2: Run the test and verify the expected failure**

Run:

```powershell
rustup run stable-x86_64-pc-windows-gnu cargo test --locked -p orchester-konsole chat_home_renders_transcript_busy_state_and_fixed_composer -- --exact --nocapture
```

Expected: the test fails because transcript entries and busy state are not
rendered by the current startup frame.

- [ ] **Step 3: Add only the view-model scaffolding needed to compile the test**

Define crate-local `TranscriptRole`, `TranscriptEntry`, and the additional
`transcript` and `busy` fields on `ChatHomeView`. Update existing test view
literals with `transcript: &[]` and `busy: None`; do not change rendering yet.

- [ ] **Step 4: Re-run the test and confirm it is still red for behavior**

Use the same command. Expected: compilation succeeds, then the assertion about
the transcript or busy marker fails.

- [ ] **Step 5: Commit the RED test and scaffolding**

```powershell
git add kisten/konsole/src/interactive.rs
git commit -m "test(tui): specify persistent transcript frame"
git push origin main
```

### Task 2: Implement bounded transcript rendering and key/session helpers

**Files:**
- Modify: `kisten/konsole/src/interactive.rs` transcript frame rendering,
  `ChatSession`, and key handling.
- Modify: `kisten/konsole/src/self_agent/render.rs` to expose a plain captured
  outcome string.

- [ ] **Step 1: Implement the minimal transcript frame**

Add a transcript branch to `render_chat_home_frame` that reserves header,
separator, composer, and status rows; renders only the newest transcript lines
that fit; displays `* Creating...` while busy; sanitizes control characters;
and keeps the input row within the viewport. Keep the existing empty-home
panel path unchanged.

- [ ] **Step 2: Add a plain outcome capture helper**

Implement `self_agent::render_outcome_transcript` by rendering through the
existing `render_outcome` into a `Vec<u8>`, removing only the renderer's own
DIM/RESET sequences, trimming the result, and returning UTF-8 text. Add a unit
test asserting model text and usage survive while ANSI style codes do not.

- [ ] **Step 3: Expose one persistent chat session and shared key semantics**

Add crate-local `ChatSession::enter`, `present`, and `read_key` methods. Extract
the existing key handling into `handle_chat_key`, preserving Enter submission,
slash palette selection, help, Esc, and Ctrl+C. Refactor `run_home_tui` to use
these helpers so old home behavior stays covered.

- [ ] **Step 4: Run focused renderer and renderer-module tests**

```powershell
rustup run stable-x86_64-pc-windows-gnu cargo test --locked -p orchester-konsole chat_home_renders -- --nocapture
rustup run stable-x86_64-pc-windows-gnu cargo test --locked -p orchester-konsole self_agent::render -- --nocapture
```

Expected: the transcript test and outcome-render tests pass, with no viewport
overflow or trailing frame newline failures.

- [ ] **Step 5: Commit the renderer implementation**

```powershell
git add kisten/konsole/src/interactive.rs kisten/konsole/src/self_agent/render.rs
git commit -m "feat(tui): render bounded persistent transcript"
git push origin main
```

### Task 3: Keep the async self-agent loop inside the TTY session

**Files:**
- Modify: `kisten/konsole/src/main.rs:187-255` TTY interactive path.
- Modify: `kisten/konsole/tests/cli.rs` only if a non-TTY assertion needs the
  shared behavior clarified.

- [ ] **Step 1: Add the TTY state transition test seam**

Use the real `ChatSession` view state in a pure unit test to assert that a
submitted user entry plus `busy: Some("Creating...")` renders before a model
result and that a subsequent assistant entry renders without leaving the
composer frame. Keep the existing loopback two-turn integration test as the
process-level non-TTY regression.

- [ ] **Step 2: Run the new test before the main-loop change**

Run the exact focused test command and confirm it fails only because the main
TTY state is not yet wired to update transcript entries.

- [ ] **Step 3: Replace the one-action TTY launcher with the persistent loop**

Create one `ChatSession` before the loop. On submit, clear the composer,
append the user entry, present the busy frame, await `SelfAgentHost::submit`
without dropping the session, append the captured outcome or an error entry,
clear busy, and continue. Handle `/quit`, Esc, Ctrl+C, and EOF as successful
exit. Leave nested native-agent/plugin commands through the existing path and
re-enter a fresh `ChatSession` after they return.

- [ ] **Step 4: Run the full CLI integration suite and lint**

```powershell
rustup run stable-x86_64-pc-windows-gnu cargo test --locked -p orchester-konsole --test cli
rustup run stable-x86_64-pc-windows-gnu cargo clippy --locked -p orchester-konsole --all-targets -- -D warnings
rustup run stable-x86_64-pc-windows-gnu cargo fmt --all -- --check
git diff --check
```

- [ ] **Step 5: Commit and push the TTY integration**

```powershell
git add kisten/konsole/src/main.rs kisten/konsole/tests/cli.rs
git commit -m "feat(tui): keep self-agent chat on one screen"
git push origin main
```

### Task 4: Verify the installed binary and real TTY behavior

**Files:**
- No source changes expected.

- [ ] **Step 1: Reinstall the Cargo binary**

```powershell
rustup run stable-x86_64-pc-windows-gnu cargo install --path kisten/konsole --force --locked
```

- [ ] **Step 2: Verify command resolution**

Confirm `Get-Command orchester -All` lists
`C:\Users\30119\.cargo\bin\orchester.exe` first. Do not use `npx orchester`.

- [ ] **Step 3: Run a real two-turn smoke test**

In a real PowerShell terminal, run `orchester`, submit two short prompts, and
then `/quit`. Confirm the same full-screen session shows both user prompts,
both assistant responses, a visible composer after each response, and no
PowerShell prompt between turns.

- [ ] **Step 4: Record final verification and repository state**

Run `git status --short`, `git rev-parse HEAD`, and `git rev-parse origin/main`;
both revisions must match and the worktree must be clean.
