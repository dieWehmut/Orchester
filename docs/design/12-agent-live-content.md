# Agent live content

Date: 2026-10-11. The user explicitly requested current content from Codex, Claude Code and OpenCode, including tasks launched outside Orchester.

## Contract and interaction

Selecting an agent opens a live session reader in the main workspace. The agent list appears before historical conversations so current work remains discoverable. Each plugin also offers a view-content action without requiring adapter installation. The existing Home, schedules, plugins and bottom navigation order stays unchanged; no bottom panel is introduced.

The reader shows current/recent sessions, project, last update, user requests, visible assistant replies and tool calls/results. It follows new output only when already at the bottom and otherwise offers a latest-output action. Switching sessions preserves the selected session during refresh. Loading, missing local records, unreadable storage and disconnected refresh are explicit states. External sessions are read-only.

`GET /api/v1/agents/activity?provider=codex|claude|opencode|orchester` returns schema version 1, observation time, source availability and up to 24 session summaries per provider. Each summary has an opaque ID, provider, source (`external` or `managed`), title, project, update time, activity state, preview and truncation flag.

`GET /api/v1/agents/activity/{id}` returns the summary and up to 100 recent entries. Entries have a stable ID, kind (`user`, `assistant`, `tool`, `result`, `status`), text and optional tool name. Unknown IDs return 404; invalid providers return 422; unsupported storage returns an explicit unavailable source, not an empty-success assertion. Responses are `no-store` and use the existing local authenticated session. No client-supplied file path is accepted.

While the reader is visible, it refreshes at two-second intervals, with one request at a time and cancellation on navigation. Reads are bounded and cached; no process is launched to inspect sessions. Managed runs use the existing runtime event registry. Codex and Claude use local JSONL transcripts, OpenCode uses its local SQLite database in read-only/query-only mode. Existing environment overrides for provider data roots are respected. Storage records are treated as data and rendered as text; system/developer instructions, hidden reasoning, encrypted payloads and credentials/configuration files are excluded.

Activity labels follow transcript lifecycle markers where available. A fresh incomplete turn may be active; old unclosed records become unknown instead of staying running indefinitely. A provider process existing alone never proves a particular session is active. Content updates are limited by what the provider has written to disk; sessions on a different machine are not available from this local reader.

## Design

Reuse the current dark surfaces, pink accent, system font, focus rings and spacing tokens. A compact session list and a wide transcript occupy the main pane; on narrow widths they stack. Replies stay visually quiet; tool inputs/results are expandable. Current session identity and a small refresh indicator provide orientation without additional dashboard cards.

The rail begins directly with New chat and search. Both the product/workspace header and the duplicate account/runtime footer are removed, as requested in the updated Codex reference. Pinned, agents and recent conversations share one scroll area. Agent rows omit redundant vendor names, group totals and window counters; the existing global settings and pet actions remain available.

The machine-readable HTTP contract is in [agent-activity.openapi.yaml](../agent-activity.openapi.yaml). The first implementation supports current Codex/Claude JSONL and OpenCode SQLite storage. Legacy OpenCode JSON directories and remote-machine sessions are outside this local reader.

## Verification and delivery

Verify the three formats with isolated records, append a live update, complete a turn, reject malformed/oversized records, handle removed/locked files and avoid symlink traversal. Check actual local provider storage without changing it, the authenticated HTTP boundary, and rendered interaction. Split contract/backend, frontend and desktop delivery into commits; push after verification and refresh the local installation.

Provider format references: [Claude transcript hook fields](https://code.claude.com/docs/en/hooks#common-input-fields), [OpenCode local storage](https://opencode.ai/docs/troubleshooting/). Field shapes are also checked against this machine's installed provider data; these are compatibility readers, not a provider execution API.
