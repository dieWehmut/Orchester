# Model and reasoning menus

Reference: the user's 2026-10-17 screenshot. Keep the existing product and CLI version numbers.

The composer opens a two-row menu: Model and Reasoning level, each with its current value on the right. The model submenu opens to the left, aligned to the bottom of the parent. Provider headings group real configured models; a right-hand check identifies the active selection. Named profiles retain a secondary label so identical model names remain distinguishable. Settings and retry stay inside the model submenu.

Palette follows the existing theme: canvas `#181818`, menu `#2c2c2c`, hover `#3c3c3c`, border `#404040`, text `#f1f1f1`, secondary `#a8adb2`. Use semantic theme tokens for light mode. Keep the existing system sans font, 15px choices, 13px headings, 12px descriptions. Root width 284px; model submenu up to 484px; 12px corners and 6px padding. Left-align labels; right-align values/checks.

```text
[ Provider                         ]
[ model name                     ✓ ] [ Model       current  > ]
[ model name / profile description ] [ Reasoning   Max      > ]
                                     [ current model Max ^ ]
```

On narrow windows the submenu replaces the parent and offers Back. Keyboard arrows, Escape, Tab, outside click, and focus restoration must work. Long names truncate visually while retaining their full accessible text. Current unknown reasoning values remain selectable. Changing an effort preserves the selected model/provider/profile; changing a model preserves the current effort.

CLI `/model` uses the same two choices, with separate model and reasoning panels, current selection, Back via Escape, and no loss of the composer draft. Direct `/model <profile>` commands remain supported. Add Minimal and Max to match the web choices, preserve unknown configured values, and keep the current choice visible when opening a panel. Validate in a native pseudo-terminal without opening command windows.

Review against the reference: use the screenshot's restrained nested menu rather than a flat settings list. Do not invent provider models or descriptions; use the runtime catalog. Preserve the existing API contract and session selection scope.

## Acceptance — 2026-10-17

- Web implementation: `3813d76`; CLI implementation: `79b99d2`.
- Full WebUI suite: 116 files, 690 tests passed. Production typecheck/build passed.
- CLI: 208 unit tests and all 6 native Windows ConPTY tests passed. The new terminal test switches a profile while retaining High, chooses Max independently, navigates back, resizes to 44 × 14, and checks that exactly one subsequent request carries the selected model and `reasoning.effort = max`. Its local mock endpoint does not establish external-provider support for any particular effort value.
- Browser acceptance uses the real embedded server and isolated configuration: provider grouping, checked state, independent selection, reload persistence, hover/click, keyboard arrows, Escape, Tab, and settings navigation. Component tests also cover unavailable models and stale-catalog retry. Dark/light screens and 420 × 800 compact navigation were checked. The full-width child menu stays left of its parent with an 8px gap and equal bottom edges. Final trigger text is 15px.
- Desktop 0.1.8 and CLI 0.1.2 rebuilt and installed without version changes. The installation contains source revision `79b99d253c66004e7a86b424805c56dd8432b29f`.
- Installer SHA-256: `0639257872a5ad72162f2ee01c449dc19bb7c3947d5faad74b53ce52d1f03f27`. Its executable matches the build after the expected Tauri NSIS bundle marker substitution.
- 62 payload/installed-file comparisons passed; all 30 web resources served by the installed process match the final build. Both shortcuts target the installed executable.
- The installed desktop process responds, has an Orchester main window, and serves a healthy local runtime. The installed CLI completed an isolated mock run with the expected five events.
- Three existing configuration/session files retained their hashes. Backup: `C:/Users/30119/AppData/Local/Orchester-backups/model-menu-cli-20261017-124253`.
- Native desktop clicking remains deferred under the user's earlier “先完成其余交付” instruction. Browser interaction evidence is separate from the installed process/payload evidence. Existing CSP warnings from `navigate-to` and the appearance bootstrap are unchanged.

Local evidence: `D:/project/Orchester/desktop-installers/model-menu-cli-20261017/` contains the installer, checksum, test logs, ConPTY captures, screenshots, backup/install reports, and runtime/payload verification JSON. Main screenshot: `model-menu-desktop-final.png`.
