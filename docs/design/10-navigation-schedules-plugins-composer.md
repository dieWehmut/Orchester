# Navigation, local schedules, plugins and the reference composer

The compact navigation is Home, Schedules, Plugins at the top, with Pet, Theme,
Settings at the bottom. It remains available on management pages. The bottom
panel and its menu/settings entries are retired. The transcript composer follows
the supplied Codex reference: a large rounded text surface, attachment and
permission controls on the left, model/effort, microphone and a pink send/stop
button on the right. Controls keep their real state and existing permissions.

## Local management contract

All paths are under `/api/v1`, use the existing desktop session, CSRF and origin
boundary, return `Cache-Control: no-store`, and use the existing JSON error body
(`error`, `code`, `request_id`, `retryable`). Invalid input is 422, malformed JSON
400, missing records 404, conflicting active operations 409, unavailable local
storage/runtime 503. Names are bounded; errors never echo package output or
configuration secrets. Management is unavailable in the static demo.

| Method | Resource | Result |
| --- | --- | --- |
| GET | `/schedules` | 200 `{schema_version:1,schedules:[Schedule]}` |
| POST | `/schedules` | 201 `Schedule` |
| PUT | `/schedules/{id}` | 200 `Schedule` (replace editable fields) |
| DELETE | `/schedules/{id}` | 204; active tasks cannot be deleted |
| POST | `/schedules/{id}/runs` | 202 `ScheduleRun`; runs even if paused |
| GET | `/plugins` | 200 `{schema_version:1,plugins:[Plugin]}` |
| POST | `/plugins` | 201 catalog after installing `{name}` |
| DELETE | `/plugins/{name}` | 200 catalog after managed removal |

Schedule input is `{name,prompt,enabled,first_run_at,interval_seconds}`. Times
are Unix seconds. Name is 1–80 characters, prompt 1–8000 characters, first time
is positive, interval is null (once) or 60–31536000 seconds. At most 100 tasks
per workspace. A task stores the current model selection when created/edited;
credentials remain in the existing configuration. Output adds `id`, `next_run_at`,
`created_at`, `updated_at`, and `history` (newest 20 runs). Run records contain
`id`, nullable `run_id`, `started_at`, nullable `finished_at`, `status`
(`running`, `succeeded`, `failed`, `cancelled`, `interrupted`) and nullable
`error_code`. Run details/cancellation use the existing run endpoints.

Schedules persist in the local home state database, partitioned by workspace.
The app must remain running (including in the tray). A due task runs once after
restart; missed intervals do not produce a burst. Claims are transactional and
leased across processes; a task cannot overlap itself, even with manual starts.
Expired executions become interrupted, not successful. Closing the runtime
cancels its executions and records interruption. Schedule edits while running
are rejected. One-shot tasks disable after a claim; recurring next times advance
from the actual claim time. Pausing does not cancel an already-running task.

Plugin output includes `name`, `display_name`, `package_name`, `version`,
`installed`, `origin` (managed/project/null), and `removable`. The catalog offers
the shipped Claude, Codex and OpenCode adapters plus discovered plugins.
Installation uses the CLI's pinned `@orchester/<name>@<core-version>` npm package
flow with scripts disabled, strict archive validation and receipts. Removal is
limited to receipt-owned managed packages; project packages are read-only.
Changes refresh discovery for subsequent runs and the UI without restarting.

## Delivery and evidence

1. Shared plugin installation/removal, API contract and backend tests.
2. Persistent scheduler, execution integration and restart/concurrency tests.
3. Management pages, navigation and removal of the bottom panel.
4. Reference composer, interaction/accessibility checks and rendered validation.
5. Production packaging, payload checks, reinstall and installed runtime smoke.

Commit these scopes separately. Integrate and push only after final validation.
Preserve the original checkout's unrelated unfinished merge and user settings.
