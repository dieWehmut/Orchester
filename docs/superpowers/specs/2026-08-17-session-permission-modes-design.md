# Session Permission Modes

## Goal

Make `/permissions` a real, session-scoped permission picker matching the
reference interaction: four selectable modes, a current marker, nested
confirmation for Full Access, Esc back-navigation, and no alternate-screen
restart or header movement. A selection must change the policy and execution
used by future self-agent turns; it must never be a display-only preference.

## User-Facing Modes

The picker exposes these ordered presets:

| Mode | Automatic access | Requires approval | Always blocked |
| --- | --- | --- | --- |
| Read Only | Read/list/search files in the current workspace | Workspace edits, structured commands, network, and paths outside the workspace | Hard invariants |
| Ask for approval | Read and edit the current workspace and run ordinary structured workspace commands | Network, paths outside the workspace, and actions classified as unsafe | Hard invariants |
| Approve for me | Read/edit the workspace and run supported low/medium-risk actions, including network | Supported high-risk actions | Hard invariants |
| Full Access | All supported file, structured command, and network actions, including paths outside the workspace | None | Hard invariants |

`Ask for approval` is the initial session mode because it most closely matches
the current configured governance. The picker marks exactly one current mode.
Selections are not written to `orchester.jsonc`; a new process starts from the
configured/default mode again.

## Hard Invariants

Permission modes decide approval and filesystem scope, but do not disable the
security properties below:

- Orchester never injects provider credentials or configured secrets into tool
  environment variables, model-visible context, or terminal output;
- streamed and durable output remains redacted before display/persistence;
- every tool attempt remains behind the durable pre-execution barrier and the
  append-only audit chain;
- cancellation, byte limits, argument limits, and process-tree termination
  remain active;
- malformed commands, shell interpreters and wrappers, command composition,
  privilege escalation, root/system destructive operations, and protected
  Orchester state and credential paths remain denied;
- Full Access is limited to Orchester's structured tool schema. It does not
  turn arbitrary model text into a shell string.

The Full Access description and confirmation explicitly state these retained
invariants. The confirmation has `Yes, continue anyway` and `Cancel`; Esc is
equivalent to Cancel. Full Access is applied only after the affirmative action.

## Runtime Model

Introduce a public `SelfAgentPermissionMode` enum in Laufzeit. The enum owns a
stable ID, label, description, and a policy/execution profile. The profile
separates three concerns instead of overloading the existing tightening-only
`GovernanceConfig`:

1. approval policy for each action risk/effect class;
2. automatic and maximum approvable filesystem scopes (`Workspace` or
   `FullDisk`), plus whether mutation is automatic or approval-gated;
3. structured execution capabilities (`workspace_commands`, `network`, and
   `external_working_directory`), each marked automatic, approval-gated, or
   unavailable.

Read Only therefore does not make approved writes impossible: workspace reads
are automatic, while a specific write receives a one-shot capability only
after approval. Ask behaves similarly for network and external paths. Full
Access is the only mode whose automatic filesystem scope is `FullDisk`.

The existing config remains the ceiling/default input for the initial Ask
mode. A session override is explicit and is included in the policy snapshot
hash. A durable run created under one mode cannot silently continue under a
different mode; continuation returns the existing dependency-mismatch error.
Mode descriptions state the requested maximum. If protected configuration
tightens it, the picker and permissions projection show the effective limit
instead of claiming unavailable access.

`SelfAgentHost` stores the selected mode next to the model-session override.
Changing it drops the cached runtime. The next turn rebuilds the runtime from
the same protected configuration and credential store plus the selected
permission profile. Status and permissions projections report the active
session mode without changing the protected user file.

## Policy Mapping

`PolicyEngine` continues to classify structured commands first. Permission
mode is applied after classification and before configured tightening:

- hard-denied categories remain Deny in every mode;
- Read Only changes all mutation/external effects to Ask;
- Ask allows workspace reads, structured file edits, checks, and ordinary
  workspace commands, while network/out-of-workspace/high-risk actions Ask;
- Approve for me upgrades supported low/medium-risk Ask results to Allow but
  leaves high/critical results at Ask or Deny;
- Full Access upgrades every non-invariant supported result to Allow.

Configured governance may tighten a session mode but cannot weaken a hard
invariant. The final decision, mode ID, rule ID, risk, and effect class are
persisted and audited.

## Execution And Approval

Replace the read-only `ToolExecutor` dispatch with one governed asynchronous
executor that routes the already-authorized `StartedTool` to existing bounded
implementations:

- list/search/read -> scope-aware `FileTools` that apply the selected
  filesystem scope and the invariant protected-path guard;
- write -> a scope-aware `GovernedWorkspaceWriter`;
- patch -> a scope-aware `GovernedWorkspacePatcher`;
- command/check -> `GovernedProcessRunner` with cancellation and bounded
  output.

The executor never makes policy decisions. It consumes only the permit created
by `PreExecutionBarrier` after the durable policy result or approval decision.
Expanding from `Workspace` to `FullDisk` changes the allowed root set; it
does not bypass canonicalization, symlink/reparse-point checks, protected-path
checks, byte limits, or atomic-write rules.

For Ask decisions, the runtime returns a typed pending-approval outcome. The
TTY opens an approval overlay in the same `ChatSession`. Approve records the
durable approval decision, consumes its capability exactly once, executes the
tool, and continues the same run. Reject records denial and returns to the
conversation. EOF/cancellation never implicitly approves.
The approval capability binds the canonical action, canonical target paths,
owner/workspace identity, permission-mode hash, policy snapshot, and expiry;
it cannot authorize a modified command or a broader path on replay.

## TUI Flow

`/permissions` opens a typed `TerminalOverlay` rather than the old inspection
report. Up/Down changes selection; Enter applies the first three modes. Enter
on Full Access pushes a nested warning overlay whose parent is the mode picker.
Esc from the warning returns to the picker; Esc from the picker closes it.

On successful selection, the overlay becomes a persistent result view and the
status line updates to the selected mode. During a busy model turn, the command
is queued and visibly acknowledged, then applied before the next model turn.
The existing stable header, fixed composer row, transcript scroll, theme, and
partial synchronized redraw paths are reused unchanged.

## Failure Handling

- Runtime rebuild failure leaves the previous mode active and shows a typed
  error in the overlay.
- A stale approval or mismatched run/mode is rejected without executing a
  tool.
- Unsupported tool actions remain Deny and explain the invariant; Full Access
  never changes Unsupported into execution.
- Audit or durable-store failure is fail-closed and keeps the TUI session alive.
- Full Access confirmation is never inferred from the selected row, key repeat,
  EOF, or a queued command.

## Verification

Tests are layered around the actual contracts:

1. Policy matrix tests cover every mode against reads, writes, ordinary
   commands, network, delete, shell, privilege escalation, malformed input,
   and explicit approval.
2. Executor integration tests prove permitted write/patch/process actions run,
   denied actions never start, output is bounded, cancellation terminates the
   process tree, full-disk paths are unavailable outside Full Access, and
   protected paths remain unavailable in every mode.
3. Durable approval tests prove approve/reject/replay/stale-capability behavior
   and same-run continuation.
4. Host tests prove mode selection is session-only, invalidates the runtime,
   appears in projections, and changes the policy snapshot.
5. TUI reducer/render tests cover current markers, nested Full Access
   confirmation, Esc parent navigation, queued selection, stable header rows,
   fixed composer position, and active-theme rendering at roomy and 80x24
   viewports.
6. A Windows ConPTY smoke test drives `/permissions`, selects each non-dangerous
   mode, cancels and confirms Full Access, submits a loopback tool turn, and
   asserts one alternate-screen session with no full-screen clear.
7. Full workspace tests, Clippy with warnings denied, formatting, release build,
   and a real provider text-only turn are rerun before release.

## Release Gate And Plan Boundaries

The permission feature depends on streaming and durable redaction being safe
at provider-response boundaries. The existing sanitizer review findings for
cross-response fragments, overlapping detector order, normalized configured
secrets, bounded incremental cost, and multiline assistant persistence are a
separate security-hardening plan and must be closed before these permission
modes or v0.1.2 are released. Full Access remains unavailable in release
artifacts until that gate and the protected-path matrix pass.

Implementation is split into two dependent plans rather than one oversized
change. The runtime plan owns the mode model, policy matrix, scoped executor,
and durable approval continuation. The TUI plan starts only after those APIs
are green and owns the session override, picker, confirmation, projections,
and ConPTY/frame verification. Each plan uses the commit boundaries below.

## Commit Boundaries

1. Add the permission-mode policy model and exhaustive matrix tests.
2. Add governed asynchronous write/patch/process execution and approval
   continuation.
3. Add session override/projections to `SelfAgentHost`.
4. Add the typed `/permissions` picker and Full Access confirmation.
5. Add ConPTY/frame-sequence verification and release hardening.
