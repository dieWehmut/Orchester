# Persistent Command Overlays and Theme Picker

> Status: approved by the existing persistent-chat direction and the supplied
> Codex/Claude/OpenCode reference screenshots.

## Goal

Keep the Orchester startup panel visible for the lifetime of a fullscreen
session. Slash commands must open an in-session selection surface instead of
leaving and re-entering the alternate screen. `/model`, `/permissions`,
`/resume`, `/status`, `/config`, and `/plugins` must show a stable selectable
view or captured result; `/theme` must provide a live-preview theme picker.

## Invariants

1. Exactly one `ChatSession` owns the alternate screen from launch until quit.
2. Every frame renders the startup panel/header first, followed by transcript,
   overlay content, composer, and status line within the terminal viewport.
3. Opening, moving within, confirming, and cancelling a command overlay never
   writes directly to stdout or recreates terminal modes.
4. `Esc` cancels an overlay without changing the selected model, permission, or
   theme. `Enter` confirms a selection and returns to the same chat frame with
   a visible result entry.
5. Overlay text is sanitized and bounded; credentials, raw provider bodies,
   and internal identifiers are never rendered.

## State and data flow

`TerminalChatState` owns an optional `CommandOverlay` in addition to the
transcript and composer. The overlay contains a title, prompt/help text, a
bounded list of rows, selected index, and an optional preview/result. Keyboard
events are routed to the overlay first. A command action loads its existing
service projection into memory, then the renderer displays it in the same
frame. Read-only commands are immediately rendered; mutations keep the
existing governed ordering and report their completion in the transcript.

The startup panel remains a pure renderer input, so it is not duplicated in
command implementations. The frame presenter redraws the complete bounded
viewport, which removes stale rows without leaving the alternate screen.

## Theme behavior

`Theme` is a small, serializable palette selected from built-in names. It
controls the Orchester accent, selection, warning, and dim colors used by the
fullscreen renderer. `/theme` opens a list with a code-diff preview; moving the
cursor applies a temporary palette, `Esc` restores the snapshot, and `Enter`
persists the selected name in the user TUI configuration. Unknown or malformed
stored names fall back to `default` without failing startup.

## Verification

- Parser tests prove every requested slash command, including `/theme`, maps
  to a deterministic action and selected row.
- Renderer tests assert the startup panel marker is present before and after
  each overlay, that overlays and composer coexist, and that frames stay
  within a fixed height.
- State tests prove overlay navigation, Enter confirmation, Esc restoration,
  and command result insertion without a session drop.
- Theme tests prove live preview, cancel restoration, persistence format, and
  fallback for an unknown name.
- The focused konsole suite, formatting, clippy, package installation, and a
  Windows interactive smoke run are required before the `v0.1.1` release.
