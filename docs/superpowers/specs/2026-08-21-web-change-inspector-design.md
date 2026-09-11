# Web Change Inspector Design

## Goal

Turn the WebUI inspector's `Changes` tab into a useful, Codex-style workspace
change view without inventing data that the current protocol does not carry.

## Scope

- Project the run view's real `fileChanges` events into a deterministic,
  path-keyed summary while retaining the latest event sequence and history.
- Render a compact file-change list with add/update/delete status, selection,
  keyboard-safe buttons, and an empty state.
- Provide a standalone safe text diff renderer for future bounded file-preview
  responses. It must reject binary/control-heavy content and truncate large
  input before rendering.
- Wire the projection and list into `InspectorDock` and `WorkspaceView`.
- Keep approvals and context tabs unchanged.

## Non-goals

- No fabricated line counts or line-level diff when the wire event only has a
  path and change kind.
- No file read endpoint, mutable editor, or Monaco dependency in this slice.
- No client-side access to arbitrary filesystem paths.

## Architecture

`@orchester/ereignis` owns the pure `RunView` projection. A new WebUI helper
derives a display-only `ChangeSummary[]` from `RunView.fileChanges`; it does
not mutate the run store. `ChangeInspector.vue` receives that data and emits a
selected path. `InspectorDock` remains a tab shell and accepts a named changes
slot, so it can be reused by the desktop shell later. `SafeDiffPreview.vue`
is an isolated renderer with explicit limits and no HTML interpretation.

## Data rules

- Paths are displayed exactly as redacted by the protocol; no absolute-path
  reconstruction is attempted.
- Repeated events for one path collapse to the latest `kind`, while the list
  exposes the latest sequence and event count.
- The summary is ordered by latest sequence descending, then path ascending.
- Empty, whitespace-only, NUL-containing, or oversized diff text is refused or
  truncated with a visible status; text is rendered as escaped plain text.

## Verification

- Unit tests cover projection collapse/order and malformed diff handling.
- Component tests cover empty, populated, selected, and keyboard-accessible
  change rows.
- WebUI typecheck, all tests, production build, and `git diff --check` run
  before integration.
