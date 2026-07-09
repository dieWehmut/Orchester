# Orchester

**A conductor for heterogeneous coding agents.**

Orchester puts existing coding agents — Claude Code, Codex CLI, OpenCode, … —
behind **one** unified CLI, **one** event protocol, and **one** lifecycle. It is
explicitly *not* another coding agent: it never re-implements planning, tools,
memory, or context. It launches agents you already have as subprocesses,
normalizes their native output into a single typed event stream, and manages
their lifecycle.

> The moat is the **protocol**. Get the unified `Event` stream right and any new
> agent needs only a thin adapter — usually just a TOML manifest, no Rust.

---

## Install

One-line install:

```bash
curl -fsSL https://raw.githubusercontent.com/dieWehmut/Orchester/main/install.sh | sh
```

The installer checks for required build dependencies (`git`, `curl`/`wget`,
Rust/Cargo, and a C linker) and tries to install missing pieces automatically
using the host package manager or rustup. It then installs the `orchester`
binary into `~/.cargo/bin`.

On Windows, the `curl | sh` installer also writes the install bin directory
(`%USERPROFILE%\.cargo\bin` by default) to the Windows user `PATH`. It also
creates an `orchester.cmd` shim in
`%LOCALAPPDATA%\Microsoft\WindowsApps` when that directory is writable; Windows
normally keeps that directory on `PATH`, so the same `cmd.exe` window can run
`orchester` immediately. If that shim directory is not available, open a new
terminal after installation.

On Windows PowerShell from a cloned checkout:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\werkzeug\install.ps1
```

After installation:

```bash
orchester --version
orchester doctor
orchester --agent mock "hello"
```

## Source Quickstart

Requires a Rust toolchain (1.80+) if you do not use the installer.

```bash
# List discovered adapters and their capabilities.
cargo run -p orchester-konsole -- list

# Run the built-in mock agent — no external CLI or API key needed.
cargo run -p orchester-konsole -- --agent mock "hello"

# Emit Orchester's own Event JSONL (machine mode) instead of rendered output.
cargo run -p orchester-konsole -- --agent mock --json "hello"
```

Once a real agent CLI is installed and authenticated:

```bash
orchester --agent codex "list the files in this repo"
orchester --agent claude --resume <session-id> "and now add tests"
```

`--json` writes each event as one line of Orchester's own protocol on **stdout**
(the human-readable footer goes to stderr), so Orchester can itself be piped
into another tool — or another Orchester.

---

## How it works

```
User ──▶ orchester (konsole) ──▶ Conductor (laufzeit)
                                     │
                                     ├─ Registry (verzeichnis) ── built-ins + manifeste/*.toml
                                     ├─ Session  (laufzeit)     ── Starting→Running→Completed/Failed
                                     └─ Adapter  (vertrag)      ── spawn subprocess, parse JSONL stdout
                                           │                        └─▶ normalize ─▶ protokoll::Event
                                           ▼
                              claude / codex / opencode / mock
```

**Key finding that grounds the design:** every target agent converges on the
same headless shape — *spawn subprocess → pass prompt → read line-delimited JSON
from stdout → capture a session id for resume*. So Orchester models exactly that
and maps each vendor's JSONL into one vendor-neutral `Event` enum.

### The hybrid adapter model

Adding an agent normally means shipping a **manifest**, not writing code:

```toml
# manifeste/claude.toml (excerpt)
name    = "claude"
command = "claude"
args    = ["-p", "{prompt}", "--output-format", "stream-json", "--verbose"]
resume_args = ["-p", "{prompt}", "--resume", "{session_id}", "--output-format", "stream-json", "--verbose"]
supports_resume = true

[parse]
discriminator = "type"        # top-level field selecting a branch
session_id    = "session_id"  # dotted path; emits SessionStarted once

[parse.map]
assistant = { event = "message", text = "message.content[0].text" }
result    = { event = "result",  text = "result" }
```

A generic `ManifestAdapter` interprets any such file. Code is written only where
a vendor is irregular (e.g. Codex's `exec resume <id>` **subcommand**, handled
declaratively via a full `resume_args` override).

---

## Repository layout

German role names (see [`handbuch/ARCHITEKTUR.md`](handbuch/ARCHITEKTUR.md) for
the full rationale and the naming map):

```
kisten/            # the crates (Cargo workspace members)
  protokoll/       # THE core: Task, Event, RunResult, Capability, SessionState
  vertrag/         # adapter contract: AgentAdapter trait + ManifestAdapter engine
  adapter/         # built-ins: mock (scripted) + embedded claude/codex/opencode
  verzeichnis/     # registry: discover built-ins + load manifeste/*.toml
  laufzeit/        # runtime: Conductor (dispatch) + Session (state machine)
  konsole/         # the `orchester` CLI binary
manifeste/         # declarative adapter definitions (claude, codex, opencode)
handbuch/          # documentation
forschung/         # reference-corpus research + ANALYSIS.md (see agent-research/)
werkzeug/          # build/dev helper scripts
pruefung/          # cross-crate tests
```

---

## Status & roadmap

**v0.1 (current): unify agent invocation.** Single-agent run, JSONL + rendered
output, registry with disk-override manifests, session capture/resume, mock
adapter for deterministic tests. Delivered and green (`cargo test --workspace`).

Later stages are staged deliberately so the lean core stays lean (full detail in
[`forschung/ANALYSIS.md`](agent-research/ANALYSIS.md)):

- **v0.2 — reliable local runtime:** config dir, `doctor` command, persistent
  session metadata, richer capabilities, basic TUI, more adapters as manifests.
- **v0.5 — multi-agent orchestration:** run adapters in parallel, aggregate /
  compare results, PR-review workflow, cancellation & timeouts, Git preflight,
  worktree-per-agent.
- **v1.0 — agent workflow runtime:** DAG workflows, checkpoint/resume, human
  approval interrupts, MCP/ACP bridge, cost/latency-aware routing, optional web
  UI, plugin system beyond manifests.

> **Design principle:** small at the center (protocol, adapter contract,
> registry, runtime), broad at the edges (manifests, subprocess adapters, future
> MCP/ACP bridges, workflow & UI layers). Don't reimplement agent internals —
> make agents interoperable through one runtime and one event stream.

---

## License

MIT OR Apache-2.0.
