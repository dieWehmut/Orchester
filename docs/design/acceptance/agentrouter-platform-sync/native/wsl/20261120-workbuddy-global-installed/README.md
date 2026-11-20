# WorkBuddy international native Linux installation, 2026-11-20

The official international Linux package **5.7.6.40488862-3e43260f** is installed at `/home/han/.local/opt/workbuddy-ai/5.7.6.40488862-3e43260f`, with the distinct `~/.local/bin/workbuddy-ai` command and “WorkBuddy AI” desktop entry. The existing domestic command, desktop entry and executable retain their original hashes.

A community manifest supplied an exact candidate URL. The binary was fetched directly from the Tencent COS bucket that the [official CodeBuddy homepage](https://www.codebuddy.ai/) explicitly uses. The complete Debian package is 421,733,316 bytes, SHA256 `3cf1244520d54eca391378a29579918122006f5bfdba942fe977b908365e93db`. Its actual Debian metadata, native x86-64 ELF, ASAR package and bundled product configuration identify the international SaaS product: `WorkBuddy AI`, `isOversea: true`, `workbuddy.ai`, and `.workbuddy-ai`. No community binary or installer script was executed.

At **03:58:34–03:58:42 UTC**, an owned Xvfb display inside a fresh, network-isolated bubblewrap namespace displayed a **1199×799 “WorkBuddy AI”** window. A normal Ctrl+Q produced application exit 0 and outer exit 0 without application signals. Thirteen observed non-infrastructure process identities became terminal. The raw receipt also records PID 1 (`bwrap`) as still live while the controller ran; the separate derived receipt excludes that infrastructure identity and uses the actual outer exit 0. The raw result remains unchanged.

The primary profile was `.workbuddy-ai/app`. The clean test HOME also created two helper log files under `.workbuddy/logs`; this evidence does not claim all helper paths differ between editions. The original HOME was hidden. Authentication, model calls and regional service availability were not tested.

The first `--version` attempt entered the desktop application and failed without an X server. Headless Ozone also failed in GTK initialization. Two Xvfb setup attempts failed before the successful controlled run; raw exits and errors are retained under `failures/`. Successful `--version` output and latest-version status are not claimed. The earlier [feed investigation](../20261120-workbuddy-global/README.md) remains valid historical evidence for its six exact dead feed URLs.

`sources/` contains public discovery and publisher sources, the actual ASAR package metadata, and the exact GUI controller/runner. Large binaries, full product configuration and fresh-test logs remain outside the repository and are hash-bound in `omitted-source-bindings-safe.json`. The independent review predates GUI recovery and is retained with that limitation in its filename.

Run the offline evidence and negative checks:

```sh
python3 validate.py --self-test
```

On the installation machine, also rehash the actual package, ELF, ASAR, product metadata, launchers and preserved domestic files without executing an application:

```sh
python3 validate.py --self-test --live-install
```
