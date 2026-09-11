# apps — the Orchester frontends

Orchester has one runtime and several faces. This directory holds the JavaScript
half of them.

| Package | What it is |
| --- | --- |
| `protokoll` | `@orchester/protokoll` — the TypeScript mirror of the Rust wire protocol |
| `design` | `@orchester/design` — design tokens and primitives shared by every face |
| `ereignis` | `@orchester/ereignis` — the components that render an agent run |
| `web` | the local WebUI served by `orchester web` |
| `website` | the project site published to GitHub Pages |
| `desktop` | the Tauri shell, which loads the `web` bundle |

## Why three shared packages instead of one app

The WebUI, the desktop shell and the marketing site all display the same thing: a
stream of `Event` values coming out of a run. If each of them owned its own copy
of the event union and its own tool-call card, the first change to the Rust
protocol would leave three places to fix and no way to tell which one had been
missed. So the union lives in `protokoll`, the rendering lives in `ereignis`, and
the site's scripted demo replays *real* `Event` objects through the *real*
components — which means a demo that has drifted from the product cannot compile.

## Source-only packages

`protokoll`, `design` and `ereignis` have no build step. They expose TypeScript
source through `exports`, and consumers reach them through the `paths` aliases in
`tsconfig.base.json` plus pnpm's workspace linking, which Vite follows natively.
There is no `dist/` to stale, and a change in a shared package is visible to
every app on the next HMR tick.

## Commands

`apps/stack.manifest.json` is the machine-readable source for surface package
names, local ports, Pages metadata, and toolchain requirements. Use the stable
workspace commands instead of copying package filters into new scripts:

```sh
pnpm install              # once, from this directory
pnpm run doctor:web       # Node/pnpm preflight
pnpm run doctor:desktop   # adds Rust and native desktop checks
pnpm run dev:webui        # http://127.0.0.1:4173/
pnpm run dev:website      # http://127.0.0.1:4174/
pnpm run dev:desktop      # Tauri; runs WebUI itself after preflight
pnpm run stack:verify     # reject package/port/Tauri/Pages drift
pnpm run typecheck        # vue-tsc / tsc across every package
pnpm run test             # tooling contract plus package tests
```

See `docs/FRONTENDS-OPERATIONS.md` for Pages, Giscus, Windows ARM64/MSVC, and
production-build details. Per-app focused commands remain in each app's own
`package.json`.
