# Composer additions and official provider marks

Implementation checks: 2026-10-15. Local installation: 2026-10-16.
Application source/build commit: `963af5464f919080d9806d28c25127059522f2fd`.
Desktop version remains **0.1.8** at the user's request.

## Delivered behavior

- The plus button opens a grouped, scrollable panel above the entire composer. It matches the composer width, stays within the viewport, supports arrow/Home/End keys, and restores focus on Escape. Opening or closing it preserves the draft.
- Files and folders use the actual pickers. Supported text files become removable attachments with relative folder paths. Limits are 20 attachments, 64 KiB per file, and 28,000 combined prompt characters. Binary images are not accepted by this text-only runtime.
- Workspace attachments contain branch and file-status summaries. Large repositories retain whole filenames within a bounded summary and explicitly report omitted paths; the attachment does not claim to contain file contents.
- A workspace goal can be saved, edited, paused, resumed, completed, or removed. Active goals accompany each new interactive request. Plan mode is also saved per workspace and asks the existing read-only executor to inspect and plan. These are persistent request context; they do not create an autonomous background goal runner or a new permission sandbox.
- Drawing supports pen, line, arrow, rectangle, text, color, undo/redo and clear. The visible thumbnail and submitted attachment are generated from the same SVG. Attachment rejection keeps the editor open so the sketch is not lost.
- MCP server definitions support stdio and Streamable HTTP. Users can add, edit, remove, connect, browse tools/resources/prompts, explicitly invoke/read an item, inspect its result, and attach it to the draft. Connections are opened for explicit operations and closed afterward; tools are not invoked automatically by the model.
- MCP credentials reference host environment variable names instead of storing plaintext secrets. Stdio launches directly with Windows console suppression, bounded messages and deadlines. A failed tool call is never automatically retried because the remote operation may already have run.
- Installed Codex/Claude/OpenCode adapter entries can attach existing external task context. The plugin management entry opens the existing management page.
- Provider marks use official local/vendor assets with light and dark variants. Their origins are recorded in `apps/web/public/agents/README.md`; unknown providers retain the generic icon.
- Approval choices include explanatory text and selected-state semantics. Existing permission handling and full-access confirmation remain in place.
- Initial routing now finishes before exchanging the one-time login token, preventing the router from restoring a consumed token and breaking refresh.

## Validation

- Full WebUI suite: **116 files, 685 tests passed**. The final workspace-summary fix passed all 7 composer additions tests, including the added large-repository case. Production typecheck/build passed. The shared AppMenu suite passed 7 tests.
- Backend checks: 47 `orchester-netz` library tests, 3 composer/MCP route tests including the explicitly enabled real stdio fixture, and 23 run/idempotency/static route tests passed. Cases cover persistence, stale revisions, workspace isolation, MCP handshakes, pagination, SSE, resources/prompts, and invocation.
- A real embedded server served production assets with an isolated workspace. Browser checks exercised saved/paused goals across reload and server restart, persisted plan mode, drawing/undo/redo/preview, real file and directory pickers, and a real local MCP echo process. The actual result was attached without losing the draft.
- Desktop-size menu and composer widths both measured 768 px, with an 8 px gap. The menu remained within a 640×760 viewport. Dark and light provider images loaded correctly; light mode used `codex-light.png` and `opencode-light.svg`.
- End/Escape navigation, focus restoration, outside-click closing, approval selection, and navigation to plugin management were exercised in the browser. Fresh launch removed the login fragment, and authenticated refresh succeeded.
- No external model was called during acceptance. Two existing CSP diagnostics remain: unsupported `navigate-to` and the blocked inline appearance bootstrap. Native visual automation is unavailable; browser rendering checks and installed-native payload/health checks are recorded separately.

## Local installation

- The final Windows x64 release and CLI builds succeeded. The 11,735,191-byte `Orchester_0.1.8_x64-setup.exe` installed successfully on 2026-10-16; its SHA-256 is `44315a4871debb0c0a32cd5a7fe192598862bf8a007377deffa5561f762a5540`.
- The installed desktop executable and Windows registration both report **0.1.8**. The core/CLI version remains **0.1.2**; no version number was changed.
- All 62 executable, CLI and WebUI payload comparisons passed. The extracted installer executable matches the freshly built executable except for the verified Tauri installer marker. All 30 assets served by the installed application match the production WebUI build.
- Three existing configuration/history files retained their hashes. Both desktop and Start menu shortcuts point to the installed executable. The previous application and data were backed up before installation.
- The installed application launched with a responding native window titled `Orchester`. Its health endpoint returned `ok`, and its main page returned HTTP 200. Composer and MCP routes rejected unauthenticated requests with HTTP 401.
- An isolated CLI smoke run emitted the expected five lifecycle events without calling an external model. The installed application was left running for the user.

Local evidence is kept outside the source worktree in `D:/project/Orchester/desktop-installers/composer-20261015`, including final dark/light screenshots, the full WebUI test log, build/install logs and verification reports. Authentication tokens, user configuration and provider transcripts are not committed.
