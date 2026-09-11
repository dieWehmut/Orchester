# Web run live-stream integration plan

## Goal

Connect the existing WebUI run submission API to the existing run socket client so a submitted run can hydrate its current snapshot, consume server-issued events, recover from a resync request, and release the connection when the app stops.

## Boundaries

- Keep the existing `StartRunResponse`, snapshot, and stream frame protocol unchanged.
- Use only the opaque `events_url` returned by the server; never construct a ticket URL from local runtime details.
- Keep deterministic event projection inside `@orchester/ereignis`.
- Do not add approval decisions or backend run execution in this batch.

## Tasks

1. Add store tests for snapshot hydration, stream startup, event delivery, and shutdown.
2. Inject a run-socket factory into the run store for production and deterministic tests.
3. Start the stream only after the start response and snapshot belong to the active run.
4. Refresh the snapshot when the server explicitly requests resynchronization.
5. Close the active stream during cancel, reset, and application shutdown.
6. Run focused WebUI tests, typecheck, and build before integration.
