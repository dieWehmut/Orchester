# Frontend Operations

Orchester exposes three developer surfaces from one pnpm workspace. Their
packages, addresses, deployment metadata, and toolchain requirements are
versioned in `apps/stack.manifest.json`; `pnpm --dir apps stack:verify` rejects
drift between that manifest and the Vite, Tauri, package, Cargo, and Pages
configuration.

## Install and diagnose

From the repository root:

```powershell
pnpm --dir apps install --frozen-lockfile
pnpm --dir apps doctor:web
```

The WebUI and website require Node.js 22.12 or newer and pnpm 10.32.1. The
Pages workflow deliberately uses its separately pinned Node.js 24.8.0 runner.

Before starting Tauri, run:

```powershell
pnpm --dir apps doctor:desktop
```

The desktop profile also checks Rust/Cargo and native Windows tools. Any failed
required check produces a non-zero exit code. `dev:desktop` runs the same
preflight and refuses to start Tauri when it fails; there is no supported bypass
flag. Fix the environment described by the output, then run the command again.

For automation or support reports, JSON output is available directly:

```powershell
node werkzeug/frontend/doctor.mjs desktop --json
```

## Local WebUI

```powershell
pnpm --dir apps dev:webui
```

Open `http://127.0.0.1:4173/`. The launcher passes `--strictPort`, so it fails
instead of silently moving to another port when 4173 is occupied. This address
is also the Tauri development frontend and must remain stable.

Focused checks:

```powershell
pnpm --dir apps --filter @orchester/web typecheck
pnpm --dir apps --filter @orchester/web test
pnpm --dir apps --filter @orchester/web build
```

## GitHub Pages website

```powershell
pnpm --dir apps dev:website
```

Open `http://127.0.0.1:4174/`. This static site is independent of the local
WebUI API. Like the WebUI launcher, it uses a strict port.

For a production-shaped local build:

```powershell
$env:BASE_PATH = '/Orchester/'
pnpm --dir apps --filter @orchester/website build
Remove-Item Env:BASE_PATH
```

The deployment contract is `BASE_PATH=/Orchester/`, artifact directory
`apps/website/dist`, and public URL
`https://diewehmut.github.io/Orchester/`. GitHub Actions is the only supported
deployment mechanism: `.github/workflows/pages.yml` runs frozen install,
tooling tests, manifest verification, website typecheck/tests/build, artifact
upload, and Pages deployment for relevant pushes to `main` or manual dispatch.
It does not use a `gh-pages` branch.

Giscus remains optional. The Action reads public repository variables
`GISCUS_REPO`, `GISCUS_REPO_ID`, `GISCUS_CATEGORY`, and
`GISCUS_CATEGORY_ID`; the component stays disabled if the complete set is not
available.

## Tauri desktop shell

```powershell
pnpm --dir apps dev:desktop
```

Tauri starts the WebUI on `http://127.0.0.1:4173/` through its checked-in
`beforeDevCommand`; do not start a second WebUI process for the same desktop
session. On Windows, install Microsoft C++ Build Tools with **Desktop
development with C++**, the architecture-matched MSVC tools, the Windows SDK,
and WebView2. Open the matching Visual Studio Developer PowerShell before
launching an MSVC Rust target. The upstream prerequisite reference is
<https://v2.tauri.app/start/prerequisites/>.

The current ARM64 workstation selects `aarch64-pc-windows-msvc` but resolves
`link.exe` from MSYS2 and cannot find `cl.exe`. The doctor therefore reports
`windows-linker-shadowed` and `windows-msvc-compiler-missing` and exits 1. That
is an honest environment failure, not a passing desktop build.

## Contract and stage gates

Inspect commands without starting long-running servers:

```powershell
node werkzeug/frontend/launch.mjs webui --dry-run
node werkzeug/frontend/launch.mjs website --dry-run
node werkzeug/frontend/launch.mjs desktop --dry-run
```

Run the machine-level contract checks:

```powershell
pnpm --dir apps test:tooling
pnpm --dir apps stack:verify
```

Run the complete frontend gates before merging:

```powershell
pnpm --dir apps typecheck
pnpm --dir apps test
pnpm --dir apps --filter @orchester/web build
$env:BASE_PATH = '/Orchester/'
pnpm --dir apps --filter @orchester/website build
Remove-Item Env:BASE_PATH
git diff --check
```

The stable commands should be used instead of copying package-filter details
into new documentation or scripts. Change the stack manifest and its tests in
the same commit when a package, port, URL, base path, or toolchain requirement
changes.
