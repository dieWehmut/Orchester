# Session Permission Modes

## Goal

Make `/permissions` a real, session-scoped permission picker matching the
reference interaction: four selectable modes, a current marker, nested
confirmation for Full Access, Esc back-navigation, and no alternate-screen
restart or header movement. A selection must change the policy and execution
used by future self-agent turns; it must never be a display-only preference.

## User-Facing Modes

The picker exposes these ordered presets:

| Mode | Target automatic access | Requires approval | Always blocked |
| --- | --- | --- | --- |
| Read Only | Structured read/list/search in the current workspace | Workspace edits and supported external effects | Hard invariants and unsandboxed processes |
| Ask for approval | Structured reads and edits in the current workspace | Supported external paths, network, and unsafe actions | Hard invariants and unsandboxed processes |
| Approve for me | Supported low/medium-risk structured actions | Supported high-risk actions | Hard invariants and unsandboxed processes |
| Full Access | All supported structured file actions, including paths outside the workspace | None | Hard invariants and unsandboxed processes |

`Ask for approval` is the initial session mode because it most closely matches
the current configured governance. The picker marks exactly one current mode.
Selections are not written to `orchester.jsonc`; a new process starts from the
Ask mode again.

The table describes the target after each capability gate passes. The first
shippable slice covers structured file tools only. `RunCommand`, `RunChecks`,
and tool network access stay visibly unavailable in every mode until an OS
sandbox proves filesystem roots, network policy, and child-process inheritance
on the current platform. A mode name must never imply that an unavailable
capability is active.

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
3. structured execution capabilities (`workspace_files`, `sandboxed_process`,
   `tool_network`, and `external_working_directory`), each marked automatic,
   approval-gated, or unavailable.

Read Only therefore does not make approved writes impossible: workspace reads
are automatic, while a specific write receives a one-shot capability only
after approval. Ask behaves similarly for network and external paths. Full
Access is the only mode whose target automatic filesystem scope is `FullDisk`,
and that scope stays unavailable until its platform path-hardening gate passes.

Every process starts in Ask mode. A session override is explicit and is
included in the policy snapshot hash. A durable run created under one mode
cannot silently continue under a different mode; continuation returns the
existing dependency-mismatch error. Mode descriptions state the requested
maximum. If an explicit administrator ceiling or an unavailable capability
gate tightens it, the picker and permissions projection show the effective
limit instead of claiming unavailable access.

`SelfAgentHost` stores separate requested and active modes next to the model
session override. Selection first builds and validates a replacement runtime;
only a successful build atomically swaps the active mode and runtime. Failure
keeps the previous pair. Status and permissions projections report requested,
active, and effective capability gates without changing the protected user
file.

## Policy Mapping

`PolicyResult` gains a typed disposition in addition to decision, rule, risk,
and effect:

- `HardInvariant` can never be upgraded;
- `Unsupported` has no executor and remains Deny;
- `ExplicitCheckpoint` always pauses even in Full Access;
- `ModeAdjustable` is the only disposition a permission mode may change.

`PolicyEngine` classifies the structured action first. Permission mode is
applied only to `ModeAdjustable` results and before explicit administrator
tightening:

- hard-denied categories remain Deny in every mode;
- Read Only changes all mutation/external effects to Ask;
- Ask allows workspace reads and structured file edits, while supported
  out-of-workspace/high-risk actions Ask; process/check/network actions remain
  Unsupported until their capability gates pass;
- Approve for me upgrades supported low/medium-risk Ask results to Allow but
  leaves high/critical results at Ask or Deny;
- Full Access upgrades every supported `ModeAdjustable` result to Allow.

Ordinary built-in defaults are replaced by the selected session preset; they
are not treated as an administrator ceiling. Configuration adds a distinct,
provenance-preserving `permission_ceiling` whose absent value means no extra
tightening and whose explicit Allow/Ask/Deny values may only tighten the
preset. The loader preserves absent versus explicit values. The exact
precedence matrix is tested for every mode and ceiling state.

The final decision, disposition, mode ID/hash, rule ID, risk, effect class,
granted scope, and ceiling provenance are persisted and externally audited for
Allow, Ask, and Deny outcomes.

## Execution And Approval

Replace the read-only `ToolExecutor` dispatch with one governed asynchronous
structured-file executor that routes the already-authorized `StartedTool` to
bounded implementations:

- list/search/read -> scope-aware `FileTools` that apply the selected
  filesystem scope and the invariant protected-path guard;
- write -> a scope-aware `GovernedWorkspaceWriter`;
- patch -> a scope-aware `GovernedWorkspacePatcher`;
- command/check -> Unsupported until the process-sandbox milestone.

The executor never makes policy decisions. It consumes only the permit created
by `PreExecutionBarrier` after the durable policy result or approval decision.
Every automatic or approved permit binds the mode hash, policy snapshot,
granted roots, owner/workspace identity, expiry, action hash, and capability
gates. The barrier revalidates those bindings immediately before appending
`ToolStarted`; changing mode or scope invalidates the candidate.
Expanding from `Workspace` to `FullDisk` changes the allowed root set; it
does not bypass canonicalization, symlink/reparse-point checks, protected-path
checks, byte limits, or atomic-write rules.

For Ask decisions, the runtime returns a typed pending-approval outcome. The
TTY opens an approval overlay in the same `ChatSession`. Approve records the
durable approval decision, consumes its capability exactly once, executes the
tool, and continues the same run. Reject records denial and returns to the
conversation. EOF/cancellation never implicitly approves.
The approval capability binds the canonical structured action, canonical
target paths, owner/workspace identity, permission-mode hash, policy snapshot,
granted roots, and expiry; it cannot authorize a modified action or a broader
path on replay.

Process execution is a separate capability milestone. The current
`GovernedProcessRunner` validates cwd but launches an ambient OS process, so it
does not satisfy any mode's filesystem or network guarantees. Before enabling
it, a platform sandbox must confine the executable and every child to granted
roots and network policy. A process permit binds the executable, argv digest,
canonical cwd, sandbox profile, granted roots, and network capability; it does
not claim to enumerate arbitrary command target paths. Windows drive, UNC,
junction, hardlink, and child-process escape tests are required. Unsupported
platforms keep process actions disabled.

`RunChecks` is not an alias for `RunCommand`. Its future executor requires a
separate specification for allowed checks, per-check barriers, cancellation,
mutation snapshots, audit records, and terminal outcomes.

Protocol and storage changes are part of the runtime work: policy events,
execution candidates, permits, actions, and audit records gain the fields
above through a versioned migration. Reopen and backward-compatibility tests
cover older databases. Every final policy decision is externally audited,
including Deny and unresolved Ask, rather than only decisions that reach tool
preparation.

## TUI Flow

`/permissions` opens a typed `TerminalOverlay` rather than the old inspection
report. Up/Down changes selection; Enter applies the first three modes. Enter
on Full Access pushes a nested warning overlay whose parent is the mode picker.
Esc from the warning returns to the picker; Esc from the picker closes it.

On successful selection, the overlay becomes a persistent result view and the
status line updates to the selected mode. During a busy top-level run, the
command is queued and visibly acknowledged. One mode snapshot remains fixed
for the entire run, including all automatic tool/model continuations; the
queued change applies only after that run completes and before the next
top-level run starts.
The existing stable header, fixed composer row, transcript scroll, theme, and
partial synchronized redraw paths are reused unchanged.

## Failure Handling

- Runtime rebuild failure leaves the previous active mode/runtime pair intact,
  preserves the failed requested mode for diagnostics, and shows a typed error
  in the overlay.
- A stale approval or mismatched run/mode is rejected without executing a
  tool.
- Unsupported tool actions remain Deny and explain the invariant; Full Access
  never changes Unsupported into execution.
- Audit or durable-store failure before `ToolStarted` is fail-closed. If a
  terminal append fails after a side effect starts, the already-durable
  `ToolStarted` remains without a terminal event, the runtime reports
  `UnknownOutcome`, and reopen requires reconciliation instead of replay while
  the TUI stays alive.
- Full Access confirmation is never inferred from the selected row, key repeat,
  EOF, or a queued command.

## Verification

Tests are layered around the actual contracts:

1. Policy matrix tests cover every mode and absent/explicit ceiling state
   against reads, writes, process/network requests, delete, shell, privilege
   escalation, malformed input, unsupported actions, and explicit checkpoints.
2. Structured executor integration tests prove permitted write/patch actions
   run, denied actions never start, full-disk paths are unavailable before the
   FullDisk gate, and protected paths remain unavailable in every mode.
3. Durable approval tests prove approve/reject/replay/stale-capability behavior,
   mode changes between decision and start, and same-run continuation.
4. Protocol/schema/audit tests prove all policy outcomes carry mode,
   disposition, risk, effect, scope, and provenance across migration/reopen.
5. Host tests prove mode selection is session-only, uses two-phase atomic
   runtime replacement, appears in projections, and changes the policy
   snapshot only on successful activation.
6. TUI reducer/render tests cover current markers, nested Full Access
   confirmation, Esc parent navigation, queued selection, stable header rows,
   fixed composer position, and active-theme rendering at roomy and 80x24
   viewports.
7. A Windows ConPTY smoke test drives `/permissions`, selects each non-dangerous
   mode, cancels and confirms Full Access, submits a loopback tool turn, and
   asserts one alternate-screen session with no full-screen clear.
8. Adversarial process tests attempt outside/protected reads and writes,
   outbound network, child/grandchild escape, drive/UNC/junction/hardlink
   traversal, and cancellation for every mode. Process capabilities stay
   unavailable until these pass on the target platform.
9. Recovery tests inject storage failure before and after side-effect start;
   the former never executes and the latter records a non-replayable
   `UnknownOutcome` requiring reconciliation.
10. Full workspace tests, Clippy with warnings denied, formatting, release build,
   and a real provider text-only turn are rerun before release.

## Release Gate And Plan Boundaries

The permission feature depends on streaming and durable redaction being safe
at provider-response boundaries. The existing sanitizer review findings for
cross-response fragments, overlapping detector order, normalized configured
secrets, bounded incremental cost, and multiline assistant persistence are a
separate security-hardening plan and must be closed before these permission
modes or v0.1.2 are released. Full Access remains unavailable in release
artifacts until that gate and the protected-path matrix pass. Process/network
capabilities additionally remain unavailable until the OS-sandbox gate passes.

Implementation is split into dependent plans rather than one oversized change:
policy/protocol/schema, structured workspace execution and approval, host/TUI,
FullDisk path hardening, and OS process sandboxing. The TUI may expose only
capabilities whose preceding runtime plan is green, and renders unavailable
gates honestly. Each plan uses the commit boundaries below.

## Commit Boundaries

1. Add typed dispositions, permission presets/ceilings, and policy matrix.
2. Version protocol events, database schema, and external audit records.
3. Bind mode, policy, identity, expiry, and granted scope into every permit.
4. Add governed asynchronous workspace write/patch execution.
5. Add durable approval, rejection, unknown-outcome, and reconciliation flows.
6. Add two-phase session override/projections to `SelfAgentHost`.
7. Add the typed `/permissions` picker, truthful capability gates, and Full
   Access confirmation.
8. Add Windows FullDisk path hardening and protected-path adversarial tests.
9. Add an OS process sandbox and only then enable supported command/network
   capabilities per mode.
10. Add ConPTY/frame-sequence verification and release hardening.
