# Codex desktop app — UI research

> Evidence-based research into the UI of OpenAI's Codex desktop surface, gathered
> to drive Orchester's own front-end design. Every claim below names the artefact
> it came from. Nothing here is reconstructed from memory.

## 1. Method and evidence

Three kinds of evidence were available on this machine, in descending order of
strength:

| # | Evidence | Path | Strength |
|---|---|---|---|
| E1 | **The shipped production webview bundle of the Codex/ChatGPT desktop app**, as installed inside the official OpenAI VS Code extension. Contains the real stylesheets, so the real token names and values. | `%USERPROFILE%\.vscode\extensions\openai.chatgpt-26.5908.31748-win32-arm64\webview\` | Primary — the actual shipped CSS |
| E2 | **The official product changelog**, which documents each desktop release and its user-visible features. | `https://learn.chatgpt.com/docs/changelog` (`ChatGPT & Codex changelog`) | Primary — vendor documentation |
| E3 | **A full clone of the OpenAI Codex repository** (Rust workspace, TUI, app-server protocol, in-tree docs). | `agent-research/codex/` | Primary — vendor source |

The extraction from **E1** was mechanical, not visual:

- `webview/assets/app-initial-ddb6c251267b.css` (834 KB) plus 218 other
  stylesheets were parsed for every `--custom-property` declaration, grouped by
  the selector that declares them. Result: **2075 declarations across 102
  selector blocks** — a complete picture of the token system.
- All stylesheets were scanned for CSS-module class names of the form
  `._ComponentName_hash_N`, for `data-*` attributes, and for `@keyframes`.
  Result: **937 component names, 332 state attributes, 74 keyframes, 81
  Codex-scoped custom properties**. The class names are readable even though the
  bundles are minified, and they enumerate the real component tree.
- The `data-*` attributes are the app's component state machine, which is the
  single most useful thing in the bundle for a designer: they say exactly which
  variants each surface has.

**Limits.** This is a stylesheet-and-markup reading, not a rendered inspection:
no screenshot of the running desktop app was analysed, so pixel measurements are
only known where the CSS states them. Layout is inferred from class names,
`data-*` state attributes, grid/flex declarations, and the changelog. Claims that
rest on inference are marked **(inferred)**; everything else is quoted.

## 2. What "the Codex desktop app" is

The changelog corrects a common assumption. The Codex desktop app is no longer a
separate product:

- **2026-02-02** — "Introducing the Codex app": Codex app launches for macOS,
  described as "a desktop interface for running agent threads in parallel …
  It includes a project sidebar, thread list, and review pane." (E2)
- **2026-03-04** — Codex app ships for Windows, version 26.304. (E2)
- **2026-07-09** — *"Codex joins the ChatGPT desktop app"*, version **26.707**:
  "Codex is now part of the ChatGPT desktop app on macOS and Windows. Existing
  Codex app users can update as usual and keep their projects, settings, and
  workflows. You can make Codex the default view and, on macOS, keep the Codex
  app icon." (E2)
- **2026-08-12/13** — Codex in the ChatGPT desktop app reaches Linux preview. (E2)

So the shipping UI today is **the Codex view of the ChatGPT desktop app**,
alongside Chat and Work. This matches the bundle: its root carries
`data-codex-window-type` with values `extension`, `browser` and an Electron
variant, and `data-codex-os` for platform differences (E1). One codebase renders
as the IDE extension panel, as the desktop Electron window, and as a browser tab.

That is itself the first design lesson for Orchester, which has exactly the same
problem: `apps/web` is served three ways (local WebUI, GitHub Pages site, Tauri
desktop window). Codex solves it with **one root attribute per axis** rather than
three builds.

## 3. The token system

This is the most transferable artefact in the bundle. Codex's design system is
layered, and the layering is the point:

```
raw palette          --gray-fixed-700, --blue-300, --red-500 …
      ↓
theme-aware aliases  --gray-0 … --gray-1000        (light/dark pairs)
      ↓
semantic intents     --color-text-secondary, --color-surface-elevated …
      ↓
app tokens           --app-color-text-foreground, --app-color-border …
      ↓
component tokens     --composer-layout-surface-background, --elevation-composer …
```

Light/dark is expressed by compiling CSS `light-dark()` into
`var(--lightningcss-light, A)var(--lightningcss-dark, B)` — a single declaration
that carries both values. Orchester currently expresses the same two axes with
`[data-theme='dark']` / `[data-theme='light']` attribute selectors, which is
equivalent in effect and simpler to read; the interesting part of Codex's scheme
is the **layering**, not the compile trick.

### 3.1 Raw palette

Neutral ramp — the whole app is built from this. `--gray-fixed-*` are absolute
(same value in both themes); `--gray-*` flip.

| token | value | token | value |
|---|---|---|---|
| `--gray-fixed-0` | `#fff` | `--gray-fixed-500` | `#5d5d5d` |
| `--gray-fixed-25` | `#fcfcfc` | `--gray-fixed-550` | `#4f4f4f` |
| `--gray-fixed-50` | `#f9f9f9` | `--gray-fixed-600` | `#414141` |
| `--gray-fixed-75` | `#f3f3f3` | `--gray-fixed-650` | `#393939` |
| `--gray-fixed-100` | `#ededed` | `--gray-fixed-700` | `#303030` |
| `--gray-fixed-150` | `#dfdfdf` | `--gray-fixed-750` | `#282828` |
| `--gray-fixed-200` | `#cdcdcd` | `--gray-fixed-800` | `#212121` |
| `--gray-fixed-250` | `#b9b9b9` | `--gray-fixed-850` | `#1c1c1c` |
| `--gray-fixed-300` | `#afafaf` | `--gray-fixed-900` | `#181818` |
| `--gray-fixed-350` | `#9f9f9f` | `--gray-fixed-925` | `#161616` |
| `--gray-fixed-400` | `#8f8f8f` | `--gray-fixed-950` | `#131313` |
| `--gray-fixed-450` | `#767676` | `--gray-fixed-975` | `#101010` |
| | | `--gray-fixed-1000` | `#0d0d0d` |

Dark-mode aliases (`--gray-N`) resolve to, in order:
`--gray-0 #0d0d0d`, `--gray-25 #101010`, `--gray-50 #131313`, `--gray-75 #161616`,
`--gray-100 #181818`, `--gray-150 #1c1c1c`, `--gray-200 #212121`, `--gray-250 #282828`,
`--gray-300 #303030`, `--gray-350 #393939`, `--gray-400 #414141`, `--gray-450 #4f4f4f`,
`--gray-500 #5d5d5d`, `--gray-550 #767676`, `--gray-600 #8f8f8f`, `--gray-650 #9f9f9f`,
`--gray-700 #afafaf`, `--gray-750 #b9b9b9`, `--gray-800 #cdcdcd`, `--gray-850 #dcdcdc`,
`--gray-900 #ededed`, `--gray-1000 #fff`.

Light-mode resolves to the `--gray-fixed-*` ramp read the other way: `--gray-0 #fff`,
`--gray-50 #f9f9f9`, `--gray-75 #f3f3f3`, `--gray-100 #ededed`, `--gray-400 #8f8f8f`,
`--gray-500 #5d5d5d`, `--gray-750 #282828`, `--gray-1000 #0d0d0d`.

Accent ramps (all six are complete 25→1000 scales; the `-a*` variants are the
same hue at 4/13/25/40/60/80 % alpha):

| step | blue | green | orange | red | purple | yellow |
|---|---|---|---|---|---|---|
| 25 | `#f5faff` | `#edfaf2` | `#fff5f0` | `#fff0f0` | `#f9f5fe` | `#fffbed` |
| 50 | `#e5f3ff` | `#d9f4e4` | `#ffe7d9` | `#ffd9d9` | `#efe5fe` | `#fff6d9` |
| 75 | `#cce6ff` | `#b8ebcc` | `#ffcfb4` | `#ffc6c5` | `#e0cefd` | `#ffeeb8` |
| 100 | `#99ceff` | `#8cdfad` | `#ffb790` | `#ffa4a2` | `#ceb0fb` | `#ffe48c` |
| 200 | `#66b5ff` | `#66d492` | `#ff9e6c` | `#ff8583` | `#be95fa` | `#ffdb66` |
| **300** | **`#339cff`** | `#40c977` | `#ff8549` | `#ff6764` | `#ad7bf9` | `#ffd240` |
| 400 | `#0285ff` | `#04b84c` | `#fb6a22` | `#fa423e` | `#924ff7` | `#ffc300` |
| 500 | `#0169cc` | `#00a240` | `#e25507` | `#e02e2a` | `#8046d9` | `#e0ac00` |
| 600 | `#004f99` | `#008635` | `#b9480d` | `#ba2623` | `#6b3ab4` | `#ba8e00` |
| 700 | `#003f7a` | `#00692a` | `#923b0f` | `#911e1b` | `#532d8d` | `#916f00` |
| 800 | `#013566` | `#004f1f` | `#6d2e0f` | `#6e1615` | `#3f226a` | `#6e5400` |
| 900 | `#00284d` | `#003716` | `#4a2206` | `#4d100e` | `#2c184a` | `#4d3b00` |
| 1000 | `#000d19` | `#001207` | `#211107` | `#1f0909` | `#100a19` | `#1a1400` |

**`--blue-300 #339cff` is the Codex accent.** It is not used as a fill for
primary buttons: see §3.3.

Alpha over the foreground is a first-class scale, used for every border and
separator rather than hard-coded `rgba()`: `--alpha-base` is `#0d0d0d` in light
and `#fff` in dark, and `--alpha-{0,01,02,04,05,06,08,10,12,15,16,20,25,30,35,40,50,60,70}`
are that base at that percentage.

### 3.2 Semantic intents

Text and surface are split into an **intent × emphasis** matrix. The intent is
the meaning (`primary`, `secondary`, `info`, `danger`, `warning`, `caution`,
`discovery`); the emphasis is the treatment (`ghost`, `outline`, `soft`, `solid`,
`surface`). Every combination exists as a token:

| intent | text | background (soft) | border (outline) |
|---|---|---|---|
| primary | `--color-text-emphasis` → `--gray-1000` | `--alpha-05` / `--alpha-08` | `--alpha-16` / `--alpha-25` |
| secondary | `--gray-500` / `--gray-700` | `--color-surface-secondary` | same as primary |
| info | `--blue-500` / `--blue-200` | `--blue-a25` / `--blue-a50` | `--blue-500` |
| success | `--green-700` / `--green-400` | `--green-a25` / `--green-a50` | `--green-500` / `--green-600` |
| warning | `--orange-700` / `--orange-500` | `--orange-a25` / `--orange-a50` | `--orange-500` |
| caution | `--yellow-700` / `--yellow-500` | `--yellow-a25` / `--yellow-a50` | `--yellow-700` |
| danger | `--red-700` / `--red-500` | `--red-a25` / `--red-a50` | `--red-500` |
| discovery | `--purple-700` / `--purple-500` | `--purple-a25` / `--purple-a50` | `--purple-500` |

Three text roles sit alongside: `--color-text` (body, `--gray-750` / `--gray-850`),
`--color-text-tertiary` (`--gray-400` / `--gray-600`), `--color-text-disabled`
(`--gray-400` / `--gray-500`).

Surfaces: `--color-surface` (`--gray-0` / `--gray-200`), `--color-surface-secondary`
(`--gray-50` / `--gray-100`), `--color-surface-tertiary` (`--gray-75` / `--gray-50`),
`--color-surface-elevated` (`--gray-0` / `--gray-300`),
`--color-surface-elevated-secondary` (`--gray-50` / `--gray-400`).

**The stack is unusually flat.** In dark mode the page is `#0d0d0d`, the primary
surface is `#212121`, secondary `#181818`, tertiary `#131313`, elevated `#303030`.
There is no large jump between "page" and "card": separation is carried by the
hairline border and the 0.5 px stroke shadow, not by a big value shift.

### 3.3 The `--app-color-*` layer, and the inverted primary button

Codex keeps its own vocabulary on top, which is what the VS Code theme bridge
consumes (702 properties map `--vscode-*` onto `--app-color-*`):

| `--app-color-*` | light | dark |
|---|---|---|
| `text-foreground` | `#1a1c1f` | `--gray-fixed-150` `#dfdfdf` |
| `text-foreground-secondary` | `text-foreground` | `--gray-fixed-0` `#fff` |
| `text-foreground-tertiary` | `text-foreground` | `--gray-fixed-0` `#fff` |
| `background-surface` | `--gray-fixed-0` `#fff` | `--gray-fixed-900` `#181818` |
| `background-surface-under` | `--gray-fixed-50` | `black` |
| `background-elevated-primary` | `#fff` | `--gray-fixed-800` `#212121` |
| `background-elevated-primary-opaque` | `#fff` | `--gray-fixed-750` `#282828` |
| `background-editor-opaque` | `--gray-fixed-100` | `--gray-fixed-800` |
| `background-application-menu` | `--gray-fixed-50` | `--gray-fixed-800` |
| `background-button-primary` | `text-foreground` | `--gray-fixed-1000` `#0d0d0d` |
| `background-button-secondary` | `text-foreground` | `--gray-fixed-0` `#fff` |
| `background-button-tertiary` | `text-foreground` | `--gray-fixed-0` `#fff` |
| `text-button-primary` | `--gray-fixed-0` | `--gray-fixed-1000` |
| `text-button-secondary` | `text-foreground` | `--gray-fixed-300` |
| `text-button-tertiary` | `--gray-fixed-500` | `--gray-fixed-500` |
| `border` | `text-foreground` @ 8 % | `#fff` @ 8 % |
| `border-heavy` | `text-foreground` | `#fff` |
| `border-light` | `text-foreground` | `#fff` |
| `border-focus` | `--blue-300` | `--blue-300` |
| `accent-blue` / `text-accent` / `icon-accent` | `--blue-300` | `--blue-300` |
| `background-tip-badge` | `#e8f3fe` | `--blue-300` |
| `icon-success` | `--green-500` | `--green-300` |
| `icon-warning` | `--orange-500` | `--orange-300` |
| `icon-error` | `--red-500` | `--red-300` |
| `decoration-added` / `editor-added` | `--green-500` | `--green-300` |
| `decoration-deleted` / `editor-deleted` | `--red-600` | `--red-400` |
| `decoration-modified` | `--orange-700` | `--orange-300` |
| `decoration-unchanged` | `--gray-fixed-300` | `--gray-fixed-600` |

The single most surprising finding: **the primary button is achromatic and
inverts.** The shell does not use the `--app-color-*` bridge for it; it uses the
L1 pair, which resolves as:

| | `--color-background-primary-solid` | `--color-text-primary-solid` |
|---|---|---|
| dark | `--gray-950` = **`#f3f3f3`** (near-white) | `--color-text-inverse` = `--gray-0` = **`#0d0d0d`** |
| light | `--gray-900` = **`#181818`** (near-black) | `--color-text-inverse` = `--gray-0` = **`#fff`** |

So the primary action is a **white button with black text in dark mode**, and a
near-black button with white text in light mode. Codex does *not* paint its accent
blue onto primary buttons; the accent is reserved for links, focus, tips and
active states. This is a deliberate, unusual choice and it is what makes the
product read as calm rather than as a "blue app". Orchester currently uses the
accent as the primary button fill, so this is a real divergence to decide on
(§12).

(A related trap in the same bundle: the VS Code bridge sets
`--app-color-background-button-primary` and `--app-color-text-button-primary` to
the *same* `--gray-fixed-1000 #0d0d0d` in dark mode, which would be invisible.
That pair only feeds `--vscode-button-*` and is overridden by the editor in the
extension; the desktop shell renders from the L1 pair above. Reading only the
`--app-color-*` layer would have produced the wrong conclusion — which is why
every claim here was traced to the selector that actually wins.)

Note also that `text-foreground-secondary` and `-tertiary` collapse to the *same*
value as `text-foreground` in the app layer — hierarchy in the Codex shell comes
from size and weight first, with opacity/muted greys only in dense lists.

### 3.4 Typography

Font stacks:

- `--font-openai-sans: "OpenAI Sans", var(--font-sans-default)`
- `--font-sans-default: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif`
- `--font-mono-default: ui-monospace, "SFMono-Regular", "SF Mono", Menlo, Consolas, "Liberation Mono", monospace`
- `--font-serif: ui-serif, Georgia, Cambria, "Times New Roman", Times, serif`

Two scales coexist. A **heading** scale and a **text** scale, each with size,
line-height, tracking and weight bound together in one token set:

| heading | size | line-height | tracking | weight |
|---|---|---|---|---|
| `xs` | 1rem | 1.5rem | normal | 600 |
| `sm` | 1.125rem | 1.625rem | normal | 600 |
| `md` | 1.25rem | 1.625rem | normal | 600 |
| `lg` | 1.5rem | 1.75rem | tight | 600 |
| `xl` | 2rem | 2.375rem | tight | 600 |
| `2xl` | 2.25rem | 2.625rem | tight | 600 |
| `3xl` | 3rem | 3rem | tight | 600 |
| `4xl` | 3.75rem | 3.75rem | tight | 600 |
| `5xl` | 4.5rem | 4.5rem | tight | 600 |

| text | size | line-height | tracking |
|---|---|---|---|
| `3xs` | .5rem | .75rem | wide |
| `2xs` | .625rem | .875rem | wide |
| `xs` | .75rem | 1.125rem | wide |
| `sm` | .875rem | 1.25rem | normal |
| `md` | 1rem | 1.5rem | normal |
| `lg` | 1.125rem | 1.8125rem | normal |

Plus a `small-caps` pair (`md` .6875rem/.9375rem at .65px tracking, `lg`
.8125rem/1.1875rem at .6px) used for section labels.

Fluid tokens: `--tracking-tight: -.025em`, `--tracking-normal: 0`,
`--tracking-wide: .025em`; `--leading-tight 1.25`, `--leading-snug 1.375`,
`--leading-normal 1.5`, `--leading-relaxed 1.625`, `--leading-dense: calc(7/6)`.

**The app chrome runs small.** The Codex chat surface overrides both scales with
`--codex-chat-font-size: 13px` (from `--font-ui-size`, default `13px`) and
`--codex-chat-code-font-size: 12px`, and the extension build rebinds
`--font-ui-family`/`--font-code-family` to the VS Code editor font so the panel
matches the surrounding editor. Elsewhere, the Tailwind bridge exposes
`--text-xs 11px`, `--text-sm 12px`, `--text-base 14px`, `--text-lg 16px`,
`--text-xl 28px`, `--text-2xl 36px`, `--text-3xl 48px`, `--text-4xl 72px`.

### 3.5 Shape, and the Codex corner

Base radii: `--radius-2xs-base .125rem`, `-xs-base .25rem`, `-sm-base .375rem`,
`-md-base .5rem`, `-lg-base .625rem`, `-xl-base .75rem`, `-2xl-base 1rem`,
`-3xl-base 1.25rem`, `-4xl-base 1.5rem`, `--radius-full 9999px`.

Every radius is then `calc(<base> * var(--corner-radius-scale))`, and that
multiplier is a global knob: `--corner-radius-scale: 1` by default, but **Codex
sets `--codex-corner-radius-scale: 1.25`**, so Codex surfaces are 25 % rounder
than the shared ChatGPT scale. Under support, Codex additionally sets
`--codex-corner-shape: superellipse(1.5)` via
`@supports (corner-shape: superellipse(1.5))`, which turns circles into squircles.
That is the whole reason the desktop app looks "soft" without looking bubbly.

Component radii worth copying: `--radius-button-toolbar: var(--radius-lg)`,
`--radius-token-row: 9999px`, `--radius-token-composer-single-line:
calc(var(--spacing) * 5.5)`.

**`--border-width-hairline: .5px`** — Codex uses true half-pixel strokes, and
`--elevation-stroke: 0 0 0 .5px var(--color-border-strong)` makes the hairline
available as a shadow so it does not affect layout.

### 3.6 Spacing

Everything is a multiple of `--spacing: .25rem` (4 px). Named tokens that matter
for the shell:

| token | value | meaning |
|---|---|---|
| `--padding-toolbar` | `calc(var(--spacing) * 4)` = 16px | toolbar inset |
| `--spacing-token-sidebar` | `clamp(240px, var(--codex-sidebar-preferred-width, 275px), min(520px, calc(100vw - 320px)))` | **sidebar width** |
| `--spacing-token-safe-header-left/right` | `0px` | header safe-area overrides |
| `--spacing-button-toolbar-inline` | 8px | toolbar button gutter |
| `--spacing-token-button-composer` | 28px | composer action button |
| `--spacing-token-button-composer-sm` | 20px | compact composer button |
| `--spacing-token-button-composer-gap` | 4px | |
| `--spacing-menu-row-content` | 8px | menu row padding |
| `--home-composer-inline-inset` | `calc(var(--spacing) * 3.25)` = 13px | |
| `--composer-inline-overhang` | `calc(var(--spacing) * 6)` = 24px | |
| `--spacing-composer-footer-bottom` | 8px | |
| `--spacing-composer-input-footer` | 4px | |
| `--spacing-composer-single-line-controls` | 5px | |
| `--spacing-home-suggestion-row` | 40px | suggestion row |

The sidebar rule is the important one: **minimum 240 px, preferred 275 px,
maximum `min(520px, 100vw − 320px)`** — the max is derived from leaving 320 px
for the rest of the window, so the sidebar can never crowd out the transcript.
That single clamp encodes a layout policy that Orchester currently only
approximates with three unrelated variables.

### 3.7 Elevation

Codex separates *geometry* from *colour* so the same shape can be reused at
different strengths:

```
--elevation-100-geo: 0 1px 2px -1px      --shadow-100 → var(--elevation-100-geo) rgb(var(--shadow-color) / var(--shadow-alpha-100))
--elevation-200-geo: 0 2px 4px -1px       --shadow-200 → …-200-geo / var(--shadow-alpha-200)
--elevation-300-geo: 0 4px 8px -2px       --shadow-300 → …-300-geo / var(--shadow-alpha-300)
--elevation-400-geo: 0 8px 16px -4px      --shadow-400 → …-400-geo / var(--shadow-alpha-400)
                                          -strong = alpha × 1.25, -stronger = alpha × 1.6
```

Named surfaces:

| token | value |
|---|---|
| `--elevation-stroke` | `0 0 0 .5px var(--color-border-strong)` |
| `--elevation-sidebar` | `var(--elevation-stroke), 0 3px 7.5px #00000008, 0 0 16px #00000005` |
| `--elevation-prominent` | `var(--elevation-stroke), 0 3px 7.5px #0000000a, 0 0 20px #0000000d` |
| `--elevation-composer` | `0 0 0 1px #0000000a, 0 2px 8px 0 #0000000a, 0 4px 80px 8px #00000006` |
| `--elevation-composer-dark` | `inset 0 0 1px 0 #fff3` |

`--elevation-composer` deserves attention: an 80 px blur at 8 px spread and
6/255 alpha. It is almost invisible, and that is the effect — the composer reads
as *lifted off* the page without a visible drop shadow.

### 3.8 Motion

```
--cubic-enter: cubic-bezier(.19, 1, .22, 1)     /* fast out, very long tail   */
--cubic-exit:  cubic-bezier(.8, 0, .4, 1)       /* slow out, sharp in         */
--cubic-move:  cubic-bezier(.65, 0, .35, 1)     /* symmetric                  */
--ease-enter-snappy: cubic-bezier(.23, 1, .32, 1)
--ease-enter: var(--cubic-enter)   --ease-in-out: cubic-bezier(.4, 0, .2, 1)
--ease-out: cubic-bezier(0, 0, .2, 1)
```

Repeated keyframes in the bundle: `FadeIn`, `shimmer`, `cadencedShimmer`,
`cadencedShimmerSweep`, `cadencedShimmerHighlight`, `wordArrival`,
`ImageEnter`, `statusPillProgress`, `daybreak`, `bloom`, `dots`, `curtain`.

Reduced motion is handled by an **explicit root attribute**, not only the media
query: components style off `:root[data-reduced-motion='true']` *and*
`@media (prefers-reduced-motion: reduce) { :root:not([data-reduced-motion='false']) … }`,
and one rule collapses `--composer-rail-transition-duration` to `0s` under both.
That two-way design lets a user override their OS in either direction.

## 4. App shell anatomy

Reconstructed from `data-*` state attributes and CSS bundle names (E1).

**Root attributes** (`<html>` or `#root`): `data-theme`, `data-codex-os`,
`data-codex-window-type` (`extension` | `browser` | Electron),
`data-codex-window-chrome`, `data-codex-layout`, `data-reduced-motion`,
`data-transitions-ready`.

**Shell regions**, each with its own state attribute:

| region | state attributes | evidence |
|---|---|---|
| left sidebar | `data-app-shell-left-panel-appearance`, `data-app-shell-responsive-sidebar` | collapses to a responsive variant |
| header | `data-app-shell-header-layout`, `data-app-shell-header-edge-scroll`, `data-app-shell-header-edge-scroll` | edge-scroll shadow when the header content scrolls |
| main content | `data-app-shell-main-surface`, `data-app-shell-main-content-layout`, `data-app-shell-main-content-top-fade`, `data-app-shell-main-toolbar-trailing-slot`, `data-app-shell-focus-area` | a top fade masks content scrolling under the toolbar |
| tabs | `data-app-shell-tabs`, `data-app-shell-tab-controller`, `data-app-shell-tab-strip-controller`, `data-app-shell-tab-row`, `data-app-shell-tab-separator`, `data-app-shell-unified-tab-strip` | **a unified tab strip is a first-class shell region** |
| right panel | `data-app-shell-right-panel-full-width` | the right panel can go full-width |
| workspace | `data-app-shell-workspace-layout` | |
| thread | `data-app-shell-thread-edge-divider`, `data-app-shell-thread-selected`, `data-app-shell-tab-selected` | |
| scroll | `data-can-scroll-up`, `data-can-scroll-down` | drives the top fade and the scroll-to-bottom affordance |

**Panel model.** Bundle names name the real panels:
`terminal-panel`, `exec-shell-container`, `xterm-window-zoom`, `diff-comment-card`,
`right-panel-composer-overlay`, `cloud-browser-side-panel`, `cloud-browser-preview`,
`pdf-preview-panel`, `text-file-editor-tab-content`, `use-file-editor-tab-lifecycle`,
`PopcornElectronPresentationPanel`, `floating-panel`, `quick-chat-frame`,
`worktree-environment-dropdown`, `thread-user-message-navigation-rail-app`,
`thread-scroll-layout`, `scroll-to-bottom-buton` (sic), `home-composer-mode-toggle`,
`codex-micro-bridge`, `mcp-app-resource-content`, `tab-content`, `dialog`, `form`,
`editor`, `impl`, `page`, `profile`, `control-panel`, `library-page`,
`chronicle-settings-page`, `onboarding-page`, `global-dictation-page`,
`external-agent-import-provider-step`, `thread-emoji-picker-content`.

The changelog confirms the behaviour of two of these: *"Added `Default terminal
location` in General settings. When the bottom panel is enabled, choose whether
the terminal shortcut and environment actions open terminal tabs in the bottom
panel or the right panel."* (26.601, E2) — so the app has **both a bottom panel
and a right panel, and the user chooses where terminal tabs land**.

**Sidebar contents** (from the changelog and bundle): projects (`data-project-row`,
`data-project-row-wrapper`, `data-projects-rows`, `data-project-selector-icon`,
`data-clear-project-button`/`data-clear-project-available`), a thread/task list,
an **Activity view** ("Added a new 'Activity view' in the sidebar to view which
chats you engaged with recently and require attention. Click the bell or use
Cmd/Ctrl+Opt+U"), Sites, Profile, and Settings.

**Projects are multi-folder.** Changelog 26.715: *"Local projects in the ChatGPT
desktop app can now include multiple related folders. From a project's menu,
select `Edit project` to add folders and choose the primary folder. New chats, Git
operations, and automatic discovery of `AGENTS.md`, skills, and `config.toml` use
the primary folder. Secondary folders remain available for file search, reading,
and editing."* Review then spans repositories: 26.727 *"See all repositories in a
multi-folder project and the lines changed in each one. Select `Review` to inspect
diffs across those repositories without switching between separate review views."*

## 5. The thread surface

State attributes on the transcript: `data-virtualized-turn-content` (turns are
virtualised), `data-thread-scroll-footer`, `data-compact-status-slot-clearance`,
`data-turn-action-width`, `data-title-aligned-trailing-rail`,
`data-codex-history-swipe-navigation`, `data-thread-find-target`,
`data-in-progress-fixed-content`, `data-codex-history-swipe-navigation`.

**Message navigation rail.** `thread-user-message-navigation-rail-app` with
`data-floating-navigation-rail-list`, `data-hierarchy-level`, `data-scrub-target`
and `data-scrubbing`: a floating rail that lets the user scrub through user
messages in a long thread, with hierarchy levels. This is the answer to "how do I
get back to something in a 3-hour agent thread", and it is a genuinely
differentiated pattern.

**Scroll behaviour** is explicit state, not a CSS trick: `data-can-scroll-up`,
`data-can-scroll-down`, and a dedicated `scroll-to-bottom-buton` bundle.

**Turn actions**: `data-turn-action-width` plus `BlockActions`, `MarkdownRoot`,
`CodeBlock`/`CodeBlockPlaceholder`, `Table*` (HeaderCell, Row, Cell, Scroller,
Wrapper, Actions, TableCellFileLink), `MermaidBlock`/`MermaidSurface`/`MermaidWideBlock`,
`VisualizationBlock`/`VisualizationWideBlock`/`VisualizationWideBlock`, `MediaParagraph`,
`MediaGridParagraph`, `MediaWideBlock`, `FadeIn`/`FadeListDecoration`,
`InlineMarkdownIsolate`, `InlineMentionFocusRing`, `ExternalMarkdownLink`,
`HorizontalRule`, `TaskList`/`TaskListItem`, `Heading`, `Blockquote`,
`NumericTableCell`, `SingleChildTableCell`, `MaskedRemoteMarker`,
`remoteGlobeBadge`, `wordArrival`. So transcripts render markdown with wide-block
escape hatches, inline mentions with their own focus ring, animations tuned per
node type (`wordArrival`), and remote/masked content marked.

**Subagents** are visible: *"Made task and subagent activity easier to follow
while Codex works"* (26.707) and *"Added stable identicons for background
subagents across the app"* (26.527) — background subagents get a stable
identicon, the same idea Orchester's `agent-presence` feature already gestures at.

**Task list semantics**: *"Improved the task list with consistent task
terminology, clearer delegated task titles, and a `Needs input` status"*
(iOS 1.2026.181) — note **"task"** is the user-facing noun, with a distinct
**needs-input** state.

## 6. The composer state machine

The richest surface in the bundle. Full attribute list, grouped:

- **Placement**: `data-composer-placement`, `data-composer-inline`,
  `data-composer-rail-placement`, `data-composer-rail-contained`,
  `data-composer-rail-overlap`, `data-composer-rail-item`,
  `data-composer-rail-variant`, `data-composer-peeked` (right-panel overlay).
- **Surface**: `data-composer-surface-variant` (`default` | `opaque`),
  `data-composer-dark`, `data-composer-surface-overflow`,
  `data-composer-radius-variant`, `data-composer-padding-variant`,
  `data-composer-spacing`, `data-composer-layout`.
- **Input**: `data-composer-rows`, `data-composer-input-variant`,
  `data-composer-markdown`, `data-composer-code-block`,
  `data-composer-code-block-toolbar`, `data-composer-inline-atom-selected`.
- **Footer**: `data-composer-footer-layout`, `data-composer-footer-collapse`,
  `data-composer-footer-label-responsive`, `data-composer-footer-label-width`,
  `data-composer-footer-responsive`.
- **Tray / utility bar**: `data-composer-expanded-top-tray`,
  `data-composer-top-tray-embedded`, `data-composer-top-tray-in-flow`,
  `data-composer-top-tray-padded`, `data-composer-utility-bar-variant`,
  `data-composer-utility-bar-scroll-area`, `data-composer-dropdown-viewport`,
  `data-composer-dropdown-foreground`, `data-composer-dropdown-has-category`,
  `data-composer-dropdown-overflowing`.
- **Attachments & drag**: `data-composer-attachments`, `data-visible-attachments`,
  `data-composer-drag-active`, `data-file-drop-active`.
- **Navigation (the `/` palette)**: `data-composer-navigation-target`,
  `data-composer-navigation-open`, `data-composer-navigation-selected`,
  `data-composer-navigation-highlight` — all in `codex-micro-bridge`.

Components: `ComposerLayoutRoot`, `ComposerLayoutBody`, `ComposerLayoutInput`,
`ComposerLayoutFooter`, `ComposerLayoutFooterLabel`, `ComposerLayoutAttachments`,
`ComposerFooter`, `ComposerFooterDropdown`, `ComposerFooterLabel`,
`ComposerDropdownLabel` (+`Category`, `Chevron`, `Icon`, `SecondaryChevron`,
`Text`, `Value`, `ValueContent`), `ComposerTopMenuShell`, `ComposerTopMenuPanel`,
`ComposerHomeUtilityBar`, `ComposerUtilityBarStaticLabel`, `ComposerSurface`,
`home-composer-mode-toggle`.

**The composer is a docked bar with a top tray, a footer, and two rails.** The
footer holds `ComposerFooterDropdown` items — these are the model picker, the
reasoning-effort control, and the mode toggle. `ModelPickerTrigger*` proves the
effort control is rendered as a layered/stacked indicator:
`ModelPickerTriggerEffortLayers`, `ModelPickerTriggerEffortViewport`,
`ModelPickerTriggerEffortText`, `ModelPickerTriggerEffortLabel`, plus
`ModelPickerTriggerInlineModeIcon`, `ModelPickerTriggerModelGroup`,
`ModelPickerTriggerModelLabel`, `ModelPickerTriggerMeasured`.

Mode / effort state: `data-fast-mode`, `data-fast-mode-enabled`,
`data-fast-mode-dot-transition`, `data-max-effort`, `data-effort-only`,
`data-explicit-model`, `data-ultra-warning-visible`, `data-max-power-selection`,
`data-maximum`. Changelog: *"Added clearer Full access warnings and a dialog when
combining Full access with Ultra"* (26.707) and *"Improved model, reasoning, and
Fast settings so changes remain scoped to the current task"* (iOS 1.2026.181).
`data-ultra-warning-visible` and `data-max-power-selection` are the state behind
that warning: **a high-effort model selection is treated as a risk worth
interrupting for.**

Also in the composer: `data-dictation-arrival`, `data-realtime-voice-reasoning-notice`,
`data-browser-autocomplete-enabled`, `data-dismissing-browser-autocomplete-mention`,
`data-image-transparency-backdrop-scope`, `data-inline-mention-interactive`,
`data-inline-url-icon`, `data-breakable-url`, `data-subtle-hover`.

Session-scoped: `data-explicit-model`, plus a real detail from the changelog —
*"Fixed restored tasks changing the selected model"* and *"Improved model,
reasoning, and Fast settings so changes remain scoped to the current task"*. The
model selection belongs to the **task**, not to the app.

## 7. Review, terminal, and browser

- **Review** (`diff-comment-card`, `data-composer-surface-variant` shared):
  *"Added filters for staged, unstaged, branch, and last-turn changes, with
  controls for comparing branches"* (iOS 1.2026.181); *"Improved workspace diff
  accuracy and expand-and-collapse navigation"*; *"Kept review diff ordering
  consistent with the file tree"* (26.608); *"Added inline review comments when
  viewing changed files"* (iOS 1.2026.153); *"Added an optional … toggle for line
  wrapping for code diffs"* (iOS 1.2026.146); *"Use PR Chat to review GitHub pull
  requests … Send inline review feedback, inspect proposed patches, and edit,
  accept, or reject them without leaving the app"* (26.707). Diff colours come
  from `--app-color-decoration-added/deleted/modified/unchanged` and
  `--app-color-editor-added/deleted`.
- **Terminal**: `terminal-panel`, `exec-shell-container`, `data-codex-xterm`,
  `xterm-window-zoom`. *"terminal scrollbar alignment"* was a fixed bug (26.602),
  i.e. the terminal is a real xterm.js surface inside a panel, not a log view.
- **Browser**: `cloud-browser-preview`, `cloud-browser-side-panel`,
  `data-has-timeline`, `data-streaming`. *"Type in the built-in browser's address
  bar to revisit pages from your browsing history or search Google…"* (26.727);
  *"It uses tab icons to indicate status"* instead of tab groups (26.519).
- **Inspector/context**: 702 `--vscode-*` mappings exist because in the extension
  the sidebar *is* the editor's sidebar, so `--color-surface-secondary` is
  `var(--vscode-sideBar-background)` and `--app-color-background-editor-opaque`
  feeds `--vscode-editor-background`. In the desktop app these fall back to the
  `--app-color-*` values.

## 8. Command palette, tabs, and menus

`data-cmdk-root`, `data-cmdk-list`, `data-cmdk-empty`, `data-command-menu-empty-state`,
`data-command-menu-loading` — the palette is **cmdk**, with explicit empty and
loading states rather than a bare list.

`data-menu-row-content`, `data-menu-section-label` — menus have sections with
labels. Overlays: `dialog`, `floating-panel`, `PopcornElectronPresentationPanel`
with `data-codex-layout`, `data-expanded`, `data-open`, `data-overflowing`.

Toasts are **sonner**: `data-sonner-toaster`, `data-sonner-toast`.

Skeleton/shimmer for loading: `SkeletonBlock` equivalents via `shimmer`,
`cadencedShimmer*`, `SkeletonBlock`-like `Placeholder`.

## 9. Settings and keyboard surface

- *"Expanded Settings search to find options from more panels, including Git and
  pets"* (26.608) — settings has a search field, not just tabs.
- *"Improved keyboard shortcut settings with keypress search and a reset-all
  action"* (26.527) — shortcuts are user-editable and discoverable by pressing keys.
- *"Added configurable Home Screen shortcuts"*, *"Added configurable shortcuts for
  the agents dashboard"* (E2) — an **agents dashboard** with configurable
  shortcuts is a shipping surface.
- `settings/Keyboard shortcuts` is a named destination (26.7xx).
- Platform-correct modifiers are spelled out in the docs (`Cmd/Ctrl+Opt+U`).

## 10. Flags, chrome, and platform adaptation

- Window chrome is a first-class bundle concern: `data-codex-window-chrome`,
  `data-codex-window-type`, `data-codex-os`, `PopupElectronPresentationPanel`,
  `popcorn-electron-surface-style`, `external-agent-import-provider-step`.
- *"Improved window rendering on systems that don't support translucent backdrops,
  including Windows 10"* (26.608) — translucency is opt-in per platform.
- *"Fixed … overlay positioning at non-default zoom levels"* (26.6xx) and
  `--codex-window-zoom: 1` with `xterm-window-zoom` — zoom is a shell-level state
  that panels must respect.
- *"Verify Codex app signatures before launch or install"*.

## 11. Terminology

| Codex noun | meaning |
|---|---|
| **task** / **thread** | a unit of agent work; "task" is the user-facing list noun, "thread" the conversation |
| **project** | a workspace, now multi-folder, with a **primary folder** |
| **worktree** | an isolated checkout for a new/forked session |
| **environment** | the setup for a task (`worktree-environment-dropdown`, environment setup script) |
| **review** | the diff/change review surface |
| **side chat** | a conversation branched off the current thread |
| **goal** | an objective the agent drives toward over hours/days (Goal mode) |
| **plan** | the agent's step list, with progress |
| **approval / permission mode** | whether the agent may act without asking; "Full access" is the dangerous preset |
| **Fast mode**, **reasoning effort**, **Ultra** | the cost/quality dials, scoped per task |
| **needs input** | a task state that requires the human |
| **activity view** | sidebar view of recently engaged-with chats |
| **appshot** | a screenshot of the frontmost app window sent to Codex |

## 12. What this implies for Orchester

The features Orchester already has map onto Codex surfaces almost one-to-one —
which means the design work is mostly about **anatomy, not invention**:

| Orchester concept | Codex equivalent | Design consequence |
|---|---|---|
| run / run timeline | thread / task | a task list, not just a history list; needs a `needs input` state |
| sessions (delegate history) | task list | group by project; show repo/branch/status |
| approvals | approval prompts + permission presets | a preset control in the composer footer, plus a queue in the right panel |
| validations / todos | plan progress | render as a progress-bearing plan, not a bare list |
| file changes / `ChangeInspector` | Review | right panel with staged/unstaged/branch/last-turn filters |
| agent fleet / `agent-presence` | subagent activity + agents dashboard | stable identicons; group by provider |
| model catalog | ModelPicker + effort layers | composer footer dropdown, scoped to the run |
| memory, governance, plugins | Settings sections | settings with **search** |
| workspace | project (multi-folder, primary folder) | support many roots with one primary |
| `--resume` | resume/reconnect | explicit resume affordance on the task row |

What Orchester does **not** have and should consider, in priority order:

1. **A unified tab strip** in the shell (open transcripts/terminals/diffs side by
   side as tabs).
2. **A bottom panel** in addition to the right panel, with a user preference for
   where terminal tabs open.
3. **A message navigation rail** with scrubbing for long transcripts.
4. **Explicit scroll state** (`data-can-scroll-up/down`) driving a top fade and a
   scroll-to-bottom button.
5. **A command palette with empty and loading states**.
6. **A settings search** and a **keyboard-shortcut editor** with keypress search.
7. **Half-pixel hairlines and the 0.5 px stroke shadow**.
8. **A global corner-radius multiplier** so Orchester can have its own roundness
   without forking every radius.
9. **`data-reduced-motion` as an explicit, overridable root attribute**.

And two things to deliberately **reject**:

- **Do not copy the achromatic inverted primary button.** Orchester's accent
  primary is more legible for a governance-heavy product where "Run" is a
  consequential action; Codex's calm primary works because its primary action is
  "send a message". Keep the accent, but adopt Codex's restraint everywhere else
  (borders, hovers, focus).
- **Do not copy the collapsed text hierarchy** (`-secondary`/`-tertiary` ==
  `-foreground`). Orchester renders governance state — risk, approval, denial —
  where a genuinely subordinate text colour carries meaning.

## 13. Open questions

- The exact `--shadow-alpha-*` values are declared in a block the extractor read
  as `--shadow-100 → var(--elevation-100-geo) rgb(var(--shadow-color) / var(--shadow-alpha-100))`;
  the numeric alphas were not captured. Treat the composed shadows in §3.7 as the
  usable form.
- Sidebar *minimum* behaviour below 240 px of available width is not stated; the
  clamp implies the layout switches to a drawer instead, which matches
  `data-app-shell-responsive-sidebar`.
- Tab-strip contents (which surfaces are tab-able, tab overflow behaviour) are
  only partially recoverable from CSS; the changelog mentions "background agent
  tab restoration" and "Browser tab dragging", so agents and the browser are
  tab-able.