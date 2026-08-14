<h1 align="center">Orchester</h1>

<p align="center">
  <img src="https://count.getloli.com/get/@Orchester?theme=rule34" alt="Visitors">
</p>

<div align="center">

<a href="https://www.rust-lang.org/" target="_blank">
  <img src="https://img.shields.io/badge/RUST-1.80%2B-000000?style=flat-square&logo=rust&logoColor=white&labelColor=555555" alt="Rust">
</a>
<a href="https://www.npmjs.com/package/@orchester/cli" target="_blank">
  <img src="https://img.shields.io/badge/NPM-%40orchester%2Fcli-CB3837?style=flat-square&logo=npm&logoColor=white&labelColor=555555" alt="npm">
</a>
<a href="#1-install">
  <img src="https://img.shields.io/badge/PLATFORM-WIN%20%7C%20MAC%20%7C%20LINUX-4C8BF5?style=flat-square&logo=windowsterminal&logoColor=white&labelColor=555555" alt="Platform">
</a>
<a href="https://github.com/dieWehmut/Orchester/blob/main/LICENSE-MIT">
  <img src="https://img.shields.io/badge/LICENSE-MIT%20OR%20APACHE--2.0-green?style=flat-square&logo=github&logoColor=white&labelColor=555555" alt="License">
</a>

</div>

<div align="center">

[简体中文](../README.md) | [繁體中文](README.zh-TW.md) | English

</div>

---

`Orchester` is an **independent coding agent** written in Rust. It owns task understanding, context assembly, planning, model calls, tool execution, validation feedback, memory, human approval, and durable recovery.

Claude Code, Codex CLI, OpenCode, and other external agents are optional delegation capabilities. Orchester can drive them through manifests and a unified event protocol, but they do not define Orchester's identity or take ownership of its core loop.

## Links

- Repository: <https://github.com/dieWehmut/Orchester>
- npm package: <https://www.npmjs.com/package/@orchester/cli>

## Features

- Independent agent loop: context → model → action → policy/approval → tool → feedback/memory → stop
- Governed file and command tools with path guards, validators, and a hash-chain audit
- Durable runs, project memory, one-shot approvals, and `--resume` recovery
- Interactive terminal with `/status`, `/permissions`, `/resume`, `/model`, `/plugins`, and related commands
- One event protocol: a `Task` goes in, an `Event` stream comes out, and a `RunResult` closes the run
- Optional external-agent delegation through manifest-driven `claude`, `codex`, `opencode`, and `mock` adapters
- `--json` emits Orchester's own JSONL for integration with other tools
- `doctor` checks the local runtime and optional external agents
- Plugin management
- One-line installers (macOS / Linux / Windows) and npm distribution

## Quickstart

### 1. Install

macOS and Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/dieWehmut/Orchester/main/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/dieWehmut/Orchester/main/install.ps1 | iex
```

The installer checks for `git`, `curl`/`wget`, Rust/Cargo, and a C linker, installs whatever is missing through the host package manager or rustup, then puts `orchester` into `~/.cargo/bin`.

On Windows the installer also writes the install bin directory (`%USERPROFILE%\.cargo\bin` by default) to the user `PATH`, and creates an `orchester.cmd` shim in `%LOCALAPPDATA%\Microsoft\WindowsApps` when that directory is writable, so the same `cmd.exe` window can run `orchester` right away. If the shim directory is unavailable, open a new terminal after installation.

Because `irm | iex` cannot bind parameters, the PowerShell one-liner reads its settings from the environment:

```powershell
$env:ORCHESTER_INSTALL_ROOT = "D:\tools\orchester"   # default: %USERPROFILE%\.cargo
$env:ORCHESTER_NO_PATH_UPDATE = "1"                  # leave PATH untouched
$env:ORCHESTER_REF = "main"                          # branch, tag, or commit
irm https://raw.githubusercontent.com/dieWehmut/Orchester/main/install.ps1 | iex
```

You can also install from a cloned checkout:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\werkzeug\install.ps1
```

### 2. Install from npm

Once the tagged npm release is approved, the meta package selects the native package for the current platform automatically:

```bash
npm install -g @orchester/cli
pnpm add -g @orchester/cli
yarn global add @orchester/cli
bun add -g @orchester/cli
```

The package has no lifecycle downloader. The release workflow publishes the six native platform packages first and submits `@orchester/cli` only after those versions are visible in the public registry.

### 3. Build from source

Without the installer you need a Rust 1.80+ toolchain:

```bash
git clone https://github.com/dieWehmut/Orchester.git
cd Orchester
cargo build --release
```

### 4. First run

The built-in `mock` adapter spawns no subprocess and needs no API key, so it exercises the whole pipeline on its own:

```bash
orchester --version
orchester doctor
orchester --agent mock "hello"
```

From source:

```bash
cargo run -p orchester-konsole -- list
cargo run -p orchester-konsole -- --agent mock "hello"
cargo run -p orchester-konsole -- --agent mock --json "hello"
```

### 5. Delegate to an external agent (optional)

Once the corresponding agent CLI is installed and authenticated:

```bash
orchester --agent codex "list the files in this repo"
orchester --agent claude --resume <session-id> "and now add tests"
```

`--json` writes each event as one line of Orchester's own protocol on **stdout** (the human-readable footer goes to stderr), so Orchester can be piped into another tool — or into another Orchester.

### 6. Configure

The Orchester home is `~/.orchester` on every platform, kept beside the `~/.claude` and `~/.codex` homes of the agents it drives. `ORCHESTER_HOME` overrides that root as a whole, so the config and the state always move together.

| Path | Purpose |
|---|---|
| `~/.orchester/orchester.jsonc` | User config: models, providers, governance policy, plugins |
| `~/.orchester/sessions.jsonl` | Delegated-agent session records, read by `sessions` |
| `~/.orchester/state/runs.db` | Orchester run records, read by `/resume` |
| `~/.orchester/state/audit.jsonl` | Hash-chain audit log |
| `<project>/.orchester/project.jsonc` | Project config, validated as untrusted input — it cannot introduce credentials or relax security |

`orchester.jsonc` allows comments and looks roughly like this:

```jsonc
{
  "version": 1,
  "model_provider": "OpenAI",
  "model": "gpt-5.6-sol",
  "model_reasoning_effort": "high",
  "model_providers": {
    "OpenAI": {
      "name": "OpenAI",
      "base_url": "https://api.openai.com/v1",
      "wire_api": "responses",
      "api_key": "${secret:OpenAI}"
    }
  },
  "projects": {
    "/path/to/repo": { "trust_level": "trusted" }
  },
  "governance": { "approval_reviewer": "user" },
  "tui": { "status_line": ["current-dir", "model", "permissions"] },
  "plugins": { "example@source": { "enabled": true } }
}
```

`${secret:Provider}` resolves through the credential vault and `${env:NAME}` remains available for legacy or non-provider environment entries. Put the selected provider's `base_url`, `wire_api`, and `api_key` directly in `model_providers`; a present `api_key` enables Bearer authentication by default, so no separate `env` relay or `requires_openai_auth` flag is required. A literal `api_key` is accepted only from a **protected** user config file — `0600` on the file and `0700` on the directory under Unix, a tightened ACL under Windows — and is always redacted when serialized or formatted into an error.

## Slash commands

Available in interactive mode (run `orchester` with no arguments):

| Command | Purpose |
|---|---|
| `/agent` | Choose or switch the delegate agent |
| `/model` | Inspect Orchester's model catalog, switch profile |
| `/config` | Show the resolved configuration: both layers, redacted body, permission findings — and still report the path and the reason when the config cannot be read |
| `/permissions` | Show the effective permissions |
| `/resume` | List resumable runs |
| `/status` | Show Orchester's workspace status |
| `/login` | Store a provider API key in the OS keyring; config keeps only a reference |
| `/logout` | Forget a stored provider API key |
| `/plugins` | Manage plugins (`list` / `status` / `install` / `remove`) |
| `/claude` `/codex` `/opencode` | Launch the corresponding native agent |
| `/help` | Show help |
| `/quit` | Exit; `/exit` and `/q` are synonyms |

Typing `/` opens the command palette — arrow keys to select, Enter to confirm.

## Command line

| Command | Purpose |
|---|---|
| `orchester run <prompt>` | Run one adapter with a prompt; also the default mode |
| `orchester list` | List discovered adapters and their capabilities |
| `orchester doctor [--strict]` | Check local adapter availability |
| `orchester sessions` | List locally recorded session metadata |
| `orchester config` | Print the resolved configuration; secrets appear as references only |
| `orchester login [provider]` | Store a provider API key; omit the provider to use the active one |
| `orchester logout [provider]` | Remove a stored provider API key |
| `orchester plugin <list\|status\|install\|remove>` | Plugin management |

Global flags: `--agent/-a`, `--resume`, `--model/-m`, `--json`.
`--agents`, `--parallel`, and `--auto` are parsed but not yet wired; they fail loudly with "not yet implemented" rather than silently doing the wrong thing.

## Adding an external agent (optional)

Adding an agent normally means shipping a **manifest**, not writing code. Drop a TOML file into `manifeste/`:

```toml
# manifeste/claude.toml (excerpt)
name    = "claude"
command = "claude"
args    = ["-p", "{prompt}", "--output-format", "stream-json", "--verbose"]
resume_args = ["-p", "{prompt}", "--resume", "{session_id}", "--output-format", "stream-json", "--verbose"]
kinds = ["code", "chat"]
supports_resume = true
streaming = true

[parse]
discriminator = "type"        # top-level field selecting a branch
session_id    = "session_id"  # dotted path; emits SessionStarted once

[parse.map]
assistant = { event = "message", text = "message.content[0].text" }
result    = { event = "result",  text = "result" }
```

A generic `ManifestAdapter` interprets any such file. Rust is written only where a vendor is genuinely irregular — for example Codex resumes through an `exec resume <id>` **subcommand**, handled declaratively via a full `resume_args` override.

A disk manifest wins over a built-in of the same name, so tweaking a built-in agent's flags never requires a rebuild.

## How it works

```
Developer ──▶ orchester CLI ──▶ Application Service
                                      │
                                      ├─▶ Independent Agent Runtime
                                      │     Context → Model → Action
                                      │     → Policy/Approval → Tool
                                      │     → Feedback/Memory/Stop
                                      │
                                      └─▶ Optional Delegation
                                            Registry → Adapter
                                            → claude / codex / opencode / mock
```

Orchester's independent loop always owns execution authority and the stop decision. External-agent delegation is a separate, optional path: adapters spawn subprocesses, parse JSONL, retain session metadata, and convert results into the unified `Event` stream.

## Repository layout

Crates are separated by responsibility:

```text
kisten/            # Cargo workspace members
  protokoll/       # the core: Task, Event, RunResult, Capability, SessionState
  modell/          # provider-neutral, single-call language-model boundary
  vertrag/         # adapter contract: AgentAdapter trait + ManifestAdapter engine
  adapter/         # built-ins: mock + compile-time embedded claude/codex/opencode
  verzeichnis/     # registry: discover built-ins + load manifeste/*.toml
  laufzeit/        # runtime: independent agent loop, Conductor, Session, and governance subsystems
  konsole/         # the orchester CLI binary
manifeste/         # declarative adapter definitions
werkzeug/          # install and development helper scripts
npm/               # npm distribution packages
.github/           # CI and release workflows
```

`kisten/laufzeit/src/harness/` is the independent agent core: config, credentials, context, model boundary, memory, audit, policy, approvals, tool registry, process sandbox contract, validators, and the feedback engine.

## Common commands

```bash
cargo build --release          # build
cargo test --workspace         # full test suite
cargo fmt --all -- --check     # formatting check
cargo clippy --all-targets -- -D warnings
```

## Roadmap

- **v0.1 (current) — independent agent foundation:** autonomous loop, governed tools, approvals, feedback, memory, recovery, JSONL, and deterministic mock tests.
- **v0.2 — reliable local agent:** complete config home, `doctor`, durable runs, richer terminal interaction, validators, and plugins.
- **v0.5 — advanced delegation:** optional parallel external-agent runs, result aggregation and comparison, PR-review workflows, cancellation and timeouts, Git preflight, and isolated worktrees.
- **v1.0 — agent workflow runtime:** DAG workflows, checkpoint/resume, human approval interrupts, MCP/ACP bridge, cost- and latency-aware routing, and a plugin system beyond manifests.

> Design principle: Orchester owns its main loop, execution authority, and stop conditions. Protocols, manifests, and adapters extend optional delegation without handing control of the core agent to an external process.

## Contributing

Issues and pull requests are welcome. To keep maintenance smooth, please follow these conventions:

1. Open an issue first for large changes, describing motivation, scope, and expected behavior.
2. Branch off the latest `main`, e.g. `feat/manifest-timeout` or `fix/resume-id`.
3. Keep changes focused — one concern per pull request.
4. Follow the write-a-failing-test, confirm Red, implement, confirm Green loop.
5. Run `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --workspace` before submitting.
6. If a change affects configuration, installation, or usage, update all three READMEs.

## License

MIT OR Apache-2.0. See [LICENSE-MIT](../LICENSE-MIT) and [LICENSE-APACHE](../LICENSE-APACHE).
