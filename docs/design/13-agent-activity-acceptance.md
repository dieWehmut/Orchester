# Agent activity and clean sidebar acceptance

Date: 2026-10-12. Desktop source/build commit: `889d6ed0c8a7a231cf6af82ce7b6ee767ee364f3`.

## Delivered behavior

- The sidebar begins with New chat and search. The product/workspace header and account/runtime footer are removed. Agents use compact rows without vendor subtitles or group/window counters.
- Selecting Codex, Claude Code or OpenCode opens recent local tasks and visible conversation/tool content in the main pane, including tasks launched outside Orchester. Plugins also link to the reader. Orchester-managed runs use the runtime event registry.
- The reader refreshes while visible, keeps the selected task, preserves surviving expanded tool entries, and stops polling on exit. External tasks remain read-only; source applications own continuing/cancelling them.
- Existing navigation order, composer, schedules/plugins management and absence of a bottom panel remain intact.

## Checks

- Rust `orchester-netz` release unit/integration suite: 161 passed. Includes transcript format fixtures, append/truncation, malformed data, symlink boundaries, read-only SQLite, authentication, opaque IDs and managed runtime events.
- Web typecheck and production builds passed. The full 114-file run passed 112 files; the two failures asserted the removed sidebar account menu/artwork. Their corrected suites passed all 9 remaining tests. All 671 applicable web tests are covered by the full run plus that focused rerun.
- Desktop security configuration test passed. OpenAPI YAML parsed with both authenticated activity routes present. `git diff --check` passed.
- A browser preview served the **installed** web directory with the newly compiled embedded server and an isolated Orchester workspace. Provider files were read from their normal local locations; none were edited.
- Real source availability: Codex, Claude and OpenCode all available. The reader listed 24 Codex tasks, 24 Claude tasks and 9 OpenCode tasks; representative details rendered 38, 100 and 2 entries respectively. Counts are a point-in-time observation, not a fixed product guarantee.
- A subsequent request observed one external Codex task update. Explicit before/after DOM checks confirmed an expanded tool stayed open and connected across polling. Selected task retention, plugin-to-Claude navigation, return to composer, and zero further activity requests after exit passed.
- At a 960×640 viewport the reader measured 612×594 with matching client/scroll dimensions. The clean sidebar was also visually inspected at desktop size. Both branding sections were absent and all four agent rows were compact.
- Browser console contained two initial CSP diagnostics (unsupported `navigate-to`, blocked inline script); no additional errors appeared during the activity flows. Authentication and all tested activity responses succeeded with `Cache-Control: no-store`.

## Local installation

The Windows x64 NSIS installer is `Orchester_0.1.8_x64-setup.exe`, 11,535,342 bytes, SHA-256 `74a8acc26d7b3ac42edf6fd761b104edc8496630f1c53ca3d6c03fe56bae42f6`.

The first silent attempt returned success and updated the registry while retaining the old executable. A second invocation of the same installer with an explicit destination succeeded. Acceptance used payload hashes, rather than the installer exit code alone:

- Installed executable and registry both report 0.1.8.
- 50 executable/CLI/web file comparisons passed. The extracted executable matches the release binary after the verified Tauri UNK-to-NSS bundle marker change; no other binary differences were accepted.
- All 24 served web files match the installed payload.
- Three existing configuration/history files retained their hashes; both application shortcuts point to the installation.
- The installed native process responded, exposed its main window handle, served a healthy local runtime, and passed an isolated CLI mock run with five expected events and no external model invocation.

The native Computer Use helper twice returned `failed to activate captured window`, and its capture showed wallpaper. Therefore native visual interaction is **not** claimed: rendering/interaction acceptance used the installed files in the browser, while installed-native version, health and payload checks were performed independently.

Local logs, reports, backup metadata and the clean-sidebar screenshot are under `desktop-installers/agent-live-content-20261011` outside the source worktree. Private provider transcripts and local authentication material are not committed.
