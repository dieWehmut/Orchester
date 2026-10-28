# 21. Codex alignment pass: the column, the block and the accent

> The objective this records is "优化 ui 对齐 codex（好好搜索相关参考图）要逐步截图验证":
> the reference set was gathered again, the surfaces were compared against it
> one at a time, and every change below was captured in a real browser before
> the next one started.
>
> The reference images live in `_scratch/refs/` and are not committed: they are
> other people's screenshots. What they showed, and what was done about it, is
> what this file keeps.

## 1. What the reference actually is

Two different surfaces were being conflated in earlier waves, and separating
them is what made this pass possible:

| the picture | what it is | what it is authoritative for |
| --- | --- | --- |
| `ref-g.png` / `ref-g-app.png` | the Codex **desktop app**, cropped from openai.com/codex | the app's own anatomy: sidebar rows, thread header, review pane |
| `ref-d.webp` | the Codex desktop app, **plugin/skills** screen | how the app draws a browse surface and its sidebar groups |
| `ref-c.png` | **chatgpt.com**, not Codex | the centred new-conversation block, and the achromatic send control |

The distinction matters because the third picture is a different product: it
was used for the one thing it demonstrates cleanly — a greeting and a composer
drawn as a single centred block — and not for anything Codex-specific.

## 2. The palette was already right

Measured against `packages/design/src/tokens.css` rather than guessed:

| the reference | Orchester's dark theme | verdict |
| --- | --- | --- |
| page `#0d0d0d` | `--gray-0: #0d0d0d` → `--color-bg-base` | identical |
| surface `#212121` | `--gray-200: #212121` → `--color-bg-surface` | identical |
| secondary `#181818` | `--gray-100: #181818` | identical |
| tertiary `#131313` | `--gray-50: #131313` | identical |
| elevated `#303030` | `--gray-300: #303030` | identical |

No colour was changed. The alignment work in this pass is **anatomy**, which is
also what `01-codex-desktop-app-research.md` §12 predicted it would be.

## 3. The changes, in the order they were made

Each row was captured before the next began; the screenshot names are the
files in `_scratch/shots/` at the time.

### 3.1 The rail is a list of rows, not a toolbar (`a1-rail.png`)

The reference's sidebar opens with plain rows — an icon, a label, no box around
any of them. Orchester had a filled, boxed "新建对话" button with a separate
search icon beside it, which read as a toolbar with one prominent action.

- `AppRail.vue`: `New` and `Search` are now the same row shape, both drawn
  flat; the hover and the current state are the row's own.
- Group headings lost their always-visible chevron: the reference states
  "Threads" as a label, so the chevron is drawn on hover and the text never
  moves.
- Row height 38 → 34 px, radius 10 → 8 px, section padding tightened, so the
  column has one rhythm rather than two.

### 3.2 The send control follows the chosen accent (`a2-composer.png`, `a3-send.png`)

`--composer-send` was a hard-coded `#ed70ad`, which meant the composer ignored
the accent scheme the appearance axis offers — a reader on the blue or teal
scheme still got pink. It now resolves through `--color-action-solid` /
`--color-action-contrast`.

Two smaller things came with it: the field's radius went 28 → 26 px and its
surface moved from a literal `#343434` to `--color-bg-element`, so the field is
the same neutral family as the rest of the stack.

A first attempt also set `opacity: .4` on the disabled send control; that
compounded with `AppButton`'s own `opacity: .5` to 0.2 and the arrow nearly
vanished. The rule was removed — the component already dims itself.

### 3.3 The icon rail joins the column (`a4-nav.png` → `d-rail-rows.png`)

Codex draws **one** sidebar. Orchester drew two columns: a 52 px icon rail and
the list beside it. This was the largest remaining structural difference, and it
is now closed:

- `AppRail.vue` gained the destinations as rows — `定时任务` and `插件` under
  "New thread", plus a footer of `隐藏伙伴` / `主题` / `设置`, which is where
  `ref-d.webp` puts `设置`.
- `WorkspaceSidebar.vue` and `WorkspaceView.vue` pass those rows through.
- `App.vue` stops drawing its strip on the workspace route, because that route
  now carries the destinations itself. Every other route still gets the strip:
  those screens are a page with a list beside them rather than a list that is
  the screen.
- `app.css`: the body became a flex row instead of a hard-coded
  `grid-template-columns: 52px minmax(0, 1fr)`.

Three things went wrong here, and each failed *silently*:

1. **The capture collapsed the workspace to a 52 px sliver.** The two-column grid
   still reserved a track for a strip that was no longer rendered, so the outlet
   — now the only child — landed in the 52 px column.
2. **`:not(:has(> .app-shell__navigation))` did not fix it.** `:has()` cannot
   negate a *descendant* selector the way this needs; the strip kept its 52 px
   while the secondary routes stopped reserving one, so those routes drew the
   icon strip as a horizontal bar across the top of the page.
3. **The flex rewrite still measured 34 px on secondary routes**, because
   `App.vue` rendered `<WorkspaceNavigation>` directly as the body's child and my
   `> .app-shell__navigation` selector matched nothing. Wrapping it in that class
   is what made the width arrive.

The lesson: a shell change like this needs the *measured* box, not just a
screenshot that looks plausible. `_scratch/probe-nav.mjs` reads the nav's own
bounding box and each body's computed track layout on every route, which is what
caught each of the three.

`apps/web/test/single-sidebar.test.ts` now pins both halves: the workspace
renders the rows and no strip, and a secondary route still renders the strip.

### 3.4 Empty groups state one quiet line (`a5-empty.png`)

The rail's empty notes were centred blocks with 2rem of padding, which made
them read as cards inside a list. They are now the column's quietest type,
left-aligned, with no block padding.

### 3.5 The greeting is type, not a hero (`a6-greeting.png`)

The mark was 48 px and the greeting was a `clamp(22px, 2.2vw, 28px)` line at
normal weight. The reference opens a conversation with a small signature and a
line of type at the transcript's own size: 40 px mark, 22 px heading at medium
weight, tertiary description.

### 3.6 The greeting and the field are one block (`a7-block.png`, `a8-centered.png`)

This was the largest visible difference. The greeting was pushed to the top of
the scroller and the composer pinned to the bottom, so a new conversation read
as two unrelated things at opposite ends of a large empty room.

`RunPanel.vue` now states whether it is opening a conversation
(`data-run-empty-conversation`) and, when it is, stops the scroller claiming
the free space and centres the pair as a unit. The scroller keeps its own
behaviour the moment there is anything to read.

### 3.7 Rows carry their age (`SessionListItem.vue`, `d-rail-rows.png`)

The reference puts `4h` on the trailing edge of a thread row. Orchester kept the
time only in hidden text and a tooltip. `src/lib/relative-time.ts` now produces
the reference's own width, and with a seeded session log the rail renders
exactly that ladder:

```
9月18日   10月19日   3 天   1 天   4 小时   25 分
```

The spelled-out sentence stays for the tooltip and the screen reader, and the
date takes over past a week. `Intl.RelativeTimeFormat` cannot shorten that far
(`4 hr. ago` is its shortest English form), which is why the units are spelled
in the module and pinned by `relative-time.test.ts`.

Hovering a row fades the age out and brings the pin in (`f-hover.png`), so the
row states one trailing fact at a time.

### 3.8 The conversation itself (`h-session-open.png`)

Selecting a stored run was the first time this pass looked at a *conversation*
rather than an empty surface, and it showed two things the reference does not do:

- **The reader's own turn was a saturated fill, hard-coded.** `UserMessage.vue`
  carried `background: #843d63; color: #fff` — a literal that ignored the accent
  scheme entirely, so a reader on the blue or teal scheme still got pink. It now
  resolves through `--color-bg-element` / `--color-text-primary`, which is what
  `ref-g.png` actually shows: a soft neutral bubble, not a coloured one.
- **The outcome was a green `SUCCESS` badge.** The reference has no such chip.
  Orchester keeps an outcome because a governed run has one and a failure must
  stay visible, so the fix was proportion rather than removal: a status dot and a
  lowercase word in tertiary type, with `failed` still allowed to be red.

## 4. How this was verified

- `pnpm exec vitest run` in `apps/web`: **722 passed / 120 files**, up from 712
  / 118 at the start of this pass. The new tests are the relative-time
  formatter, the row's trailing age, the rail's destination rows, and
  `single-sidebar.test.ts` (3 tests) for the shell's one-column arrangement.
- `pnpm typecheck`: clean across all seven projects.
- `pnpm stack:verify`: the manifest matches WebUI, website, Tauri and Pages.
- `node werkzeug/desktop/verify-bundle.mjs`: the NSIS contract holds and the web
  payload exists.
- Every surface was captured in a real browser, driven by Playwright against the
  **installed** `orchester-preview` harness — the same `EmbeddedServer` the Tauri
  shell starts, so the screenshots show the real session boundary rather than a
  bypass.
- Surfaces captured, all with **zero console errors**: dark 1440×900
  (`e-workspace.png`), light 1440×900 (`c-light.png`), dark 1024×720
  (`c-narrow.png`), a populated rail (`d-rail-rows.png`, `f-light-rows.png`), a
  row's hover state (`f-hover.png`), a stored conversation
  (`h-session-open.png`), and `/settings`, `/plugins`, `/schedules`
  (`e-*.png`).
- **The installed application was verified, not just built.** The installer was
  rebuilt (`Orchester_0.1.8_x64-setup.exe`, SHA-256
  `C3BB168553D5E6B108A490512EDEEB892F093B2B2E66DC0DFECE7CB1F8BA2EB7`), installed
  over the previous build, and the payload under
  `%LOCALAPPDATA%\Orchester\web` hashes **identically** to `apps/web/dist`. Serving
  that installed directory through the real `EmbeddedServer`
  (`j-installed-payload.png`) shows the new UI: zero navigation strips, the
  seven rail rows in order, and zero console errors.

  A screenshot of the Tauri *window* was not obtainable here: this session's
  desktop is non-interactive, so `CopyFromScreen` fails with "The handle is
  invalid" even though `GetWindowRect` succeeds (1293×808). Serving the installed
  directory is the substitute, and it is the stronger check of the two — it
  proves the bytes on disk, where a window capture would only prove the bytes
  the app happened to load.

### Two failures that were already there

`packages/design/test/component-migration.test.ts` reports 2 failures
(115 hand-picked spacing lengths, 7 literal paints). **These are not from this
work**: stashing every change and re-running the suite at `HEAD` reproduces them
exactly. They gate `pnpm test` because pnpm stops at the first failing package,
which is why the remaining packages were run individually — `ereignis` 28/28,
`website` 31/31, `web` 722/722, plus 26 tooling tests.

### The session boundary, and the trap in it

The desktop app opens with a one-time fragment token in the URL fragment; the
WebUI exchanges it for a session cookie and a CSRF token. Two consequences that
cost time here and are worth writing down:

1. A capture harness that serves `apps/web/dist` **must** start the real
   `EmbeddedServer`, or `/session` answers 401 and the UI shows "连接错误" — a
   defect that does not exist.
2. The token is single-use. A sweep that reuses one preview server across two
   browser sessions captures a 401 on the second. `_scratch/themes.mjs` now
   starts a fresh preview per pass; the earlier 401 in `c-narrow.png` was
   exactly this and was not a product bug.

## 5. What was deliberately not taken

- **The achromatic inverted primary button.** `01-codex-desktop-app-research.md`
  §12 already argues this and the argument still holds: Orchester's primary
  action is a governed run, not "send a message". The accent stays on the
  primary action; what was taken from Codex is the restraint everywhere else.
- **The collapsed text hierarchy.** Codex flattens `-secondary`/`-tertiary` into
  the foreground. Orchester renders risk, approval and denial state, which needs
  the levels.
- **The review pane's diff rows.** `ref-g` shows `2 files changed +9 −6` with
  green and red lines. Orchester's `ReviewPanel` already carries the change set
  and the counts; the row-level tinting is a separate piece of work and was not
  attempted here.

The single-column sidebar is no longer on this list: §3.3 took it.

## 6. What is left

The shell now has the reference's one column. What remains is smaller and mostly
inside the conversation:

- **The review pane's row-level diff tinting** (§5).
- **Codex's `Thought 7s` / `Explored 2 files` summaries.** Orchester's reasoning
  disclosure counts characters because the journal carries no duration; adding
  one is a runtime change, not a styling one.

Neither is a gap in the shell's anatomy, which is what this pass set out to
close.
