# Codex workspace polish and Windows 0.1.3 acceptance

Date: 2026-10-01. Base: `12fb994` on `main`.

## Reference and outcome

The locally installed Codex interface was inspected as the visual reference:
a restrained sidebar, a clear conversation surface, fine separators, and a
composer that stays at the bottom of the workspace. The existing Orchester
identity, theme tokens, menus, and native Windows caption controls remain in use.

The previously installed Orchester 0.1.2 window had an overflowing page and an
offscreen composer. The updated shell fills the viewport with a single flexible
workspace, and the conversation scrolls within its available height. The
inspector starts closed and opens on demand; the tab strip appears when there is
more than one tab.

Sidebar width preferences now reach the grid tracks. Displayed widths reserve
at least 360 px for the transcript without overwriting the user's stored widths.
The sidebar remains a column from 800 px upward; the inspector docks from
1120 px upward. Below those thresholds they use controlled drawers, which close
consistently and do not overlap each other.

The conversation measure, empty-state typography, logo size, user-message
surfaces, composer corners, and theme-aware elevation were refined. Empty
conversations no longer show zero usage statistics. The send button stays round
on narrow screens. Long prompts scroll inside a textarea capped at
`min(18rem, 30dvh)` so the action row stays visible.

## Validation

- `pnpm typecheck`: all seven projects passed. The web check was repeated after
  the final textarea change.
- `pnpm test`: 1,128 tests passed: 26 frontend tooling, 89 protokoll, 301 design,
  28 ereignis, 31 website, 643 web, 1 desktop security, and 9 desktop tooling.
- The final composer check passed all 4 tests after its CSS height limit changed.
- `pnpm stack:verify`, both frontend production builds, and
  `node werkzeug/desktop/verify-bundle.mjs` passed.
- Windows release tests passed with the locked MSVC target: 3 desktop shell tests
  and 3 native caption tests. The unchanged root Rust workspace was not retested.
- Browser acceptance covered 1280x720, 1280x800, 960x640, 390x844, and 320x640;
  light and dark themes; folded and expanded sidebar; docked and drawer inspector;
  and a 30-line prompt. At 320x640 the final composer ended at 587 px and the send
  button at 578 px, inside the 595 px transcript boundary.

## Build and installation

`build/windows-ui-0.1.3` contains the UI work from
`feat/codex-workspace-polish` and the desktop version update. The final installer
was rebuilt after the last composer change with:

```text
node werkzeug/desktop/build-installer.mjs --arch x64 --rust-target x86_64-pc-windows-msvc --version 0.1.3
```

The per-user installer completed with exit code 0. The registry and installed
executable both report 0.1.3. Its executable hash matches the executable extracted
from the NSIS installer, all 17 bundled web files match the final frontend build,
and the Desktop and Start-menu shortcuts target the installed executable.
The previous installation payload was backed up before upgrading.

The installed application opened with the new layout and live runtime discovery.
Opening the inspector, selecting its tab, and closing it with Ctrl+W preserved
the conversation window. The current local setup still reports an unavailable
model; no provider credentials were changed and no real model task was submitted.

Installer: `Orchester_0.1.3_x64-setup.exe`, 11,145,526 bytes.

SHA-256:
`bd41fa9f8e7a174b459e0ea39ba1bc68de99061d6b1addf9360750854b84bfe4`.

The local installation verified here is Windows x64. An ARM64 installer and a
public GitHub Release are outside this local build acceptance.
