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