# Reference alignment: what was taken, and what was not

> The objective this records is "仿照 ChatGPT 的进行 UI 改造": four screenshots of
> the ChatGPT/Codex desktop app were supplied — the sidebar and a conversation,
> the account menu, and the appearance screen in its dark and light themes — and
> the surfaces below were rebuilt against them, wave by wave, in
> [`03-ui-implementation-plan.md`](./03-ui-implementation-plan.md).
>
> This file is the one-page answer to "how much of the reference is in the
> product, and where is it not?". Every claim names the file that carries it and
> the test that pins it. The waves are the record of *how*; this is the record of
> *what*.

## 1. The rail (reference: sidebar + account menu)

| the reference | Orchester | evidence |
| --- | --- | --- |
| Product row with its own disclosure | Taken: the row folds every list it governs | `AppRail.vue`, `apps/web/test/app-rail.test.ts` |
| Search and bell on that row | Taken: search narrows the column, the bell carries a count | `AppRail.vue`, `session-rail-filter.test.ts` |
| One new-chat row, not a boxed button | Taken | `app-rail.test.ts` |
| A list the reader pinned, first | Taken: pins are local, newest first, one row per run | `use-pinned-sessions.ts`, `PinnedSessions.vue`, `pinned-sessions.test.ts` |
| Headed lists that fold from the heading | Taken | `app-rail.test.ts` |
| One line per entry | Taken: title, time and the outcome's dot; agent and model are a tooltip and hidden text | `SessionListItem.vue`, `session-list-item.test.ts` |
| Account row that opens a menu | Taken; the standing gear was removed | `AppRail.vue`, `app-rail.test.ts`, `workspace-rail.test.ts` |
| Menu headed by the identity, chords on rows | Taken: avatar, name, runtime line, `Ctrl+,` read from the live registry | `AppMenu.vue`, `app-menu.test.ts`, `app-rail.test.ts` |
| Remaining usage, invites, sign-out | **Not taken**: there is no account here to meter, invite to or sign out of | `03-ui-implementation-plan.md` U11, U19 |
| A projects list | **Not taken as a list**: one workspace, named on the product row | `WorkspaceSidebar.vue`; U26 |

## 2. The conversation (reference: the chat view)

| the reference | Orchester | evidence |
| --- | --- | --- |
| Reader's turn as a bubble at the inline-end | Taken, and the journal now carries that turn | `RunTimeline.vue`, `kisten/netz/src/run.rs`, `chat-message-anatomy.test.ts`, `model_routes`/`run_routes` |
| Answer as prose, not a card | Taken; the run's own record keeps its card and its sequence number | `RunTimeline.vue`, `chat-message-anatomy.test.ts` |
| Reasoning behind a disclosure | Taken (the reference's "思考了 34s" needs a duration the journal does not carry; this counts characters instead) | `ReasoningDisclosure.vue`, `reasoning-disclosure.test.ts` |
| Markdown: headings, lists, code, inline code, links, tables | Taken, from tokens rather than markup, rendered as it arrives | `markdown.ts`, `MarkdownText.vue`, `markdown.test.ts`, `markdown-text.test.ts` |
| An action row under a message | Taken, with the two actions the product can answer: copy, and the reader's next move (quote / use the prompt again) | `MessageActions.vue`, `composer-quote.test.ts` |
| A round jump-to-latest control | Taken, with the count on its corner | `RunPanel.vue`, `run-panel.test.ts` |
| A rail that navigates the turns | Taken: hover labels, and a drag that scrubs | `MessageRail.vue`, `message-rail.test.ts` |
| Regenerate and feedback | **Not taken**: the runtime cannot re-run a turn in place and keeps no feedback channel; putting the prompt back is the honest half | `03-ui-implementation-plan.md` U16 |
| Images and raw HTML in an answer | **Not taken**: markup is never interpolated, and an image is a fetch the product has not agreed to | U15, U23 |

## 3. The composer (reference: the field under the conversation)

| the reference | Orchester | evidence |
| --- | --- | --- |
| One rounded field that owns the box | Taken: the heading and the nested box are gone | `RunComposer.vue`, `composer-field-anatomy.test.ts` |
| Named by its placeholder | Taken; the words moved to the accessible name | same |
| A round send that points the way it goes | Taken; a square stop while a run is in flight | same |
| Model and effort chosen at the trailing edge | Taken, and it is a real choice now: `PUT /models/selection` stores it, applies it to every run, and answers with the catalog it produced | `ModelContextControl.vue`, `kisten/netz/src/model_selection.rs`, `model-picker.test.ts`, `model_routes.rs` |
| A microphone | **Not taken**: there is no voice input to offer | U22 |
| A free-text model or effort | **Not taken**: the picker offers what the runtime reports, because choosing is what this surface does | U22 |

## 4. The appearance screen (reference: dark and light)

| the reference | Orchester | evidence |
| --- | --- | --- |
| Theme cards with a ring on the chosen one | Taken; the corner tick the reference does not draw was removed | `ThemePreviewCard.vue`, `theme-preview-card.test.ts` |
| A two-pane preview of the theme in code | Taken, with the tinted rails and the bare head the reference draws | `CodePreview.vue`, `code-preview.test.ts` |
| One card per theme: hue, background, foreground | Taken, and each theme carries its **own** hue | `SettingsView.vue`, `theme-colours.ts`, `appearance-theme-sections.test.ts`, `appearance-scheme-per-mode.test.ts` |
| Import and copy-theme on the card, an `Aa` swatch, the theme's name | Taken | same |
| A separate personalization destination | Taken: fonts, motion, intensity and the rail's translucency live there, as the reference's own list has them | `settings-search.ts`, `settings-view.test.ts` |
| Hex pickers for background and foreground | **Not taken**: the rows report what the theme resolves to; a picker there would be a second theme editor | `SettingsView.vue` U10 |

## 5. The one open decision

**Should the model choice be remembered across restarts?** The selection lives on
the runtime for as long as it runs and is never written to `orchester.jsonc`; the
picker says so in its own menu (`run.selectionScope`). Persisting it is possible
and safe in the mechanics — `ConfigLoader::edit_user_config` splices members,
preserves comments and keeps a `.bak` — but rewriting the file a human maintains
is a decision to take deliberately rather than as a side effect of a click. It is
recorded here as a candidate, not as a defect.

## 6. How this was verified

- `pnpm typecheck` — clean across all seven projects.
- `pnpm test` — every package and every tooling suite, run in one pass as the
  root script does it: 26 tooling, 89 protokoll, 300 design, 28 ereignis, 31
  website, 605 web, 1 desktop-security and 9 desktop tooling tests, no failures.
  The build both frontends produce and `pnpm stack:verify` both pass.
- Rust: `cargo test -p orchester-protokoll -p orchester-netz` — the two crates
  this work changed, including the routes it added (`model_routes`,
  `run_routes`). `cargo check --workspace --all-targets` is clean, which is what
  proves the one wire-vocabulary change did not leave an unhandled match
  anywhere else in the workspace.
- Each visual change was looked at in a real browser (headless Chromium against a
  dev server) before it was committed: the rail, the account menu, the appearance
  screen in both themes, the message anatomy, the composer, the picker, the
  table, the session rows and the pinned list. The harnesses were temporary and
  were deleted; the measurements and observations that mattered are recorded in
  the wave notes.

### What could not be run here

`cargo test --workspace` does not complete on this host: the drive that holds the
checkout ran out of space while linking the workspace's test binaries
(`link.exe` exit 1140, then `rustc-LLVM ERROR: IO failure on output stream: no
space on device`). The two commands that matter for confidence in this work —
the affected crates' own tests, and a compile of every target in the workspace —
both ran and passed, and the crates whose tests were skipped are ones this work
did not touch. For anyone with the disk for it, `cargo test --workspace` is the
command that would close the gap.

## 7. A note on the wave numbers

Wave **U17** does not exist: the numbering skipped from U16 to U18 in the rail
work. It is a slip in this plan's numbering rather than a lost wave — every wave
between U9 and U26 is recorded above.

## 8. Since this page was written

The tables above are the alignment as of wave U26. The waves after it are in
[`03-ui-implementation-plan.md`](./03-ui-implementation-plan.md), and they answer
to the *second* reference set (the home page and a working conversation):

- **U27** — there is no right column at all; the run's surfaces moved into the
  bottom panel, which the view now controls.
- **U28** — the file explorer reads in the interface font at its normal weight.
- **U29/U32** — a transcript marks where it crossed midnight, and each answer
  states the interval it took, at the answer.
- **U30** — a run in flight is stated above the field, with its title and a live
  clock; the footer is a ledger again.
- **U31** — an empty page is the greeting and the field as one group, and the
  companion waits for a run to react to.
- **U33** — the rail files each run under the project it ran in, which is the
  name of the directory the runtime recorded for it.
- **U34** — the rail says less: a pinned list with nothing pinned is a heading,
  and the product row is one line again.

The CLI has its own record in [`05-cli-transcript.md`](./05-cli-transcript.md).

## 9. The main page, against the third reference

The operator's last reference was the conversation page as a whole, with the note
"主页面应该长这样". Its waves are U35–U39; this is what they took and what they
left, in the same shape as the tables above.

| the reference draws | Orchester | evidence |
| --- | --- | --- |
| A code block as a card: `</> <language>` and wrap/copy in its head | Taken: `MarkdownCodeCard`, wrapping per block, copy with a confirmation | `MarkdownCodeCard.vue`, `markdown-code-card.test.ts` |
| A fence with no language says `纯文本` | Taken: the catalogue's own words for it | `markdown-text.test.ts` |
| A row of controls under a message, with `…` for the rest | Taken: copy, run-this-again, and a menu holding quote/reuse | `MessageActions.vue`, `message-actions.test.ts` |
| The row stays up on the answer in front of the reader | Taken: `run-timeline__item--current`; the browser read back `opacity: 1` against `0` for the earlier rows | `chat-message-anatomy.test.ts` |
| Regenerate (`↻`) | **Taken as a different thing**: this runtime starts a run for the question instead of replacing the turn, so it is labelled "run this question again" | `composer-quote.test.ts` |
| Thumbs up and down | **Not taken**: there is no feedback channel to carry a verdict | U36 |
| A deep, muted bubble for the reader's own turn | Taken: a pair per hue with white text, checked at 4.5:1 or better in all eight scheme blocks | `tokens.css`, `tokens-contrast.test.ts` |
| `GPT-5.6 Sol 中 ⌄` at the field's trailing edge | Taken: the model, the provider, and the effort in the interface's own words | `ModelContextControl.vue`, `model-picker.test.ts` |
| A share control in the header | Taken **as a copy**, which is what it can honestly do here - and it had been emitting into nothing until U39 | `conversation-markdown.ts`, `workspace-thread-bar.test.ts` |
| `+`, the microphone and the voice button | **Not taken**: no attachment path to send, no voice input to take | U22 |
| A title in the header | **Left alone**: the reference shows a title in one screenshot and none in the next, and guessing which state hides it is not a change worth making blind | - |
| Projects with no conversations yet (`暂无项目聊天`) | **Not taken**: this runtime knows the projects that have run, not the ones that exist | U33 |
| A usage badge on the account row | **Not taken**: there is no account to meter | U11 |

### How the third reference was verified

- `pnpm typecheck` clean; the frontend suites (the last full pass read 115 files
  and 650 tests, and an earlier run under heavy load reported fewer, which is why
  it was repeated); both builds; `pnpm stack:verify`; and the Rust suites for the
  crates the session payload passes through.
- Every visual change was looked at in a browser against a fixture, and the
  numbers that mattered were read back rather than eyeballed: the code card's
  `pre`/`pre-wrap` per block, the action row's `opacity` per row, the bubble's
  fill and its measured contrast in four theme/hue/volume combinations, and the
  composer's readout in two languages.
- Two of these waves found something other than a missing feature: the bubble was
  borrowing the *action* pair, so the intensity axis could repoint it, and the
  share control was dead. Both are recorded in the plan with what found them,
  because the browser measurement is what did.