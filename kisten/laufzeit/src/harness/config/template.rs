//! The user-facing `orchester.jsonc` template and the `.orchester` home layout.
//!
//! # Why a template is shipped rather than generated field by field
//!
//! The configuration schema rejects unknown fields, so a user who guesses a
//! field name gets a hard load error rather than a silently ignored setting.
//! That strictness is only fair if there is an authoritative example to copy,
//! which is what [`USER_CONFIG_TEMPLATE`] is. It is covered by tests in
//! `tests/config_template.rs` so it cannot drift away from the schema.
//!
//! # The `.orchester` home
//!
//! Orchester keeps its state beside the homes of the agents it drives
//! (`~/.claude`, `~/.codex`), and the layout is deliberately similar so the
//! three are legible to the same operator:
//!
//! ```text
//! ~/.orchester/
//!   orchester.jsonc   user configuration; owner-only, may hold literal keys
//!   sessions.jsonl    append-only index of orchestrated agent sessions
//!   state.sqlite3     durable run store: every loop step, resumable
//!   audit.jsonl       append-only governance decisions and tool executions
//!   memory/           cross-session memory the harness retrieves on demand
//!   plugins/          managed adapter plugin packages with install receipts
//!   logs/             rotated diagnostic logs
//! ```
//!
//! A project may additionally carry `.orchester/project.jsonc`, which is read
//! as an untrusted input: it can tighten policy but can neither introduce a
//! credential nor relax a user security decision.
//!
//! # Where a credential may live
//!
//! `orchester.jsonc` is created owner-only, and a literal `api_key` is accepted
//! only after the loader has proved that privacy still holds. Literals are then
//! moved into a private vault, so no later view can print them. The template
//! nonetheless ships references rather than literals, because a template is
//! copied into places its author cannot see.

/// Directories created inside the Orchester home, relative to it.
///
/// Kept beside the template so the scaffold and the documented layout above
/// cannot disagree.
pub const HOME_DIRECTORIES: &[&str] = &["memory", "plugins", "logs"];

/// The annotated default `orchester.jsonc`.
///
/// Every value here is either a schema default or an inert example, so writing
/// this file changes no behaviour until a user edits it. The credential fields
/// are references, never literals.
pub const USER_CONFIG_TEMPLATE: &str = r##"// Orchester user configuration.
//
// Format is JSONC: comments and trailing commas are allowed.
// This file is created owner-only (0600 on unix, a single-SID ACL on Windows).
// Orchester refuses to load it if that privacy is ever lost, so keep it that
// way rather than loosening permissions to share it.
//
// Layered on top of this file, a project may add `.orchester/project.jsonc`.
// A project file is treated as untrusted: it may make policy STRICTER, but it
// can never introduce a credential, change a provider URL, or relax a security
// decision made here.
//
// `$schema` is accepted and ignored by Orchester; point it at a local JSON
// schema if your editor can use one.
{
  "version": 1,

  // ---------------------------------------------------------------------
  // Credentials
  // ---------------------------------------------------------------------
  // Three ways to supply a key, in order of preference:
  //
  //   1. `${secret:Name}` - read from the OS keyring (Windows Credential
  //      Manager / macOS Keychain / Linux Secret Service). Store one with
  //      `orchester login <Name>`, which reads the key with hidden input so it
  //      never enters your shell history. Nothing sensitive lands on disk.
  //
  //   2. A literal string, e.g. "sk-...", written directly below. This is
  //      accepted ONLY because this file is owner-only; Orchester verifies that
  //      before reading it, and moves any literal into a private in-memory
  //      vault so `/config`, logs and crash dumps show `<redacted>` instead.
  //      It is still plaintext at rest, so prefer (1) on a shared machine.
  //
  //   3. `${env:NAME}` - indirect through another entry in `env` below.
  //
  // `env` entries whose name looks sensitive (...KEY, ...TOKEN, *SECRET*,
  // *PASSWORD*, *CREDENTIAL*) must be a reference or a literal in this
  // protected file; a plaintext secret in a non-protected file is refused.
  // Keep this object empty unless a legacy or non-provider environment
  // reference is genuinely needed. Provider keys belong directly below.
  "env": {},

  // ---------------------------------------------------------------------
  // Providers: where requests go, and with which key
  // ---------------------------------------------------------------------
  // `base_url` is the API root. Change it to point at a gateway, a proxy or a
  // self-hosted endpoint; a project file cannot override it.
  // `wire_api` selects the request shape. "responses" is the currently
  // supported transport for self-agent conversations.
  "model_providers": {
    "OpenAI": {
      "name": "OpenAI",
      "base_url": "https://api.openai.com/v1",
      // Replace with "sk-..." to keep the key in this protected file instead.
      "api_key": "${secret:OpenAI}",
      "wire_api": "responses"
    },
    "Router": {
      "name": "OpenAI-compatible router",
      "base_url": "https://agentrouter.org/v1",
      "api_key": "${secret:Router}",
      "wire_api": "responses"
    }
  },

  // ---------------------------------------------------------------------
  // Active model
  // ---------------------------------------------------------------------
  // `model_provider` picks an entry from `model_providers` above.
  "model_provider": "OpenAI",
  "model": "gpt-5.6",
  // Model used for review/critique passes, when they are requested.
  "review_model": "gpt-5.6",
  // Provider-defined effort strings; left to the provider to interpret.
  "model_reasoning_effort": "medium",
  "plan_mode_reasoning_effort": "high",
  // Provider-defined tier string, e.g. "default" or "priority".
  "service_tier": "default",
  // Ask the provider not to retain the request server-side, where supported.
  "disable_response_storage": false,

  // Named model selections, switchable at runtime with `/model <name>`.
  // A profile carries no URL and no credential: selecting one cannot move
  // traffic to a different endpoint or authorise it with a different key.
  "model_profiles": {
    "fast": {
      "model_provider": "OpenAI",
      "model": "gpt-5.6-mini",
      "model_reasoning_effort": "low"
    },
    "deep": {
      "model_provider": "OpenAI",
      "model": "gpt-5.6",
      "model_reasoning_effort": "high",
      "plan_mode_reasoning_effort": "high",
      "service_tier": "default"
    },
    "router": {
      "model_provider": "Router",
      "model": "gpt-5.6-sol"
    }
  },

  // ---------------------------------------------------------------------
  // Governance: the guardrails, enforced in code before a tool runs
  // ---------------------------------------------------------------------
  // Each decision is "allow", "ask" or "deny". "ask" parks the run on a
  // durable approval barrier until a human answers, and the run survives a
  // restart while parked (see `/approvals`). Stricter always wins a merge, so
  // a project file may raise "allow" to "ask" but never the reverse.
  //
  // The top-level Codex spelling `approvals_reviewer` is also accepted and is
  // normalised into `governance.approval_reviewer` at load time.
  "governance": {
    // Who is allowed to answer an approval request.
    "approval_reviewer": "user",
    // Tools that reach the network.
    "tool_network": "ask",
    // Any path outside the workspace root. Escaping the workspace is the
    // boundary the sandbox is built on, so this defaults to a refusal.
    "out_of_workspace": "deny",
    // Spawning a shell interpreter, which would otherwise bypass the
    // per-action guardrail entirely.
    "shell_interpreters": "deny",
    // A pending approval older than this is treated as unanswered rather than
    // silently granted.
    "approval_ttl_seconds": 900
  },

  // ---------------------------------------------------------------------
  // Limits: the halting conditions
  // ---------------------------------------------------------------------
  // Without these the loop has no stop condition beyond the model deciding to
  // stop, which is not a guarantee. All four are enforced in code.
  "limits": {
    // Maximum tool/model steps in one run.
    "max_steps": 40,
    // Wall-clock ceiling for one run.
    "max_minutes": 30,
    // Give up after this many identical consecutive failures, which is how a
    // stuck self-correction loop is detected rather than run to the step cap.
    "max_same_failure": 3,
    // Observations are truncated to this size before entering the context, so
    // one enormous tool output cannot evict the whole conversation.
    "max_observation_bytes": 65536
  },

  // ---------------------------------------------------------------------
  // Validators: the objective feedback signal
  // ---------------------------------------------------------------------
  // Each validator is a real program Orchester runs; its exit status is
  // classified and fed back into the loop as evidence, so "did that work?"
  // is answered by a process rather than by asking the model to self-assess.
  // `required: true` means a failure blocks completion of the run.
  // Inspect and run these with `/validators`.
  "validators": [
    {
      "id": "tests",
      "program": "cargo",
      "args": ["test", "--workspace"],
      "required": true
    },
    {
      "id": "lint",
      "program": "cargo",
      "args": ["clippy", "--workspace", "--all-targets"],
      "required": false
    }
  ],

  // ---------------------------------------------------------------------
  // Workspace trust
  // ---------------------------------------------------------------------
  // Directories you have explicitly marked trusted. An untrusted workspace
  // still runs, but under the stricter default policy.
  "projects": {
    // "/absolute/path/to/repo": { "trust_level": "trusted" }
  },

  // ---------------------------------------------------------------------
  // Interface
  // ---------------------------------------------------------------------
  "tui": {
    "status_line": ["current-dir", "model", "reasoning", "permissions", "task-progress"],
    "status_line_use_colors": false,
    // Countdown of remaining "this model is new" hints, per model.
    "model_availability_nux": {}
  },

  // Named experiments, off unless listed here.
  "features": {},

  // Windows-only sandbox strength; ignored elsewhere.
  "windows": {
    "sandbox": "restricted"
  },
  // Acknowledge the WSL setup notice so it stops being printed.
  "windows_wsl_setup_acknowledged": false,

  // Whether outbound network access is permitted at all. Governance still
  // applies on top of this; "restricted" is the safer starting point.
  "network_access": "restricted",

  // Suppress specific one-time notices.
  "notice": {
    "hide_full_access_warning": false,
    "hide_rate_limit_model_nudge": false
  },

  // Adapter plugin packages, disabled unless enabled here.
  // Manage them with `/plugins`.
  "plugins": {
    // "claude@orchester": { "enabled": true }
  }
}
"##;
