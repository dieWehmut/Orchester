# Web Change Inspector Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a protocol-faithful, safe, testable Changes inspector to the Vue WebUI.

**Architecture:** Keep run projection pure in `@orchester/ereignis`; derive a display-only change summary in WebUI; render it through a focused inspector component and a separate safe text diff component. Use named slots so the existing inspector shell and future Tauri host remain reusable.

**Tech Stack:** Vue 3, TypeScript, Vitest, Vue Test Utils, existing design components and tokens.

---

### Task 1: Define the change-summary contract

**Files:**
- Create: `apps/web/src/components/changes/change-summary.ts`
- Test: `apps/web/test/change-summary.test.ts`

- [ ] Write tests for empty input, repeated path collapse, latest-sequence ordering, and path tie-breaking.
- [ ] Run the focused test and confirm it fails because the module does not exist.
- [ ] Implement `ChangeSummary` and `summarizeFileChanges` with no Vue dependency.
- [ ] Run the focused test and confirm it passes.
- [ ] Commit: `feat(web): project run file changes for inspector`

### Task 2: Add safe diff text normalization

**Files:**
- Create: `apps/web/src/components/changes/safe-diff.ts`
- Test: `apps/web/test/safe-diff.test.ts`

- [ ] Write tests for clean text, CRLF normalization, NUL/binary refusal, line truncation, and byte truncation.
- [ ] Run the focused test and confirm it fails.
- [ ] Implement bounded `prepareDiffText` returning an explicit render status.
- [ ] Run the focused test and confirm it passes.
- [ ] Commit: `feat(web): add bounded diff text policy`

### Task 3: Render the change list

**Files:**
- Create: `apps/web/src/components/changes/ChangeInspector.vue`
- Test: `apps/web/test/change-inspector.test.ts`

- [ ] Write component tests for empty state, row labels, status text, selection emission, and button semantics.
- [ ] Run the focused test and confirm it fails.
- [ ] Implement the compact list with stable row dimensions, icons, and accessible labels using existing design tokens.
- [ ] Run the focused test and confirm it passes.
- [ ] Commit: `feat(web): render inspector change list`

### Task 4: Render bounded diff previews

**Files:**
- Create: `apps/web/src/components/changes/SafeDiffPreview.vue`
- Test: `apps/web/test/safe-diff-preview.test.ts`

- [ ] Write tests for empty, accepted, truncated, and refused text states.
- [ ] Run the focused test and confirm it fails.
- [ ] Implement plain-text rendering with `textContent` semantics and visible policy status.
- [ ] Run the focused test and confirm it passes.
- [ ] Commit: `feat(web): render safe diff preview states`

### Task 5: Wire the changes tab to the workspace

**Files:**
- Modify: `apps/web/src/components/layout/InspectorDock.vue`
- Modify: `apps/web/src/views/WorkspaceView.vue`
- Modify: `apps/web/src/locales/en.json`
- Modify: `apps/web/src/locales/zh-CN.json`
- Modify: `apps/web/src/locales/zh-TW.json`
- Modify: `apps/web/test/workspace-components.test.ts`
- Modify: `apps/web/test/workspace-view.test.ts`

- [ ] Extend tests to assert the changes slot receives projected run changes and selected path state.
- [ ] Run focused tests and confirm the new assertions fail.
- [ ] Wire `ChangeInspector` into the named slot and keep context/approvals fallback behavior unchanged.
- [ ] Add concise localized labels for file status and summary metadata.
- [ ] Run focused tests and confirm they pass.
- [ ] Commit: `feat(web): connect changes inspector to run view`

### Task 6: Verify and integrate

- [ ] Run `pnpm --dir apps --filter @orchester/web typecheck`.
- [ ] Run `pnpm --dir apps --filter @orchester/web test`.
- [ ] Run `pnpm --dir apps --filter @orchester/web build`.
- [ ] Run `git diff --check`.
- [ ] Review the final diff and branch status.
- [ ] Merge with `--no-ff` into an integration branch based on current `origin/main`.
- [ ] Push the integration result to `origin/main` without force.
- [ ] Fetch and verify the remote `main` SHA.
