# Native conversation rendering

The agent reader should read like the source conversation. The supplied Codex screenshot is the visual contract: a right-aligned user bubble, unboxed assistant prose, a collapsed work disclosure above the answer, Markdown lists and inline code, and a quiet copy action below it.

The conversation uses a centered 48rem reading measure, 16px system text with a 1.75 line height, and generous separation between turns. The Codex dark palette is #181818 (surface), #ededed (text), #919191 (secondary text), #303030 (code), and #843d63 (user bubble). Existing application themes continue to apply. Claude Code and OpenCode keep neutral prompt surfaces and tool disclosures; all providers use the existing safe Markdown renderer rather than escaped Markdown paragraphs.

## Data contract

The existing authenticated, read-only activity endpoints retain schema version 1. Entries optionally include `timestamp` (Unix milliseconds), `phase` (`commentary` or `final`), and an opaque `turn_id`. Detail responses optionally include `turns`, containing opaque IDs and known `started_at` / `completed_at` times. Missing metadata remains missing; the client must not invent elapsed time or claim a task completed from an assistant message alone.

Only provider-visible message metadata is used. Codex lifecycle events supply turn timing and message phase; Claude and OpenCode supply timestamps and explicit completion phases where available. Hidden reasoning remains excluded. Old servers and partial transcript excerpts continue to render without metadata. Turn metadata is bounded to turns referenced by the retained entries.

## Interaction and acceptance

- User messages, work records and final answers appear in source order. Consecutive prompts are retained.
- Known commentary and tool calls appear inside a keyboard-operable work disclosure. The latest visible progress remains readable while work is active. Final answers remain outside the disclosure.
- The elapsed label appears only for a valid source start/end pair. Incomplete excerpts use a generic work label.
- Markdown lists, headings, tables, code and safe external links render using shared primitives. Raw HTML is inert. Codex citation bookkeeping is excluded from presentation without changing copied message content.
- Copy actions copy the original answer. Disclosure state and scroll position survive polling; switching tasks resets per-task presentation state.
- Validate parsing metadata, grouping with partial/legacy data, rendered Markdown, update stability, responsive overflow and dark/light presentation before packaging and local installation.
