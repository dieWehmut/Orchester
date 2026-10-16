# Agent navigation and conversation alignment

Reference: the user's four screenshots on 2026-10-16. Desktop version stays 0.1.8.

## Visual and interaction contract

Use one sidebar and one conversation pane for every external provider. The sidebar has a compact provider switcher, New chat, Pinned, collapsible Projects, and Recent. Rows are one line; selection uses a quiet rounded background, running/unread state uses a small trailing indicator. Preview paragraphs, dates and completed badges do not consume row space. Pin and project expansion controls work and persist locally. New chat returns to the existing composer.

The palette follows the reference: conversation #181818, sidebar #1b1c19, selection #2b2d2a, primary text #ececec, secondary text #969696, accent #ee78b1. Use existing system typography, 14–16 px text, a 288 px preferred sidebar and 36 px rows. Light mode uses the existing semantic theme colors. Keep a centered readable conversation, right-aligned user bubbles and plain assistant prose. All providers use the same rendering.

Work disclosures preserve chronological commentary and compact named tool actions. A tool's result belongs inside that action when the source supplies a call ID. Source timestamps produce date separators and elapsed labels; unknown times remain unknown. Refresh preserves task selection, expanded work and reader scroll position. Markdown file references render as labelled references instead of literal syntax; activating one offers its path without automatically opening or executing local files.

The activity navigation lists Codex, Claude Code and OpenCode. Orchester is absent; existing managed-run API compatibility remains available to other consumers.

## Data and verification

Use provider-owned titles where available, with a first meaningful prompt fallback. Codex metadata is read-only and optional; metadata paths must remain inside the configured transcript root. Directory fallback searches recent dated directories first. Optional project identity and source pin hints are additive to schema version 1. Pins/read markers in this app never change the source application's data.

Verify metadata fallback and root boundaries, stable tool/result association, provider switching, pin/project/recent navigation, unread clearing, Markdown references, disclosure preservation and scroll following. Inspect real rendered desktop/narrow layouts in both themes. Then build and reinstall the unchanged 0.1.8, preserve existing data, commit the acceptance record and push only after verification.
