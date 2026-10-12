# Native conversation rendering

The agent reader should read like the source conversation. The supplied Codex screenshot is the visual contract: a right-aligned user bubble, unboxed assistant prose, a collapsed work disclosure above the answer, Markdown lists and inline code, and a quiet copy action below it.

The conversation uses a centered 48rem reading measure, 16px text with a 1.75 line height, and generous separation between turns. The Codex dark colors are #181818 (surface), #303030 (code), and #843d63 (user bubble). Text colors, interface fonts and light mode follow existing application settings. Claude Code and OpenCode keep neutral prompt surfaces and tool disclosures; all providers use the existing safe Markdown renderer rather than escaped Markdown paragraphs.

## Data contract

The existing authenticated, read-only activity endpoints retain schema version 1. Entries optionally include `timestamp` (Unix milliseconds), `phase` (`commentary` or `final`), and an opaque `turn_id`. Detail responses optionally include `turns`, containing opaque IDs and known `started_at` / `completed_at` times. Missing metadata remains missing; the client must not invent elapsed time or claim a task completed from an assistant message alone.

Only provider-visible message metadata is used. Codex lifecycle events supply turn timing and message phase; Claude and OpenCode supply timestamps and explicit completion phases where available. Hidden reasoning remains excluded. Old servers and partial transcript excerpts continue to render without metadata. Turn metadata is bounded to turns referenced by the retained entries.

## Interaction and acceptance

- User messages, work records and final answers appear in source order. Consecutive prompts are retained.
- Known commentary and tool calls appear inside a keyboard-operable work disclosure. The latest visible progress remains readable while work is active. Final answers remain outside the disclosure.
- The elapsed label appears only for a valid source start/end pair. Incomplete excerpts use a generic work label.
- Markdown lists, headings, tables, code and safe external links render using shared primitives. Raw HTML is inert. Codex citation bookkeeping is excluded from presentation without changing copied message content.
- Copy actions copy the original answer. Disclosure state and scroll position survive polling; switching tasks resets per-task presentation state.
- Validate parsing metadata, grouping with partial/legacy data, rendered Markdown, update stability, responsive overflow and dark/light presentation before packaging and local installation.

## Verified delivery, 2026-10-12

The installed desktop retains version **0.1.8**, as requested. Its build source is `ffbbecd6eaa80a852999badfebbc8b22aefc0e1a`.

- The focused web suites passed 31 tests across five files; the backend release library suite passed 44 tests; desktop security configuration passed. Typechecking, production builds, OpenAPI parsing and `git diff --check` passed.
- Production-asset browser checks reproduced the supplied Codex example: right-aligned prompt, `21m 59s` work disclosure, three Markdown bullets, bold text, inline code and copy action. Work/tool disclosures stayed open and connected across polling. Task selection returned focus to the picker, and Codex, Claude and OpenCode all rendered formatted answers.
- Dark/light colors, a 960×640 viewport, long code overflow, tables, safe external links and inert raw HTML were checked. Existing CSP diagnostics remained; these checks do not claim an error-free console. Clipboard behavior was checked through its handler and unit coverage, without claiming a system-clipboard round trip.
- An authenticated preview using the current backend and installed web directory read real local provider data. Codex and Claude each returned 24 recent tasks; OpenCode returned nine. Three details per provider passed schema checks. Every sampled entry had a source timestamp; completed Codex turns supplied timing metadata and explicit final phases were retained. These counts are point-in-time observations.
- The x64 NSIS installer is 11,548,472 bytes, SHA-256 `2487ec91c93f4fb69e7fcbb71f93d1fff219b69277efdeb673075d88f6e3d79a`. The extracted executable matches the new release binary with only its verified Tauri bundle marker difference.
- Installation passed 50 executable/CLI/web file comparisons, preserved three configuration/history files and both shortcuts, and reports desktop/registry version 0.1.8. All 24 assets served by the installed native runtime match the production build.
- The installed native process responded with a visible main window handle and healthy loopback service. The installed CLI passed an isolated mock run with five expected events and no external model invocation; its existing version remains 0.1.2.

Browser visual checks used the production frontend with controlled reference conversations. A later browser automation attempt timed out, so a fresh installed-native visual inspection is not claimed. Installed binary/resource checks, native runtime checks and real-provider API checks were completed independently. External tasks remain read-only.

Local screenshots and test logs are under `desktop-installers/conversation-rendering-20261012`; final installation and runtime reports are in its `version-preserved` subdirectory, outside the source worktree. No private transcripts or authentication material are committed.
