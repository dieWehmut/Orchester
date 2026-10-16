# Agent conversation navigation acceptance

Checked on 2026-10-16. Application source/build commit: `639720892e72aaa49663820655d400ed9554187c`.
Desktop version remains **0.1.8**.

## Delivered behavior

- Codex, Claude Code and OpenCode share a single provider sidebar and conversation pane. The activity menu contains only those three providers; Orchester is absent. The separate provider tab strip and task preview cards have been removed.
- New chat, pinned tasks, collapsible projects and recent tasks use compact rows. Selection, running status and unread updates are visible without preview paragraphs or completed-date badges. Pin overrides, project expansion and read markers persist locally. Search, keyboard navigation and narrow-window drawers work.
- All three providers use the same right-aligned user bubbles, plain assistant prose, date separators, work disclosures, lists, tables and code styling. Unknown source durations remain unknown. Tool results are paired to their calls, including interleaved calls and rewritten Claude records.
- Refresh retains the selected task, expanded tools and the reader's scroll position. Readers at the bottom follow incoming content; readers inspecting older output can jump back to the latest content.
- Codex's saved display title, optional project identity and source pin hints are read from its local index. The first meaningful prompt is the fallback title; a later “continue” prompt does not replace it. Index paths cannot escape the configured transcript root.
- Local Markdown file references render as labelled, copyable paths. They do not navigate to or execute a local file. Clipboard failure leaves the path selectable.

## Browser validation

- Production assets were served by a real embedded server with isolated Orchester workspace data. The browser read actual external records: 2 Codex, 24 Claude and 9 OpenCode tasks. No external model was called.
- Inspected 1440×1000 desktop and 640×760 narrow layouts. The desktop sidebar measured 287 CSS pixels. Both themes displayed the correct official provider marks, with no horizontal page overflow.
- Verified provider menu selection, End/Escape navigation and trigger-focus restoration, pins surviving reload, project expansion, search/clear, switching tasks, New chat returning to the composer, and the narrow drawer closing on task selection.
- Reading position stayed at 100 pixels across refresh. Both expanded tool output and a deliberately collapsed active work section retained their state across refresh.
- A clearly labelled synthetic browser fixture exercised completed turn duration (21m 59s), a table, a list, inline code and a local file reference in both themes. The copy-handler check used a stubbed clipboard method and received the exact expected path. The fixture interception was removed afterward; no provider files were changed.
- Two existing CSP diagnostics remain: unsupported `navigate-to` and the blocked inline appearance bootstrap. No new application exception was observed.

## Automated checks

- Backend activity suite: **15 passed**, including metadata boundaries, title refresh, source pin hints, call identity, rewritten tools, partial writes and authenticated API routes.
- Shared Markdown suite: **25 passed**, including local path parsing, refused executable links, copying and clipboard failure fallback. Design-package typecheck passed.
- Final complete WebUI suite: **116 files / 689 tests passed**. The four affected suites also passed 34 focused checks after correcting the previous sidebar expectation and the provider-name template. Production typecheck/build passed.

## Boundaries

External tasks are read-only. New chat returns to Orchester's existing composer; it does not remotely resume a source application's task. Local pin/read preferences never modify source application data. The existing backend limit remains 24 tasks per provider and 100 visible entries per task, with bounded transcript reads and an explicit partial-history hint. Cloud-only and other-machine records are not imported. The Codex index on this machine contained older titles whose transcript files no longer existed; only the two existing local transcripts were shown.

## Local installation

- The Windows x64 release and CLI builds succeeded. The 11,681,556-byte `Orchester_0.1.8_x64-setup.exe` installed successfully. SHA-256: `7afd1cd3d38860a021d0744ef06106c8fcc54a08b13eb57468d602330395c967`.
- The installed executable and Windows registration both report **0.1.8**. Core/CLI remains **0.1.2**. No version number changed.
- All 62 executable, CLI and WebUI payload comparisons passed. The extracted executable matches the release build with only the verified Tauri NSIS marker change. All 30 assets served by the installed application match the production build.
- Three existing configuration/history files retained their hashes. Desktop and Start menu shortcuts both point to the installed executable. The previous app, CLI and data were backed up before installation.
- The installed native application launched with a responding window titled `Orchester`. Its health endpoint returned `ok` and its main page returned HTTP 200. An isolated CLI smoke run emitted the expected five lifecycle events without invoking an external model.
- The installed window's accessibility tree exposed the updated Recent navigation and composer. The screenshot showed a Windows lock screen; native clicking was paused pending unlock. Browser interaction verification and installed-runtime/payload verification are therefore recorded separately.

Evidence is kept outside Git in `D:/project/Orchester/desktop-installers/agent-navigation-20261016`; provider transcripts, authentication tokens and user configuration are not committed.
