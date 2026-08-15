# Persistent Full-Screen Chat Design

Status: approved layout A

## Context

The current TTY flow uses `run_home_tui` as a one-action launcher. It leaves
the alternate screen before the model result is rendered, then starts a line
prompt. That makes the process exit after one result in the old flow and makes
the fixed-composer transcript impossible to keep on screen.

## Goals

- Keep one alternate-screen session open for the self-agent conversation.
- Render user prompts, assistant text, tool/status lines, usage, and errors in
  a bounded transcript viewport.
- Keep a fixed composer and status row at the bottom of every frame.
- Show a `Creating...` state while a model request is in flight.
- Preserve existing slash command parsing, `/quit`, Esc, Ctrl+C, model status,
  and safe terminal text handling.
- Keep non-TTY line mode unchanged apart from its existing multi-turn loop.

## Non-goals

- Automatic conversational resume between separate durable runs. Each prompt
  continues to use the existing self-agent runtime contract.
- Rebuilding delegate-agent or plugin UIs inside the new transcript frame.
  Commands that need their own terminal are allowed to leave and re-enter the
  chat session with transcript state preserved.

## Architecture

`run_terminal_interactive` will own a `ChatSession` for the lifetime of the
self-agent TTY conversation. `ChatSession` owns `TerminalSession` and
`FramePresenter`, so the alternate screen is not re-entered between turns.

The interactive module will expose a small crate-local view/state boundary:

- `TranscriptEntry`: role plus sanitized text for one visible transcript item.
- `ChatHomeView`: existing home fields plus transcript and busy state.
- `ChatSession::present`: renders a bounded frame with the composer reserved at
  the bottom.
- `handle_chat_key`: reuses existing key semantics and command parsing without
  coupling the model runtime to crossterm details.

The main loop appends the user entry before submitting, presents the busy
frame, awaits `SelfAgentHost::submit`, then appends the captured assistant
rendering or an error entry and presents again. Model output is captured into
memory while the alternate screen is active; it is never written directly to
stdout and therefore cannot corrupt the frame.

## Rendering

The frame keeps the existing startup panel on the first empty view. Once a
transcript exists, the available rows are divided into a scrollable transcript
area, a separator, one or more composer rows, and one status row. Transcript
lines are sanitized before width calculation and clipped from the bottom so
the newest response remains visible. Frames never emit a trailing newline that
would scroll a viewport-sized terminal.

## Commands and errors

`/help`, `/model`, `/status`, `/permissions`, `/resume`, and plugin reports are
rendered as transcript/status entries when they can be captured. Native agent
launches and interactive credential prompts may temporarily leave the
alternate screen; the state is retained and the chat frame is restored after
the nested command returns. `/quit`, empty Esc, EOF, and Ctrl+C exit cleanly.

## Verification

- Renderer tests cover assistant/user transcript visibility, busy state,
  composer placement, width/height clipping, and control-character safety.
- Existing CLI loopback tests continue to cover multiple non-TTY model turns.
- A real local TTY smoke test sends two prompts and `/quit`, confirming two
  rendered responses and a single process lifetime.
