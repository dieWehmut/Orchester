# Queued messages and complete local history

Follow the supplied Codex reference: system sans, 16px prose, 12px time labels, canvas #181818, text #f1f1f1, muted #999999, bubble #843d63, accent #f472b6. User bubbles align right with time, copy and pencil controls below; assistant Markdown stays left in the same 48rem reading column. A compact queue sits above the composer. Earlier messages load above the conversation without moving the reader.

```text
                         [ user message ]
                         16:37 copy edit
Earlier messages >
---------------------------------------
Assistant Markdown and tool activity

Queued (2)       message       edit remove
[ next message                    queue ]
```

Sending transfers the draft and attachments to a FIFO queue immediately. Exactly one request runs at a time. Successful completion starts the next; cancellation, failure and pause preserve pending items for explicit continuation. Failed starts retain their retry identity. Editing a sent message prepares a new draft and never rewrites an external provider's history. Prior managed replies remain visible across queued sends.

External activity must expose older sessions and older transcript pages through the existing authenticated, read-only API. Each page remains bounded; a continuation makes content beyond the old 24-session/100-entry limits reachable. Live tail refresh must not discard loaded history, duplicate entries or reset open tools. Paths remain server-owned and symlinks are not followed. Unavailable remote/cloud content must not be fabricated.

Verify queue ordering, fast completion, retry, cancellation, draft clearing, edit/copy/time controls, pagination beyond both previous limits, preservation on refresh, rendered browser interaction, and installed payload/runtime. Keep desktop 0.1.8 and CLI 0.1.2; commit slices and push after final acceptance.

## Acceptance, 2026-10-19

Implemented slices: `11c1601` (compressed Codex source and history API), `fb366b1` (queue, message controls and history UI), `9c40ec3` (desktop dependency lock). Desktop remains 0.1.8; CLI remains 0.1.2.

### Automated checks

- Web suite: 118 files / 711 tests passed. After adding the hidden-tab catch-up regression and removing normal connection/sequence statistics, the final affected suites passed 30 tests. The targeted queue/composer/history run also passed 30 tests. These overlapping runs are not added together.
- Final web typecheck and production build passed. The packaged web build matched the checked browser assets.
- Backend activity checks: 18 passed, including authentication, invalid cursors, source isolation, stable IDs across compression, complete 253-entry history over three pages, 238 visible compressed sessions, source timestamps, and unchanged input files.

### Browser interaction

An isolated runtime and a local model fixture exercised the real run routes and event stream, without calling an external model or using user credentials.

- Sending immediately cleared the draft. Later drafts remained editable while the first request ran; only one model request was active. Completion started the next queued message automatically and kept earlier replies visible.
- Queue editing returned the exact text to the composer; removal prevented submission. Runtime failure and cancellation preserved waiting messages and paused the queue until explicit continuation.
- User bubbles show the source time and copy/pencil controls below the pink bubble. Copy passed the exact message to the browser clipboard API. Editing focused the composer and did not send. System clipboard persistence outside the browser was not established by this environment.
- A 76-session fixture exposed rows beyond the initial 50 and opened the oldest compressed session. Its 255-entry conversation loaded over three pages, preserving the reading position.
- After all earlier pages were loaded, appending 205 new messages exercised catch-up across multiple missed windows. All 205 appeared exactly once, the earliest original user message remained present, and scrollTop stayed at 80.
- Dark 1440 x 1000 and light 920 x 800 layouts passed visual inspection; no horizontal overflow. Normal completion has no connection banner or sequence/token statistics strip.
- Browser console contained the pre-existing unsupported `navigate-to` directive and blocked inline-script policy diagnostics. No application exception was observed, and CSP was not weakened.

### Real local Codex source

Read-only audit matched all 1,110 non-archived top-level threads in the local index: 1,109 compressed transcripts and one plain transcript. All 1,110 appeared in the API list. Two compressed-source checks returned 24 and 182 entries, with the latter spanning two pages. Both source hashes stayed unchanged. Initial list loading took about 5.5 seconds; a warm list took 619 ms.

Only locally persisted provider content is available. Archived/child tasks follow the main app's visible-thread filtering. The queue is held for the current application session, including route changes; it is not durable across an application restart. Extremely large individual records remain bounded with explicit truncation/error reporting.

### Installation

Built the x64 NSIS installer from the verified source and installed it over the existing local application. The packaged executable matched the release executable except for the verified Tauri NSIS marker.

- Installer exit code 0; 62 file checks passed against the extracted payload and installed files; both shortcuts target the installed executable.
- Installed desktop version and uninstall registry version both remain 0.1.8. The running process is the installed executable, has an Orchester main window, responds normally, serves its page with HTTP 200, and returns healthy local runtime status.
- All 30 web files served by the installed process match the checked build hashes.
- CLI remains `orchester 0.1.2`; its binary matches the existing release binary. An isolated Mock smoke run emitted five expected events and called no external model.
- Three configuration/session files retained their SHA-256 hashes both after installation and after startup.
- Backup: `C:/Users/30119/AppData/Local/Orchester-backups/queued-messages-20261019-134750`. Existing application, CLI and user configuration/session files were preserved before replacement; backup ACL is restricted to the user, Administrators and SYSTEM.

### Evidence

Local evidence directory: `D:/project/Orchester/desktop-installers/queued-messages-20261018`.

- `web-tests-verbose.log`, `web-final-regression.log`, `web-targeted-final.log`, `web-build-final.log`, `backend-route-final.log`
- `source-history-verification.json`
- `queue-dark-final.png`, `conversation-dark-final.png`, `conversation-light-narrow-final.png`
- `desktop-build.log`
- `bundle-verification.json`, `installation-verification.json`, `runtime-verification.json`, `served-payload-verification.json`, `post-start-data-verification.json`

Native desktop clicks remain deferred as requested. Installed executable, payload, served assets, runtime health and preserved data are checked separately from browser interaction.
