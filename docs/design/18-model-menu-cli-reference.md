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
