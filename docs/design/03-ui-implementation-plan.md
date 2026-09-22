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

- [x] U6-01: Add the macOS overlay titlebar path with correct traffic-light inset.
- [x] U6-02: Add Windows caption buttons at OS metrics with the Snap-Layouts hit region.
- [x] U6-03: Fall back to opaque surfaces where translucency is unavailable.
- [x] U6-04: Wire `⌘/Ctrl+W` to close the active tab and confirm before closing with an active run.

## Wave U7 — settings, keyboard, a11y

- [x] U7-01: Add settings search across all panels.
- [x] U7-02: Add the keyboard-shortcut editor with keypress search and reset-all.
- [x] U7-03: Add a shortcut registry that every component registers into, so the editor cannot drift from reality.
- [x] U7-04: Audit and fix the accessibility contract in §7 of the design spec.
- [x] U7-05: Add contrast assertions for every text/surface and border/surface pair.

## Wave U8 — migration and cleanup

- [x] U8-01: Re-point the legacy `--color-bg-*`, `--color-border-*`, `--color-text-*` tokens at L1 and update the snapshot.
  - `packages/design/src/tokens.css` carries the legacy names as `var(--color-surface-*)`
    and `var(--color-text-*)` aliases, and the theme snapshot holds the mapping.
- [x] U8-02: Remove the two obsolete `amber` snapshots.
  - The accent schemes are `codex`, `violet`, `teal` and `rose`; the snapshot's
    `dark-amber` and `light-amber` entries are gone.
- [x] U8-03: Sweep components for L0/L1 leakage and bare `px` spacing.
  - `packages/design/test/component-migration.test.ts` holds the sweep as a
    standing rule over both component layers.
- [x] U8-04: Replace remaining hard-coded English strings with locale keys.
  - `apps/web/test/locale-sweep.test.ts` holds the sweep as a standing rule, and
    the shortcut registry and the composer's command list carry keys rather than
    words so the surface resolves them.
- [x] U8-05: Re-run the full gate: `pnpm typecheck`, `pnpm test`, both builds.
  - `pnpm typecheck` is clean across all seven projects; `pnpm test` is green at
    25 tooling + 89 protokoll + 264 design + 31 website + 20 ereignis + 476 web
    + 1 desktop-security tests; the web and website builds both succeed.

## Wave U9 - reference alignment against the Codex desktop app

The three reference screenshots (the sidebar, its account menu, and the
appearance screen) were compared against the shipped surfaces. What the
reference has and Orchester did not is additive, so this wave follows the same
rules as U0-U8 and leaves the token and DTO contracts alone.

- [x] U9-01: Give the rail the reference's anatomy: a product row that owns its
  disclosure, a heading that folds each list, and an account row that opens a
  menu instead of sitting beside a gear.
  - `apps/web/test/app-rail.test.ts` pins the row order, the disclosure, and the
    account menu's intents.
- [x] U9-02: Make the product row's disclosure fold the lists it governs, rather
  than leaving a chevron that draws and does nothing.
  - The fold is a column layout rather than fixed grid tracks, so removing the
    lists cannot slide the account row into a list's track.
- [x] U9-03: Head the settings screen with the way back and print each resolved
  colour's hex beside its swatch.
  - `apps/web/test/theme-colours.test.ts` checks the mirrored values against
    `tokens.css`, so the readout cannot drift from the stylesheet.
- [x] U9-04: Teach each account-menu row the chord that performs it, read from
  the live shortcut registry rather than written out.
  - The registry is what the settings editor rebinds, so a printed default would
    keep teaching a key that no longer does anything.
- [x] U9-05: Carry the reference's header actions: a search glyph that narrows
  the column, and a bell that reports how much is waiting and opens it.
  - `apps/web/test/session-rail-filter.test.ts` pins the filter, including the
    difference between "no sessions" and "nothing matched".
- [x] U9-06: Answer the companion chord the reference teaches on its menu row.
  - The reference spells it `Alt+Win+P`; `Win` is the OS chord and the registry
    refuses the foreign modifier, so the shell binds `Ctrl+Alt+P` - the same
    gesture one key over - and the row prints it from the live registry.
- [x] U9-07: Re-run the full gate: `pnpm typecheck`, `pnpm test`, both builds,
  the stack manifest, and the desktop shell's `cargo check`.
  - `pnpm typecheck` is clean; `pnpm test` is green at 543 web + 264 design
    + 31 website + 27 ereignis + protokoll + tooling tests; the web and website
    builds succeed; `pnpm stack:verify` matches; the shell checks clean on the
    x64 toolchain this ARM64 host uses.

## Wave U10 - the appearance screen the reference draws

The reference's appearance screen gives each theme its own card rather than one
accent for the product, so this wave is the first that changes an appearance
*model* rather than only a surface. It stays additive: `data-color-scheme` is
still the attribute the stylesheet matches on and still carries one scheme.

- [x] U10-01: Let each theme carry its own accent hue.
  - `packages/design/test/appearance-scheme-per-mode.test.ts` pins the pair, the
    attribute following the theme in force, the legacy single key still reading
    as "both themes", and the bootstrap script painting the half belonging to
    the theme it resolved.
- [x] U10-02: Draw one card per theme, headed 浅色主题 / 深色主题, with the
  reference's own actions (导入, 复制主题), the `Aa` swatch, the theme's choice,
  and 强调色 / 背景 / 前景 reporting what *that* theme resolves to.
  - `apps/web/test/appearance-theme-sections.test.ts` pins the cards, the
    per-theme readouts, and the copy payload; `theme-colours.test.ts` walks
    both halves of every scheme out of `tokens.css`, because one scheme is two
    colours and the old single-row swatch had drifted from the stylesheet.
- [x] U10-03: Mark the chosen theme card with the ring alone.
  - The badge was a second answer to the question the ring had already asked,
    and it covered the miniature it sat on. The ring is a box shadow rather than
    an outline: `outline` belongs to the focus token.
- [x] U10-04: Bare the code preview and give each pane the rail the reference
  draws, in the colour of the change that pane carries.
  - `CodePreview`'s head is now optional, so the settings screen shows the two
    panes directly under the cards as the reference does.
- [x] U10-05: Re-run the gate: `pnpm typecheck`, `pnpm test`, both builds, and
  the stack manifest.
  - `pnpm typecheck` is clean across all seven projects; `pnpm test` is green at
    274 design + 549 web + the rest; the web build and `pnpm stack:verify` pass.

**Not taken from the reference.** The reference's account menu carries usage,
invites and sign-out, and its appearance screen offers a hex picker per colour.
Orchester has no account to sign out of, no usage meter to report, and a hex
picker beside a theme choice would be a second theme editor - so those rows are
left out rather than faked.

## Wave U11 - the rail, as the reference seats it

The reference's sidebar is one column of destinations: the product row, one new
chat row, then headed lists whose headings are the controls that fold them. It
offers each action once and each list one name. This wave removes the places
where Orchester offered two.

- [x] U11-01: Seat new chat as a rail row rather than a boxed block.
  - `apps/web/test/app-rail.test.ts` pins the row marker and that the primary
    section carries no `AppButton`: the row above a list of rows is a row.
- [x] U11-02: End the account row at the identity, with settings in the menu it
  opens rather than a gear beside it.
  - `app-rail.test.ts` and `workspace-rail.test.ts` now reach the settings route
    through the account menu, which also carries the chord.
- [x] U11-03: Let the rail heading name each list, once.
  - `apps/web/test/session-rail-anatomy.test.ts` pins that the sessions list
    drops its own uppercase title and its second new-session button while
    keeping an accessible name; `agent-fleet-panel.test.ts` pins the same for
    the fleet, which keeps its stream badge and count because those belong to
    the list rather than to the column.
- [x] U11-04: Re-run the gate: `pnpm typecheck`, `pnpm test`, both builds,
  `pnpm stack:verify`.

**Still not taken from the reference.** The reference's account menu also
carries remaining usage, invites and sign-out. Orchester runs locally with no
account, no meter and nothing to sign out of, so those rows stay out rather than
being faked; the menu carries what it can answer - the companion and settings,
both with the chord the live registry holds.

## Wave U12 - the companion, let off its seat

The companion arrived seated: one sprite cell above the composer, holding
whatever pose the run asked of it. The reference's companion is ambient instead
- it pads along the strip its corner opens for it and turns to face the way it
is going - so this wave gives it somewhere to walk and a reason to.

- [x] U12-01: Plan a walk rather than play one.
  - `apps/web/test/pet-roam.test.ts` pins the leg: a whole number of walk
    cycles, the direction with room rather than a coin flip into a wall, and a
    rest drawn from an authored window.
- [x] U12-02: Let the companion pace the stage it was given.
  - `pet-companion.test.ts` pins the walk, the row each direction draws, the
    glide timed to the leg, and that an interrupted leg leaves the companion
    standing where it had got to rather than at either end.
- [x] U12-03: Open a stage above the composer for it to walk on.
  - The band was a seat; it is a stage now, and its own width is the travel it
    reports. The span leaves the sprite its own width behind, because the
    sprite moves by transform.
- [x] U12-04: Notice the pointer that is addressing the companion, and only
    that one.
  - `pet-roam.test.ts` pins the radius as a circle. This is the correction a
    real window forced: a companion that took a look from any distance spent
    its life attending to a reader working in the transcript and never walked.
- [x] U12-05: Re-run the gate: `pnpm typecheck`, `pnpm test`, both builds,
  `pnpm stack:verify`.
  - `pnpm typecheck` is clean across all seven projects; `pnpm test` is green at
    579 web + 274 design + the rest; the web and website builds succeed;
    `pnpm stack:verify` matches.

**Deliberately not done.** The companion keeps no menu of its own and remembers
nothing about where it likes to stand: the visibility switch is still the only
preference, because a companion that had to be configured would not be ambient.
The walk is also CSS rather than canvas - one transform and one looping sprite
row - so there is no per-frame draw to spend on a decoration.

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
