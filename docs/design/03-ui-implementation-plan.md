# Orchester UI implementation plan

> Execution index for the Codex-informed UI design in
> [`02-orchester-ui-design.md`](./02-orchester-ui-design.md).
>
> Rules: every task starts with a failing test, ends green, is committed on its
> own, and leaves the workspace typechecking. Tasks are ordered so that each one
> is independently revertable.

**Invariant:** no task may break the existing public token names, component
exports, DTOs, or HTTP contracts. All new work is additive until the migration
wave, which lands with an updated snapshot.

## Wave U0 — token foundation

- [x] U0-01: Add the neutral ramp (`--gray-fixed-*`, `--gray-*`) to `packages/design/src/tokens.css` and assert every step exists in both themes.
- [x] U0-02: Add the six accent ramps and their alpha variants; assert the `codex` scheme resolves to `--blue-300` in dark and `--blue-500` in light.
- [x] U0-03: Add the `--alpha-*` scale over `--alpha-base`; assert the base flips with the theme.
- [x] U0-04: Add the L1 intent matrix (`--color-intent-*-{text,surface,border,solid}`, `--color-text-*`, `--color-surface-*`, `--color-border-*`); assert every intent has all four treatments in both themes.
- [x] U0-05: Add the heading and text type scales with bound line-height/tracking/weight; assert monotonicity of size across each scale.
- [x] U0-06: Add `--radius-*-base` and `--corner-radius-scale`, deriving every `--radius-*`; assert `1.25` is the default and that `--radius-lg` resolves to `0.78125rem`.
- [x] U0-07: Add `--border-width-hairline` and the elevation geometry/shadow tokens; assert `--elevation-stroke` uses the hairline.
- [x] U0-08: Add the motion curves and `--ease-*`; assert the reduced-motion override still collapses durations.
- [x] U0-09: Add the shell layout tokens (rail clamp, inspector clamp, tab strip, bottom panel, composer) and assert the rail clamp never leaves less than 360px for the transcript.
- [x] U0-10: Add the density axis (`--density-row-height`, `--density-gap`) behind `[data-density]`.
- [x] U0-11: Export the new token names from a typed `TOKEN_GROUPS` map for tests and tooling.
- [x] U0-12: Update the token snapshot and commit wave U0.

## Wave U1 — theme and appearance plumbing

- [x] U1-01: Add `data-intensity` (`calm` | `vivid`) as a third appearance axis in `theme.ts`, the bootstrap script, and storage.
- [x] U1-02: Add `data-reduced-motion` as an explicit, overridable root attribute with `true`/`false`/system semantics.
- [x] U1-03: Add `data-orchester-surface` and `data-orchester-os` root attributes with detection helpers.
- [x] U1-04: Extend the no-flash bootstrap script to set all four axes before first paint, and assert it stays dependency-free.
- [x] U1-05: Extend `useAppearance` and the settings view to expose intensity and reduced motion.

## Wave U2 — shell restructure

- [x] U2-01: Introduce `AppShell` owning the region state attributes; port the current three-column grid onto it without visual change.
- [x] U2-02: Replace the three ad-hoc width variables with the rail/inspector clamps; add drag-to-resize with keyboard support and persistence.
- [x] U2-03: Add the collapsible bottom panel with its own tab mechanism.
- [x] U2-04: Add the unified tab strip with open/close/reorder/keyboard cycling and overflow fades.
- [x] U2-05: Persist open tabs and panel widths through the existing settings/state path.

## Wave U3 — transcript

- [x] U3-01: Add explicit scroll state (`data-can-scroll-up/down`) and the top fade.
- [x] U3-02: Add the scroll-to-bottom button with its unread indicator.
- [x] U3-03: Virtualise turns.
- [x] U3-04: Add the floating message navigation rail with scrubbing.
- [x] U3-05: Restyle tool-invocation cards by `call_id`, with collapsed summary and expandable body.
- [x] U3-06: Add the reasoning disclosure.
- [x] U3-07: Add word-arrival streaming that does not reflow ancestors.

## Wave U4 — composer and plan strip

- [x] U4-01: Restructure the composer into top tray / input / footer with `data-composer-state`.
- [x] U4-02: Add the approval-preset footer dropdown with the three presets and the risky-combination dialog.
- [x] U4-03: Make model/effort/preset run-scoped rather than global.
- [x] U4-04: Add drag-and-drop file affordance (`data-composer-drag-active`).
- [x] U4-05: Add the `/` command palette with explicit empty and loading states.
- [x] U4-06: Add the plan strip with segmented progress and the blocked/needs-input treatment.

## Wave U5 — inspector

- [x] U5-01: Add the `Review` tab with change filters (all/staged/unstaged/branch/last turn).
- [x] U5-02: Order the file tree identically to the diff list and add expand/collapse navigation.
- [x] U5-03: Add the line-wrap toggle for diffs.
- [x] U5-04: Add the `Approvals` queue with risk summary, scoped choices and the superseded state.
  - `Allow for run` renders disabled: the protocol carries `approved`/`denied`
    only, and a run-scoped grant needs a scope the runtime does not have yet.
- [x] U5-05: Add terminal placement preference (inspector vs bottom panel).

## Wave U6 — desktop chrome

- [ ] U6-01: Add the macOS overlay titlebar path with correct traffic-light inset.
- [ ] U6-02: Add Windows caption buttons at OS metrics with the Snap-Layouts hit region.
- [ ] U6-03: Fall back to opaque surfaces where translucency is unavailable.
- [ ] U6-04: Wire `⌘/Ctrl+W` to close the active tab and confirm before closing with an active run.

## Wave U7 — settings, keyboard, a11y

- [x] U7-01: Add settings search across all panels.
- [x] U7-02: Add the keyboard-shortcut editor with keypress search and reset-all.
- [x] U7-03: Add a shortcut registry that every component registers into, so the editor cannot drift from reality.
- [x] U7-04: Audit and fix the accessibility contract in §7 of the design spec.
- [x] U7-05: Add contrast assertions for every text/surface and border/surface pair.

## Wave U8 — migration and cleanup

- [ ] U8-01: Re-point the legacy `--color-bg-*`, `--color-border-*`, `--color-text-*` tokens at L1 and update the snapshot.
- [ ] U8-02: Remove the two obsolete `amber` snapshots.
- [ ] U8-03: Sweep components for L0/L1 leakage and bare `px` spacing.
- [ ] U8-04: Replace remaining hard-coded English strings with locale keys.
- [ ] U8-05: Re-run the full gate: `pnpm typecheck`, `pnpm test`, both builds.

## Verification

```text
pnpm --filter @orchester/design test
pnpm --filter @orchester/web test
pnpm typecheck
pnpm --filter @orchester/web build
pnpm --filter @orchester/website build
```

Each wave ends with the focused package test for the packages it touched, plus a
`git diff --check`.
