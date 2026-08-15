# Live Chat Output Design

> Status: approved extension of the persistent full-screen chat design.

## Goal

Make the interactive self-agent behave like a terminal chat client: the
composer remains usable while a model call is running, visible progress and
partial assistant text are rendered in the same alternate screen, slash
commands provide immediate feedback, and long transcript output remains
inspectable.

## Scope

The existing governed self-agent loop remains the only owner of model calls,
tool execution, approvals, and durable state. This change adds an optional
model-event channel rather than a second model path. Providers that cannot
stream still emit one final text event, preserving the current behavior.

The TTY state gains a bounded scroll offset and a visible pending-action
indicator. PageUp/PageDown/Home/End navigate the transcript without changing
the composer. Slash palette rendering is composed with the transcript and
busy indicator instead of replacing them. Read-only workspace commands can be
run through a separate host while a model turn is pending; mutating commands
remain ordered behind the active turn and show a queued marker.

## Model event boundary

`LanguageModel` keeps `complete` as the compatibility entry point and gains an
event-aware default method. The Responses adapter requests SSE only for the
event-aware path, parses bounded `response.output_text.delta` events, and
returns the normal final `ModelResponse` after the `response.completed` event.
Malformed or oversized events fail closed as protocol errors. The default
implementation calls `complete` and emits the final assistant text once.

The self-agent loop forwards events from every model call through an
unbounded, cancellation-aware channel. Tool-call and durable execution logic
is unchanged. The TTY consumes text-delta events and updates one assistant
transcript entry; the final outcome still supplies usage, tool trace, and
durable completion metadata. Non-TTY mode ignores deltas and keeps its final
rendering.

## Retry and error behavior

The Responses adapter retries at most once for transport errors, timeouts, and
HTTP 408, 425, or 5xx responses. Authentication, forbidden, protocol, and
rate-limit responses are not retried. A second failure remains visible with a
redacted actionable error; credentials, request bodies, and provider response
bodies never enter the transcript or logs.

## Verification

- Renderer tests cover slash palette plus busy state, queued-action status,
  wrapping, and transcript scroll bounds.
- Responses tests cover SSE deltas, malformed/oversized events, cancellation,
  and bounded retries for timeout and 503 responses.
- Runtime tests prove event forwarding does not alter tool-call sequencing.
- Existing line-mode tests remain final-response based.
- A Windows ConPTY smoke test (when the host provides ConPTY tooling) sends a
  delayed model turn, types a second prompt and read-only command, observes
  multiple progress frames, and confirms both results before `/quit`.
