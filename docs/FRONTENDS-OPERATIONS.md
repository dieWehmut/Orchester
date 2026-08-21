# Frontend Operations

This document describes the three frontend targets and the commands used to
run their current development slices.

## Prerequisites

Install Node.js 24 and pnpm 10.32.1. From the repository root, install the
workspace dependencies once:

```powershell
pnpm --dir apps install --frozen-lockfile
```

## Local WebUI

The local WebUI lives in `apps/web`. Start the Vite development server from the
repository root:

```powershell
pnpm --dir apps --filter @orchester/web dev
```

Open `http://127.0.0.1:4173/`. The current slice includes the routed workspace
shell, secure bootstrap handshake, session history, session detail transcript,
three-column desktop layout, and drawer-based tablet/mobile navigation. The
live run stream and approval controls are added in subsequent slices.

Useful checks:

```powershell
pnpm --dir apps --filter @orchester/web typecheck
pnpm --dir apps --filter @orchester/web test
pnpm --dir apps --filter @orchester/web build
```

## GitHub Pages Site

The project site lives in `apps/website`. It is a separate static application;
it must not call a local WebUI server. For a local development server:

```powershell
pnpm --dir apps --filter @orchester/website dev
```

For a production-shaped build using the repository Pages base path:

```powershell
$env:BASE_PATH = '/Orchester/'
pnpm --dir apps --filter @orchester/website build
Remove-Item Env:BASE_PATH
```

GitHub Actions is the deployment mechanism. `.github/workflows/pages.yml`
installs the locked workspace, runs the website typecheck/tests/build with
`BASE_PATH=/Orchester/`, uploads `apps/website/dist`, and deploys it to the
GitHub Pages environment on pushes to `main` or a manual dispatch.

## Tauri Desktop Shell

The desktop target lives in `apps/desktop` and currently wraps the WebUI build
with a security-restricted Tauri shell:

```powershell
pnpm --dir apps --filter @orchester/desktop dev
```

The shell starts the WebUI dev server on `127.0.0.1:4173`. Native packaging is
kept separate from the root Cargo workspace. Embedded `orchester-netz` server
lifecycle and production bundles are tracked as later Tauri tasks; do not treat
the current shell as a completed desktop release.

## Stage Gates

Run the package checks before pushing a frontend branch:

```powershell
pnpm --dir apps typecheck
pnpm --dir apps test
pnpm --dir apps --filter @orchester/web build
pnpm --dir apps --filter @orchester/website build
```

For Rust checks on a Windows host with the GNU toolchain selected:

```powershell
rustup run stable-x86_64-pc-windows-gnu cargo test --workspace
```

