# Classic-mode typography and motion source

The requested local `D:\project\dieWehmut.github.io` path was not accessible in this environment. The user's public repository was available at commit [`89531e0ab8695e6d75c0596a658e3c1ead77093e`](https://github.com/dieWehmut/dieWehmut.github.io/tree/89531e0ab8695e6d75c0596a658e3c1ead77093e).

- Font declaration: `src/styles/fonts/index.scss`.
- Original font: `src/assets/fonts/LXGWWenKai-Regular.ttf`.
- Original SHA-256: `151210456c0e832ea63f17b463b838b783da0d37678bd16b27836c9a9b05d0e3`.
- Bundled WOFF2 SHA-256: `23685af09c203fdf70cfcc15595411162ace2f75120a056134387dc4681b898b`.
- Converted using fontTools WOFF2 compression, without subsetting or changing glyphs. Character map and glyph order verified identical (46467 mapped characters). The original family name and regular 400 weight remain unchanged.
- Font copyright and license are embedded in the font: Copyright 2021–2026 LXGW; Copyright 2020 The Klee Project Authors; SIL Open Font License 1.1. The accompanying [OFL.txt](OFL.txt) comes from [the font project's official license](https://github.com/lxgw/LxgwWenKai/blob/main/OFL.txt).

The font is packaged locally so desktop/offline use never fetches a remote font. `font-display: swap` keeps content readable while loading. Explicit sans/serif appearance choices still override the default; code retains the monospace stack.

Classic-mode motion evidence: `src/layouts/DesktopSidebarLayout.vue` lines 175–176 sets 340ms and cubic-bezier(0.22, 0.61, 0.36, 1). `SiteSidebar.vue` uses 220ms; content rows use 160ms. Orchester maps these to slow/normal/fast tokens and the enter/move curves. Existing reduced-motion preferences remain authoritative. The app does not import blog layout code or its pointer effects.
