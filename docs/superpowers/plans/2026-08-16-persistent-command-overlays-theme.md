# Persistent Command Overlays and Theme Picker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep the initial Orchester panel in every fullscreen frame, make slash commands stable in-session pickers, add `/theme`, and publish the verified result as `v0.1.1`.

**Architecture:** Retain one `ChatSession` and one `FramePresenter`. Add an optional overlay model to `TerminalChatState`; render the startup panel, transcript, overlay, composer, and footer as one bounded frame. Commands continue using existing governed service projections, but their output is captured into overlay rows or transcript entries instead of stdout. A small palette enum drives theme preview and persistence.

**Tech Stack:** Rust 2021, crossterm, Tokio, existing Orchester config/service projections, npm release workflow.

---

### Task 1: Lock command and overlay behavior with RED tests

**Files:**
- Modify: `kisten/konsole/src/interactive/commands.rs`
- Modify: `kisten/konsole/src/interactive.rs`
- Modify: `kisten/konsole/src/main.rs`

- [ ] Add tests for `/theme` parsing and for each requested command producing a non-empty overlay action.
- [ ] Add a renderer test that renders a non-empty transcript plus an overlay and asserts the startup panel marker and composer both remain present.
- [ ] Add a state test for Up/Down/Enter/Esc overlay transitions.
- [ ] Run the focused tests and record the expected failures before implementation.
- [ ] Commit the RED tests as `test(tui): specify persistent command overlays`.

### Task 2: Keep the startup panel and command overlays in one frame

**Files:**
- Modify: `kisten/konsole/src/interactive.rs`
- Modify: `kisten/konsole/src/main.rs`

- [ ] Add bounded overlay types and keyboard routing to `TerminalChatState`.
- [ ] Render the initial panel/header on every frame, then render overlay rows, transcript tail, composer, and footer without leaving the alternate screen.
- [ ] Replace `drop(chat)`/`ChatSession::enter()` around read-only workspace and plugin commands with in-memory capture and overlay/transcript updates.
- [ ] Verify focused konsole tests and commit `fix(tui): keep command overlays in the active screen`.

### Task 3: Add `/theme` palette, preview, and persistence

**Files:**
- Create: `kisten/konsole/src/theme.rs`
- Modify: `kisten/konsole/src/interactive.rs`
- Modify: `kisten/konsole/src/main.rs`
- Modify: `kisten/laufzeit/src/harness/config.rs` (only if the existing TUI config writer is needed)

- [ ] Add RED tests for built-in themes, unknown-name fallback, live preview, and cancel restoration.
- [ ] Implement the palette and a bounded diff-style preview in the overlay.
- [ ] Persist the confirmed name in the user TUI config using the existing private-file boundary; never write secrets or project config.
- [ ] Verify theme tests and commit `feat(tui): add persistent theme picker`.

### Task 4: Stabilize all requested command views

**Files:**
- Modify: `kisten/konsole/src/self_agent/*.rs`
- Modify: `kisten/konsole/src/main.rs`
- Test: `kisten/konsole/tests/cli.rs` and unit modules

- [ ] Convert model, permissions, resume, status, config, and plugin projections to bounded overlay rows with selected/current markers.
- [ ] Ensure Enter confirms a row and records a visible result; Esc returns to the unchanged chat frame.
- [ ] Add line-mode compatibility so the same commands still work when stdin is not a TTY.
- [ ] Verify the complete konsole suite and commit `fix(tui): make slash command selections actionable`.

### Task 5: Bump and publish v0.1.1

**Files:**
- Modify: `Cargo.toml`
- Modify: `npm/cli/package.json`
- Modify: `npm/cli/package-lock.json` (if present)
- Modify: platform/plugin package manifests pinned to `0.1.1`
- Modify: release metadata only where required by the existing workflow

- [ ] Add a release-version test/check proving every Rust and npm manifest agrees on `0.1.1`.
- [ ] Run format, diff, focused tests, package build, and clippy with the x64 GNU Rust toolchain.
- [ ] Commit `chore(release): prepare v0.1.1`, push `main`, and dispatch the existing `npm-release` workflow with `version=0.1.1`.
- [ ] Poll the workflow through the GitHub API until stage, publish, tag, and release jobs have explicit conclusions.
- [ ] Install the resulting CLI locally, run the fullscreen smoke path, and commit/push no further changes after the release tag.
