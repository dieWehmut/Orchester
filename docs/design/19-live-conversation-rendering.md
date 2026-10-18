# Dynamic conversation rendering

Reference: the user's Codex conversation screenshot, continued on 2026-10-18. Preserve desktop 0.1.8 and CLI 0.1.2.

Use the existing system sans font and dark conversation palette: canvas `#181818`, prose `#f1f1f1`, secondary `#999999`, code `#303030`, user bubble `#843d63`, integration glyph `#17a994`. Light mode uses existing semantic tokens. Keep a centered 48rem reading column, 16px prose, 14px tool summaries, right-aligned user bubbles and left-aligned activity.

```text
                                      [ User prompt ]
                                           17:00 copy
Working / Worked for 6m 8s  v
----------------------------------------------------
Progress prose with live Markdown
  terminal  Ran a command                 >
Further progress prose
  plug      Used Playwright integration   >
Final answer with copy action
                         (down)
```

The latest round starts expanded; earlier rounds start collapsed. Reader choices and nested tool details survive refresh and completion. Known source start/end timestamps determine elapsed time; no timing is invented for legacy records. Pending calls and the active conversation show activity, respecting reduced motion. Source errors pause the activity presentation.

Reuse the existing read-only activity API, refreshing saved source records every two seconds. Changed text updates in place; identical snapshots do not redraw the conversation. This does not imply access to tokens that an external provider has not saved. Managed conversations continue to use their event stream.

Follow new content only while the reader follows the end. Reading back or opening details keeps their position; the latest-output button marks unseen updates and restores following. Managed conversations must also follow changes within a message, not just newly appended rows. Measure changed row heights and observe layout changes without discarding offscreen measurements.

## Implementation and checks

The UI commits are `045e6d4`, `94208d4`, and `17f05b9`. The OpenCode reader correction is `51897e0`: pending/running tools no longer receive a fabricated empty result; completed empty output and legacy records are still supported. Activity endpoints remain read-only and their schema is unchanged.

- Full WebUI suite: 117 files / 698 tests passed before the final reading-position and unfinished-tail refinements.
- Focused WebUI checks after those refinements: 7 files / 58 tests passed, including same-message growth, long tool disclosures, preserved offscreen row heights, active clocks, source refresh, and completion. These overlap the full suite; the counts are not additive.
- Backend activity tests: 16 passed, including nine OpenCode state cases, stable tool identities, read-only source access, and authenticated route contracts.
- Production WebUI type checking and build passed. Desktop remains 0.1.8; CLI remains 0.1.2 and its source is unchanged in this delivery.

## Browser acceptance

The browser used the real local service and changing isolated Codex/Claude JSONL files and OpenCode SQLite records. No provider account or external model was called, and source API responses were not mocked.

Codex updates an existing progress paragraph in place, renders Markdown, follows new output at the bottom, and shows the source elapsed time. While reading at scroll position 120, completing the task retained that position and the open tool's DOM identity, then displayed the unread marker. Jumping to latest restored following within one pixel of the end. Claude also retained open tool details through completion and displayed the result.

After the OpenCode reader fix, the browser showed one pending tool and one running marker. Switching the SQLite record to completed removed both markers, displayed the actual result, and preserved the same open tool DOM node. With reduced motion enabled, both spinner animations computed to `none`. The final browser loaded `index-B9h8k7r3.js`, matching the final production build.

Dark and light themes were inspected. At 420 × 800 the page width remained 420 pixels, the transcript width was 356 pixels, and neither overflowed horizontally. Screenshots and test logs are local in `D:/project/Orchester/desktop-installers/live-conversation-20261018/`; the final OpenCode transition screenshot is `D:/project/Codex/2026-10-18/new-chat/live-opencode-verified.png`.

Two existing browser console messages concern the unsupported CSP `navigate-to` directive and the blocked inline appearance bootstrap. No new functional console errors appeared during these checks. Native desktop clicking remains deferred as requested; installed payload/runtime checks are recorded separately below.

## Installed delivery

The final x64 installer was built from `51897e0223a31b4fd098cb66b6f1d866f90acac0` with the x64 MSVC toolchain, then installed successfully on 2026-10-18. The installer is 11,302,677 bytes; SHA-256: `eb23fdef9c0eb87f139dc1d832845000b7355102a31b12c59320acbc0ae22e86`.

- Packaged executable matched the source build after accounting for Tauri's verified NSIS marker. All 62 payload/installed file checks passed, including the unchanged CLI. Both desktop and Start Menu shortcuts target the installed program.
- The installed desktop executable reports 0.1.8, has a responding Orchester window, serves a healthy loopback API, and returns the application page successfully. All 30 resources actually served by that process match the new production build.
- Installed CLI reports 0.1.2. An isolated local mock run produced all five expected lifecycle/message/result events; no external model was called.
- Three configuration/session files retained identical hashes both after installation and after startup. A private backup is at `C:/Users/30119/AppData/Local/Orchester-backups/live-conversation-20261018-164520/`.
- The task-owned preview process and browser tab were closed. The installed desktop application remains running.

Machine-readable reports (`install-result.json`, `installation-verification.json`, `runtime-verification.json`, `served-payload-verification.json`, and `post-startup-data-verification.json`) are in the local evidence directory above. The final documentation commit contains no additional product changes beyond the installed source commit.
