# Orchester UI design specification

> The design for Orchester's three faces — the local WebUI, the GitHub Pages
> site, and the Tauri desktop app — derived from the Codex desktop app research in
> [`01-codex-desktop-app-research.md`](./01-codex-desktop-app-research.md) and
> bound to what Orchester's runtime can actually serve.
>
> This document is normative. Where it says "must", a test should be able to
> assert it.

## 0. Why this design and not a reskin

Orchester is not a clone of Codex and must not become one. Codex is a hosted
assistant whose primary act is *send a message*; Orchester is a **self-owned
agent runtime** whose primary act is *authorise and run a governed task*. That
difference has to show up in the interface, or the product has no reason to exist.

The design therefore takes Codex's **anatomy, token layering, density and
restraint** — the parts that are craft — and keeps Orchester's own voice in three
places:

1. **Governance is always visible.** Policy, approval mode, validator state and
   audit are first-class chrome, not dialogs that appear only when they bite.
2. **The accent stays the primary action colour.** Codex renders its primary
   button achromatic and inverting; Orchester keeps an accent "Run" because the
   action is consequential and should be findable.
3. **Delegation is explicit.** External agents (`claude`, `codex`, `opencode`)
   are a visible fleet with their own rail, not an invisible implementation
   detail.

## 1. Principles

| # | Principle | Testable consequence |
|---|---|---|
| P1 | **One shell, three faces.** The WebUI, the Pages demo and the desktop window are the same shell; only root attributes and injected capabilities differ. | The shell renders from a single component tree; `apps/web/site` imports the same design package, and the shell branches on `data-orchester-surface`, never on a build flag. |
| P2 | **Panels, not pages.** Nothing that belongs beside the transcript gets its own route. | Only `/`, `/settings` and `*` are routes; the right panel and bottom panel are state, not navigation. |
| P3 | **The transcript is the product.** Every other region can be collapsed; the transcript cannot. | The transcript pane is never `display:none` at any breakpoint. |
| P4 | **Calm surfaces, loud states.** Surfaces differ by ≤ 2 neutral steps and are separated by half-pixel hairlines; risk, approval and failure states get saturated colour. | Surfaces use the neutral ramp with a max delta of two steps; only status intents may use `--color-background-*-soft`. |
| P5 | **Motion is a whisper.** Transitions are ≤ 240 ms and may not move layout. | Under `data-reduced-motion="true"` every duration token resolves ≤ 1 ms. |
| P6 | **Every state is designed.** Empty, loading, streaming, needs-input, failed, cancelled, resync-required. | Each surface documents and renders all six; a component with only a happy path fails review. |
| P7 | **Keyboard first.** Everything reachable by pointer is reachable by keyboard, and the shortcut is discoverable. | Every panel toggle, tab, task row and composer control has a roving-tabindex or `aria-keyshortcuts` contract. |
| P8 | **The runtime is the source of truth.** No view invents state the runtime cannot persist. | Every rendered field traces to a DTO or projection in `packages/protokoll` / `packages/ereignis`. |

## 2. Shell anatomy

```
┌──────────────────────────────────────────────────────────────────────────┐
│ A  app chrome        (macOS/Windows custom titlebar, or a plain app bar) │
├──────────────────────────────────────────────────────────────────────────┤
│ B  unified tab strip (open tasks · terminals · diffs · agents)           │
├───────────┬──────────────────────────────────────────┬───────────────────┤
│ C         │ D  thread header                         │ E                 │
│ task rail │    title · branch · status · actions     │ inspector panel   │
│           ├──────────────────────────────────────────┤                   │
│ projects  │ F  transcript                            │ Context           │
│ tasks     │    virtualised turns                     │ Review            │
│ agents    │    floating message navigation rail      │ Approvals         │
│           │    top fade · scroll-to-bottom           │ Agents            │
│           ├──────────────────────────────────────────┤ Terminal          │
│           │ G  plan strip (todos · validators)       │                   │
│           ├──────────────────────────────────────────┤                   │
│           │ H  composer                               │                   │
├───────────┴──────────────────────────────────────────┴───────────────────┤
│ I  bottom panel (terminal · exec output · audit log) — collapsible       │
└──────────────────────────────────────────────────────────────────────────┘
```

Region state attributes (mirroring Codex's shell contract, §4 of the research):

| region | attributes |
|---|---|
| A | `data-orchester-surface="web\|site\|desktop"`, `data-orchester-os`, `data-orchester-window-chrome` |
| B | `data-tabstrip`, `data-tabstrip-overflow` |
| C | `data-rail`, `data-rail-appearance="solid\|translucent"`, `data-rail-responsive` |
| D | `data-thread-header`, `data-header-edge-scroll` |
| E | *(removed)* - the right column is gone; region I carries its surfaces |
| F | `data-transcript`, `data-can-scroll-up`, `data-can-scroll-down`, `data-virtualized-turn` |
| G | `data-plan-strip`, `data-plan-state="idle\|active\|blocked\|done"` |
| H | `data-composer`, `data-composer-state`, `data-composer-drag-active` |
| I | `data-bottom-panel`, `data-bottom-panel-state="collapsed\|expanded"` |

Global root attributes: `data-theme`, `data-color-scheme`, `data-reduced-motion`,
`data-density="comfortable|compact"`, plus the surface attributes above.

### 2.1 Widths and collapse policy

Adopted from the Codex sidebar rule and restated as Orchester tokens:

| region | rule |
|---|---|
| rail | `clamp(240px, 288px, min(420px, 100vw − 360px))` — the max is derived so the transcript always keeps 360 px |
| transcript | `minmax(0, 1fr)`, `--content-max: 1180px` measure cap for prose |
| bottom panel | `--bottom-panel-height: 240px`, drag-resizable `160px`–`70vh`; it carries the run's surfaces as well as the terminal, exec output and audit log |
| tab strip | `--tabstrip-height: 36px` |

Collapse policy:

- `≥ 1280px` — the rail and the transcript, plus the tab strip.
- `900–1279px` — the rail becomes an overlay drawer; the tab strip stays.
- `< 900px` — single column; the rail is a full-height drawer; the
  composer is sticky to the visual viewport bottom.
- The bottom panel is a drawer under 900 px.

Widths are **persisted per user** and the rail's collapsed state is a single
boolean the user can toggle from anywhere (`⌘/Ctrl+B`).

## 3. Token architecture

The research shows Codex's token system is valuable mainly because of its
**layering**. Orchester adopts the same four layers inside `packages/design`,
replacing the flat `--color-*` set while keeping the existing names working.

```
L0  raw           --gray-fixed-*, --gray-*, --blue-*, --green-*, --orange-*,
                  --red-*, --purple-*, --yellow-*, --alpha-*
L1  intent        --color-text-{emphasis,default,subtle,disabled},
                  --color-surface-{base,secondary,tertiary,elevated},
                  --color-border-{subtle,default,emphasis,focus},
                  --color-intent-{info,success,warning,caution,danger,discovery}-{text,surface,border,solid}
L2  app           --app-color-* : the semantic vocabulary the shell consumes
L3  component     --rail-*, --tabstrip-*, --composer-*, --transcript-*,
                  --inspector-*, --panel-*
```

The L1 names are deliberately **not** `--color-text-primary` / `--color-border-base`:
those names already mean something in Orchester, and the whole point of the
migration in §3.2 is that the legacy name keeps working until it is re-pointed.
Two names for one role during the transition is honest; silently redefining a
name that 280 tests and 60 components depend on is not.

Rules:

- **L3 may only reference L2.** A component that reaches into L0/L1 fails review,
  because it can no longer be re-themed as a unit.
- **Status colour is only allowed at L1's intent layer.** No component declares a
  raw red or green.
- **Every margin/padding/gap is a `--space-*` multiple.** No bare `px` in a
  component's `padding`/`margin`/`gap`.

### 3.1 New tokens (wave 1, additive)

Added without changing any existing token, so the current 280 tests stay green:

**Neutral ramp** — `--gray-fixed-{0,25,50,75,100,150,200,250,300,350,400,450,500,550,600,650,700,750,800,850,900,925,950,975,1000}`
and the theme-flipping aliases `--gray-{0…1000}`.

**Accent ramps** — `--blue-*`, `--green-*`, `--orange-*`, `--red-*`, `--purple-*`,
`--yellow-*` at steps `25,50,75,100,200,300,400,500,600,700,800,900,1000`, plus
`--<hue>-a{25,50,75,100,200,300}` alpha variants.

**Alpha scale** — `--alpha-base` plus `--alpha-{0,01,02,04,05,06,08,10,12,15,16,20,25,30,35,40,50,60,70}`.

**Intent matrix** — the `--color-intent-*` quad per intent, and
`--color-text-*` / `--color-surface-*` / `--color-border-*` roles.

**Typography** — `--font-heading-{xs…5xl}-{size,line-height,tracking,weight}` and
`--font-text-{3xs,2xs,xs,sm,md,lg}-{size,line-height,tracking,weight}`, plus
`--font-small-caps-{md,lg}-*`, `--font-openai-sans`-equivalent
`--font-display`, and `--tracking-{tight,normal,wide}`.

**Shape** — `--radius-{2xs,xs,sm,md,lg,xl,2xl,3xl,4xl}-base` and the derived
`--radius-*` computed through `--corner-radius-scale` (Orchester: `1.25`, matching
Codex's softness — this is the single cheapest way to get the same *feel*).
`--border-width-hairline: .5px`.

**Elevation** — `--elevation-{100,200,300,400}-geo`, `--shadow-*`,
`--elevation-stroke`, `--elevation-rail`, `--elevation-inspector`,
`--elevation-composer`, `--elevation-panel`.

**Motion** — `--cubic-enter`, `--cubic-exit`, `--cubic-exit-snappy`, `--cubic-move`
and `--ease-*`.

**Density** — `--density-row-height`, `--density-gap`, overridden under
`[data-density='compact']`.

### 3.2 Migration of the existing tokens (wave 2)

`--color-bg-base`, `--color-bg-surface`, `--color-bg-element`, `--color-bg-elevated`,
`--color-bg-input`, `--color-border-base`, `--color-border-strong`,
`--color-text-primary/secondary/tertiary`, `--color-status-*` are **kept as names**
and re-pointed at L1, so no consumer changes:

| legacy | becomes |
|---|---|
| `--color-bg-base` | `--color-surface-tertiary` (`--gray-50` dark / `--gray-75` light) |
| `--color-bg-surface` | `--color-surface-base` (`--gray-200` / `--gray-0`) |
| `--color-bg-element` | `--color-surface-secondary` (`--gray-100` / `--gray-50`) |
| `--color-bg-elevated` | `--color-surface-elevated` (`--gray-300` / `--gray-0`) |
| `--color-border-base` | `--color-border-default` (`--alpha-12` / `--alpha-10`) |
| `--color-border-strong` | `--color-border-emphasis` (`--alpha-20` / `--alpha-15`) |
| `--color-text-primary` | `--color-text-emphasis` (`--gray-1000` both themes) |
| `--color-text-secondary` | `--color-text-default` (`--gray-700` / `--gray-700`) |
| `--color-text-tertiary` | `--color-text-subtle` (`--gray-600` / `--gray-500`) |
| `--color-status-success` | `--color-intent-success-text` |
| `--color-status-warning` | `--color-intent-warning-text` |
| `--color-status-error` | `--color-intent-danger-text` |
| `--color-status-info` | `--color-intent-info-text` |
| `--color-accent` | `--blue-300` dark / `--blue-500` light (for the `codex` scheme) |

The two visible consequences: dark surfaces get marginally flatter (page and
surface move two neutral steps closer), and every border becomes an alpha over
whatever is behind it instead of a colour mixed against a guessed background.
This lands with an updated snapshot and a contrast re-check, because
`--alpha-12` over `#101010` is not the same contrast as `#262b34` over `#0e1013`.

### 3.3 Colour schemes

Orchester keeps its four schemes (`codex`, `violet`, `teal`, `rose`) but rebuilds
each from the shared ramps so they are the *same* system at a different hue:

| scheme | dark accent | light accent |
|---|---|---|
| `codex` (default) | `--blue-300 #339cff` | `--blue-500 #0169cc` |
| `violet` | `--purple-300 #ad7bf9` | `--purple-500 #8046d9` |
| `teal` | `#4fbfad` (Orchester's own; no Codex equivalent) | `#1c7568` |
| `rose` | `--red-300 #ff6764` | `--red-500 #e02e2a` |

Plus a new, separate axis: **`data-intensity="calm|vivid"`** — `calm` uses the
accent only for focus/links/active markers, `vivid` uses it for primary fills.
`calm` is the default, because it is the Codex lesson; `vivid` preserves today's
look for anyone who wants it.

The scheme is chosen **per theme**, as the reference's appearance screen offers
it: the light accent and the dark accent of the same scheme are two colours (see
the table above), so a reader who likes `teal` in the dark often wants something
else on a light page. The pair lives in the appearance state and the older
single key is still read, meaning "both themes"; the stylesheet contract does
not change at all — `data-color-scheme` still carries whichever half belongs to
the theme in force.

## 4. Surfaces

### 4.1 Task rail (C)

- **Project block** — the current workspace, with a project switcher menu
  (`Edit project` equivalent: choose primary root, add secondary roots).
- **New task** button — full-width, secondary emphasis, `⌘/Ctrl+N`.
- **Task list** — grouped by project, then by recency. Each row:
  status glyph · title · branch (mono, truncated) · relative time · a trailing
  slot that shows either the diff stat or a **Needs input** badge.
  States: `queued`, `running`, `needs input`, `done`, `failed`, `cancelled`.
  The **needs-input** state is the only one that uses a saturated treatment.
- **Agent fleet** — collapsed by default, grouped by provider, with a stable
  identicon per agent (deterministic from `agent_id`, so it survives reconnects).
- **Account row** — workspace name, connection state, settings.

### 4.2 Unified tab strip (B)

A row of tabs over the main column. Each tab is a *surface instance*:
`task:<runId>`, `diff:<path>`, `terminal:<n>`, `agent:<agentId>`.

- Active tab: `--color-surface-base` background, hairline top accent mark.
- Overflow: scrollable strip with edge fades; `data-tabstrip-overflow` drives them.
- Close-on-middle-click, `⌘/Ctrl+W`, reorder by drag, and restore after restart
  (the runtime must persist open tabs — see the plan's T1-04).
- Keyboard: `⌃Tab` cycles, `⌘/Ctrl+1..9` selects.

### 4.3 Thread header (D)

Title (editable inline) · branch chip · run status pill · then actions:
`Resume`, `Cancel`, `Share`, `Overflow`. When the transcript scrolls the header
gains `data-header-edge-scroll`, which draws the hairline and the top fade.

### 4.4 Transcript (F)

- **Virtualised** turns; only visible turns mount. Turns carry
  `data-virtualized-turn`.
- **Turn anatomy**: role marker · content · actions. Assistant turns render
  markdown with `MarkdownRoot`, `CodeBlock` (copy + language + line numbers),
  `Table`, `MermaidBlock`, and a **wide-block** escape that breaks the prose
  measure.
- **Tool invocations** render as cards keyed by `call_id`, with a collapsed
  summary line and an expandable body. Concurrent same-name calls must remain
  visually distinct (the runtime already keys on `call_id`).
- **Reasoning** is collapsed by default behind a disclosure.
- **Message navigation rail** — a floating rail on the transcript's inline-end
  edge listing user turns; hover expands to a label; dragging scrubs
  (`data-scrubbing`). This is the mitigation for long governed runs.
- **Scroll contract** — `data-can-scroll-up` / `data-can-scroll-down` on the
  transcript root drive (a) the top fade under the header and (b) the
  scroll-to-bottom button. The transcript sticks to the bottom only while the user
  is already at the bottom.
- **Streaming** — token arrival must not reflow the page background. Streaming
  text uses `content-visibility: auto` per turn and appends without measuring
  ancestors.

### 4.5 Plan strip (G)

Sits directly above the composer. Shows the agent's **plan as progress**, not as a
list: a segmented progress bar plus the current step. Expands to the full todo
list and the validator results. `data-plan-state` ∈
`idle | active | blocked | done`; `blocked` is what surfaces *needs input*.

### 4.6 Composer (H)

Adopted from the Codex composer state machine, reduced to what Orchester needs:

```
┌───────────────────────────────────────────────────────────┐
│ context row: [project ▾] [model · effort]                 │
│ ┌ one field ────────────────────────────────────────────┐ │
│ │ prompt textarea (auto-grow 1–12 rows)                 │ │
│ │ footer: [approval preset ▾]      [count] [(↑) Run]    │ │
│ └───────────────────────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────┘
```

- The **field owns the box**, as the reference draws it: one rounded surface
  holding the prompt and the row of controls under it, rather than a card with a
  second bordered box inside it. The context row stays outside and above, because
  which workspace and model a run will use is a fact about the run rather than a
  control in the field.
- The field is **named by its placeholder**, not by a heading over it: the
  accessible name moves onto the prompt itself. The reference prints no heading
  and no tally there, and the tally appears only once the draft is within a
  tenth of the limit.
- The action is a **shape**: a round send that points the way the prompt goes,
  and a square stop while a run is in flight. Its word stays as the accessible
  name.

- The composer is **docked to the bottom of the transcript column**, not to the
  window (this is what the operator asked for in an earlier round), so the
  transcript's scroll region ends above it.
- `data-composer-state` ∈ `idle | dragging | submitting | running | cancelling`.
- Approval preset is a **footer dropdown with three named presets** —
  `Ask`, `Governed`, `Full access` — plus the risky-combination dialog. `Full
  access` requires an explicit confirmation and paints the composer border with
  the danger intent.
- Model, effort and preset are **run-scoped**: changing them affects the next run
  and is persisted per task, never globally, so a resumed run keeps its settings.
- `⌘/Ctrl+Enter` and `Enter` both submit; `Shift+Enter` newlines; IME composition
  is never interrupted.
- `/` opens the command palette above the composer with **empty and loading
  states** (cmdk-style), not a bare list. `@` opens context mentions.

### 4.7 The run's surfaces (formerly the inspector, region E)

**There is no right column.** The reference this surface is now built against
draws a conversation that runs to the window's edge, and the operator asked for
exactly that ("不要右侧栏"), so the shell draws two regions - the rail and the
transcript - and the surfaces that used to be the inspector's are drawn in the
**bottom panel** (region I), which was already the shell's remaining collapsible
region with a tab mechanism of its own.

Tabs, in the panel: `Context`, `Approvals`, `Review`, `Terminal`, `Exec output`,
`Audit log` - the run's own surfaces first, because they are what a reader
consults while a run is in flight.

The panel's open state and chosen surface are **controlled** by the view: the
header bell opens the panel on approvals, the tab strip's inspector tab opens it
on review, and the panel reports what the reader asked for rather than keeping a
second copy of "which surface".

- **Review** — file tree ordered identically to the diff list, filters for
  `All / Staged / Unstaged / Branch / Last turn`, per-file diff with the added /
  removed / modified decorations, line-wrap toggle, and inline comment affordance.
- **Approvals** — a queue. Each card: risk summary, redacted action, the exact
  scope, and `Allow once` / `Allow for run` / `Deny`. Stale entries (row-version
  mismatch) render as a distinct "superseded" state, never as an error.
- **Agents** — the fleet, per-agent metrics, and delegation controls.
- **Terminal** — `xterm`-style surface, with the terminal tabs in the panel
  itself: with no right column there is nowhere else for them to be.

### 4.8 Bottom panel (I)

Collapsible strip for the run's own surfaces, raw `exec` output and the audit log,
with a tab mechanism of its own. The tab strip's panel control toggles it.

## 5. States

Every surface implements the same six, with the same visual grammar:

| state | grammar |
|---|---|
| `empty` | centred mark + one-line "what next", never a bare "No data" |
| `loading` | skeleton blocks that match the final layout's rhythm |
| `streaming` | shimmering status pill + word-arrival on text, no layout shift |
| `needs input` | saturated caution treatment + a single obvious action |
| `failed` | inline alert with the redacted reason and a retry affordance |
| `cancelled` | neutral, non-alarming, with a resume affordance |

## 6. Motion and reduced motion

- Panel open/close: `--cubic-enter` at `--transition-normal` (240 ms).
- Tab switch: 140 ms opacity + 1 px translate; no width animation.
- Composer send: 120 ms, and the sent text never animates out of the box.
- Streaming: `shimmer` on the status pill only; never on body text.
- **Reduced motion** is an explicit, overridable root attribute:
  `:root[data-reduced-motion='true']` collapses durations, and
  `@media (prefers-reduced-motion: reduce) { :root:not([data-reduced-motion='false']) … }`
  honours the OS unless the user has opted back in. The preference is stored and
  set by the bootstrap script alongside the theme.

## 7. Accessibility contract

- Focus: 2 px `--color-border-focus` outline at 2 px offset, on `:focus-visible`.
  Never remove without replacing.
- Landmarks: `nav` (rail), `main` (transcript column), `complementary`
  (inspector), `contentinfo` (bottom panel), `banner` (header).
- Live regions: run status, approval arrival and validator results are
  `aria-live="polite"`; failures are `assertive`. Streaming tokens are **not**
  announced; the turn is announced once on completion.
- Minimum hit target 32×32 px, 40×40 in compact density.
- Contrast: body text ≥ 4.5:1, UI borders ≥ 3:1 against their adjacent surface.
  `--alpha-08` borders on `#0d0d0d` must be verified rather than assumed.
- Every icon-only control has an accessible name; every destructive action names
  its object.

## 8. Desktop chrome

- `data-orchester-surface="desktop"` set by the Tauri shell.
- macOS: `titleBarStyle: "Overlay"`, traffic lights inset, and
  `--window-chrome-height: 38px` with a draggable region that does **not** cover
  the tab strip's interactive elements.
- Windows: custom caption buttons at the OS metric (46×32 px hit target),
  Snap-Layouts hint over the maximize button, and a **fallback to an opaque
  surface** when translucency is unavailable.
- `⌘/Ctrl+W` closes the active tab; the window closes only when the last tab does,
  with a confirm when a run is active.

## 9. Density

`--density-row-height: 32px` (comfortable) / `26px` (compact), and
`--density-gap: var(--space-3)` / `var(--space-2)`. Compact is the right default
for a long task list; comfortable for the composer and inspector.

## 10. What Orchester will *not* do

- No achromatic inverted primary button (see §0).
- No collapsed `text-secondary`/`text-tertiary` (§12 of the research): Orchester's
  governance copy needs real hierarchy.
- No animated diff stat, no celebratory motion, no confetti.
- No emoji picker, pets, or ambient suggestion chrome from the Codex app: those
  belong to a consumer assistant.
- No invented runtime state. If `/api/v1/...` does not serve it, the UI shows
  `unavailable`, not a plausible guess.