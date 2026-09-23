# Orchester UI implementation plan

> Execution index for the Codex-informed UI design in
> [`02-orchester-ui-design.md`](./02-orchester-ui-design.md).
>
> Rules: every task starts with a failing test, ends green, is committed on its
> own, and leaves the workspace typechecking. Tasks are ordered so that each one
> is independently revertable.

**Invariant:** no task may break the existing public token names, component
exports, DTOs, or HTTP contracts. All new work is additive until the migration
wave, which lands with an updated snapshot.

## Wave U0 — token foundation

- [x] U0-01: Add the neutral ramp (`--gray-fixed-*`, `--gray-*`) to `packages/design/src/tokens.css` and assert every step exists in both themes.
- [x] U0-02: Add the six accent ramps and their alpha variants; assert the `codex` scheme resolves to `--blue-300` in dark and `--blue-500` in light.
- [x] U0-03: Add the `--alpha-*` scale over `--alpha-base`; assert the base flips with the theme.
- [x] U0-04: Add the L1 intent matrix (`--color-intent-*-{text,surface,border,solid}`, `--color-text-*`, `--color-surface-*`, `--color-border-*`); assert every intent has all four treatments in both themes.
- [x] U0-05: Add the heading and text type scales with bound line-height/tracking/weight; assert monotonicity of size across each scale.
- [x] U0-06: Add `--radius-*-base` and `--corner-radius-scale`, deriving every `--radius-*`; assert `1.25` is the default and that `--radius-lg` resolves to `0.78125rem`.
- [x] U0-07: Add `--border-width-hairline` and the elevation geometry/shadow tokens; assert `--elevation-stroke` uses the hairline.
- [x] U0-08: Add the motion curves and `--ease-*`; assert the reduced-motion override still collapses durations.
- [x] U0-09: Add the shell layout tokens (rail clamp, inspector clamp, tab strip, bottom panel, composer) and assert the rail clamp never leaves less than 360px for the transcript.
- [x] U0-10: Add the density axis (`--density-row-height`, `--density-gap`) behind `[data-density]`.
- [x] U0-11: Export the new token names from a typed `TOKEN_GROUPS` map for tests and tooling.
- [x] U0-12: Update the token snapshot and commit wave U0.

## Wave U1 — theme and appearance plumbing

- [x] U1-01: Add `data-intensity` (`calm` | `vivid`) as a third appearance axis in `theme.ts`, the bootstrap script, and storage.
- [x] U1-02: Add `data-reduced-motion` as an explicit, overridable root attribute with `true`/`false`/system semantics.
- [x] U1-03: Add `data-orchester-surface` and `data-orchester-os` root attributes with detection helpers.
- [x] U1-04: Extend the no-flash bootstrap script to set all four axes before first paint, and assert it stays dependency-free.
- [x] U1-05: Extend `useAppearance` and the settings view to expose intensity and reduced motion.

## Wave U2 — shell restructure

- [x] U2-01: Introduce `AppShell` owning the region state attributes; port the current three-column grid onto it without visual change.
- [x] U2-02: Replace the three ad-hoc width variables with the rail/inspector clamps; add drag-to-resize with keyboard support and persistence.
- [x] U2-03: Add the collapsible bottom panel with its own tab mechanism.
- [x] U2-04: Add the unified tab strip with open/close/reorder/keyboard cycling and overflow fades.
- [x] U2-05: Persist open tabs and panel widths through the existing settings/state path.

## Wave U3 — transcript

- [x] U3-01: Add explicit scroll state (`data-can-scroll-up/down`) and the top fade.
- [x] U3-02: Add the scroll-to-bottom button with its unread indicator.
- [x] U3-03: Virtualise turns.
- [x] U3-04: Add the floating message navigation rail with scrubbing.
- [x] U3-05: Restyle tool-invocation cards by `call_id`, with collapsed summary and expandable body.
- [x] U3-06: Add the reasoning disclosure.
- [x] U3-07: Add word-arrival streaming that does not reflow ancestors.

## Wave U4 — composer and plan strip

- [x] U4-01: Restructure the composer into top tray / input / footer with `data-composer-state`.
- [x] U4-02: Add the approval-preset footer dropdown with the three presets and the risky-combination dialog.
- [x] U4-03: Make model/effort/preset run-scoped rather than global.
- [x] U4-04: Add drag-and-drop file affordance (`data-composer-drag-active`).
- [x] U4-05: Add the `/` command palette with explicit empty and loading states.
- [x] U4-06: Add the plan strip with segmented progress and the blocked/needs-input treatment.

## Wave U5 — inspector

- [x] U5-01: Add the `Review` tab with change filters (all/staged/unstaged/branch/last turn).
- [x] U5-02: Order the file tree identically to the diff list and add expand/collapse navigation.
- [x] U5-03: Add the line-wrap toggle for diffs.
- [x] U5-04: Add the `Approvals` queue with risk summary, scoped choices and the superseded state.
  - `Allow for run` renders disabled: the protocol carries `approved`/`denied`
    only, and a run-scoped grant needs a scope the runtime does not have yet.
- [x] U5-05: Add terminal placement preference (inspector vs bottom panel).

## Wave U6 — desktop chrome

- [x] U6-01: Add the macOS overlay titlebar path with correct traffic-light inset.
- [x] U6-02: Add Windows caption buttons at OS metrics with the Snap-Layouts hit region.
- [x] U6-03: Fall back to opaque surfaces where translucency is unavailable.
- [x] U6-04: Wire `⌘/Ctrl+W` to close the active tab and confirm before closing with an active run.

## Wave U7 — settings, keyboard, a11y

- [x] U7-01: Add settings search across all panels.
- [x] U7-02: Add the keyboard-shortcut editor with keypress search and reset-all.
- [x] U7-03: Add a shortcut registry that every component registers into, so the editor cannot drift from reality.
- [x] U7-04: Audit and fix the accessibility contract in §7 of the design spec.
- [x] U7-05: Add contrast assertions for every text/surface and border/surface pair.

## Wave U8 — migration and cleanup

- [x] U8-01: Re-point the legacy `--color-bg-*`, `--color-border-*`, `--color-text-*` tokens at L1 and update the snapshot.
  - `packages/design/src/tokens.css` carries the legacy names as `var(--color-surface-*)`
    and `var(--color-text-*)` aliases, and the theme snapshot holds the mapping.
- [x] U8-02: Remove the two obsolete `amber` snapshots.
  - The accent schemes are `codex`, `violet`, `teal` and `rose`; the snapshot's
    `dark-amber` and `light-amber` entries are gone.
- [x] U8-03: Sweep components for L0/L1 leakage and bare `px` spacing.
  - `packages/design/test/component-migration.test.ts` holds the sweep as a
    standing rule over both component layers.
- [x] U8-04: Replace remaining hard-coded English strings with locale keys.
  - `apps/web/test/locale-sweep.test.ts` holds the sweep as a standing rule, and
    the shortcut registry and the composer's command list carry keys rather than
    words so the surface resolves them.
- [x] U8-05: Re-run the full gate: `pnpm typecheck`, `pnpm test`, both builds.
  - `pnpm typecheck` is clean across all seven projects; `pnpm test` is green at
    25 tooling + 89 protokoll + 264 design + 31 website + 20 ereignis + 476 web
    + 1 desktop-security tests; the web and website builds both succeed.

## Wave U9 - reference alignment against the Codex desktop app

The three reference screenshots (the sidebar, its account menu, and the
appearance screen) were compared against the shipped surfaces. What the
reference has and Orchester did not is additive, so this wave follows the same
rules as U0-U8 and leaves the token and DTO contracts alone.

- [x] U9-01: Give the rail the reference's anatomy: a product row that owns its
  disclosure, a heading that folds each list, and an account row that opens a
  menu instead of sitting beside a gear.
  - `apps/web/test/app-rail.test.ts` pins the row order, the disclosure, and the
    account menu's intents.
- [x] U9-02: Make the product row's disclosure fold the lists it governs, rather
  than leaving a chevron that draws and does nothing.
  - The fold is a column layout rather than fixed grid tracks, so removing the
    lists cannot slide the account row into a list's track.
- [x] U9-03: Head the settings screen with the way back and print each resolved
  colour's hex beside its swatch.
  - `apps/web/test/theme-colours.test.ts` checks the mirrored values against
    `tokens.css`, so the readout cannot drift from the stylesheet.
- [x] U9-04: Teach each account-menu row the chord that performs it, read from
  the live shortcut registry rather than written out.
  - The registry is what the settings editor rebinds, so a printed default would
    keep teaching a key that no longer does anything.
- [x] U9-05: Carry the reference's header actions: a search glyph that narrows
  the column, and a bell that reports how much is waiting and opens it.
  - `apps/web/test/session-rail-filter.test.ts` pins the filter, including the
    difference between "no sessions" and "nothing matched".
- [x] U9-06: Answer the companion chord the reference teaches on its menu row.
  - The reference spells it `Alt+Win+P`; `Win` is the OS chord and the registry
    refuses the foreign modifier, so the shell binds `Ctrl+Alt+P` - the same
    gesture one key over - and the row prints it from the live registry.
- [x] U9-07: Re-run the full gate: `pnpm typecheck`, `pnpm test`, both builds,
  the stack manifest, and the desktop shell's `cargo check`.
  - `pnpm typecheck` is clean; `pnpm test` is green at 543 web + 264 design
    + 31 website + 27 ereignis + protokoll + tooling tests; the web and website
    builds succeed; `pnpm stack:verify` matches; the shell checks clean on the
    x64 toolchain this ARM64 host uses.

## Wave U10 - the appearance screen the reference draws

The reference's appearance screen gives each theme its own card rather than one
accent for the product, so this wave is the first that changes an appearance
*model* rather than only a surface. It stays additive: `data-color-scheme` is
still the attribute the stylesheet matches on and still carries one scheme.

- [x] U10-01: Let each theme carry its own accent hue.
  - `packages/design/test/appearance-scheme-per-mode.test.ts` pins the pair, the
    attribute following the theme in force, the legacy single key still reading
    as "both themes", and the bootstrap script painting the half belonging to
    the theme it resolved.
- [x] U10-02: Draw one card per theme, headed 浅色主题 / 深色主题, with the
  reference's own actions (导入, 复制主题), the `Aa` swatch, the theme's choice,
  and 强调色 / 背景 / 前景 reporting what *that* theme resolves to.
  - `apps/web/test/appearance-theme-sections.test.ts` pins the cards, the
    per-theme readouts, and the copy payload; `theme-colours.test.ts` walks
    both halves of every scheme out of `tokens.css`, because one scheme is two
    colours and the old single-row swatch had drifted from the stylesheet.
- [x] U10-03: Mark the chosen theme card with the ring alone.
  - The badge was a second answer to the question the ring had already asked,
    and it covered the miniature it sat on. The ring is a box shadow rather than
    an outline: `outline` belongs to the focus token.
- [x] U10-04: Bare the code preview and give each pane the rail the reference
  draws, in the colour of the change that pane carries.
  - `CodePreview`'s head is now optional, so the settings screen shows the two
    panes directly under the cards as the reference does.
- [x] U10-05: Re-run the gate: `pnpm typecheck`, `pnpm test`, both builds, and
  the stack manifest.
  - `pnpm typecheck` is clean across all seven projects; `pnpm test` is green at
    274 design + 549 web + the rest; the web build and `pnpm stack:verify` pass.

**Not taken from the reference.** The reference's account menu carries usage,
invites and sign-out, and its appearance screen offers a hex picker per colour.
Orchester has no account to sign out of, no usage meter to report, and a hex
picker beside a theme choice would be a second theme editor - so those rows are
left out rather than faked.

## Wave U11 - the rail, as the reference seats it

The reference's sidebar is one column of destinations: the product row, one new
chat row, then headed lists whose headings are the controls that fold them. It
offers each action once and each list one name. This wave removes the places
where Orchester offered two.

- [x] U11-01: Seat new chat as a rail row rather than a boxed block.
  - `apps/web/test/app-rail.test.ts` pins the row marker and that the primary
    section carries no `AppButton`: the row above a list of rows is a row.
- [x] U11-02: End the account row at the identity, with settings in the menu it
  opens rather than a gear beside it.
  - `app-rail.test.ts` and `workspace-rail.test.ts` now reach the settings route
    through the account menu, which also carries the chord.
- [x] U11-03: Let the rail heading name each list, once.
  - `apps/web/test/session-rail-anatomy.test.ts` pins that the sessions list
    drops its own uppercase title and its second new-session button while
    keeping an accessible name; `agent-fleet-panel.test.ts` pins the same for
    the fleet, which keeps its stream badge and count because those belong to
    the list rather than to the column.
- [x] U11-04: Re-run the gate: `pnpm typecheck`, `pnpm test`, both builds,
  `pnpm stack:verify`.

**Still not taken from the reference.** The reference's account menu also
carries remaining usage, invites and sign-out. Orchester runs locally with no
account, no meter and nothing to sign out of, so those rows stay out rather than
being faked; the menu carries what it can answer - the companion and settings,
both with the chord the live registry holds.

## Wave U12 - the transcript's two idioms

A transcript is two things at once, and the reference only draws one of them.
The conversation is a question and an answer, read as prose; the run's own
record is tool calls, approvals and validations, which is a ledger with numbers
on it. §4.4 asked for "role marker · content · actions" and the surface was
drawing everything as an event card, so this wave separates the two.

- [x] U12-01: Render messages and reasoning in the conversation's idiom and
  everything else in the run's.
  - `apps/web/test/chat-message-anatomy.test.ts` pins the shapes: an answer is
    prose with no ledger number, the reader's own turn is a bubble at the end of
    the measure, and a tool call keeps its card *and* its sequence - the number
    belongs to the record, not to the sentence.
- [x] U12-02: Offer the answer as a copy, and say when it has been taken.
  - `MessageActions` is the whole action set, because it is the only one this
    product can answer: the runtime cannot re-run a turn and keeps no feedback
    channel, and a button that does nothing is worse than a button that is not
    there. The row is drawn on hover or focus but never removed from the tree.
- [x] U12-03: Read a stored session as the conversation it was.
  - `SessionTranscript` drops the uppercase `Request`/`Result` headings: the
    role is the shape, as it is in the reference. `workspace-view.test.ts` keeps
    pinning that the stored answer reaches the pane.
- [x] U12-04: Count the reasoning in the reader's own language.
  - The disclosure had `{{ count }} characters` typed into its template, which
    the locale sweep could not see through the interpolation.
- [x] U12-05: Re-run the gate: `pnpm typecheck`, `pnpm test`, both builds,
  `pnpm stack:verify`.

**What the reference has and this wave cannot yet draw.** The reference opens a
conversation with the reader's question, and a resumed Orchester transcript has
no question in it: `projectConversation` only ever projects assistant output,
because the runtime writes no user-message event when a run starts. The bubble
shape therefore renders from the `role` the timeline item already carries - and
`railMarks`, the message navigation rail, has nothing to list in production
until the journal carries the user's turn. That is a runtime and protocol change
rather than a surface one, and it is the next thing this objective needs.

## Wave U13 - the journal carries the reader's own turn

U12 could draw the reader's bubble but had nothing to draw it from: the runtime
journalled the model's answers and never the prompt, because from its side the
prompt is the request rather than an event. That made the transcript open with
an answer to nothing and left the message rail with nothing to list - so this
wave is the first in the objective that crosses into the runtime and the wire
contract, and it keeps that contract's habits: a new kind in both mirrors, the
same redaction as every other text, and a route test that proves the order.

- [x] U13-01: Add `user_message` to the UI event vocabulary on both sides.
  - `kisten/protokoll/src/ui.rs` and `packages/protokoll/src/ui.ts`; the Rust
    kind is sanitised like the model's text, because a prompt can name a path
    and the browser-facing stream is where that policy has to hold. The mirror
    test's pinned count moved with it.
- [x] U13-02: Journal the reader's turn when a run starts.
  - `start_run_handler` appends it before the runtime is spawned, so sequence 1
    is what was asked; `run_registry` treats it as the run having begun, which
    is what the API already answered.
  - `kisten/netz/tests/run_routes.rs` starts a run against a temporary workspace
    and asserts the snapshot opens with `user_message` and the prompt's text.
- [x] U13-03: Project it into the conversation.
  - `projectConversation` maps it to a settled message with `role: 'user'`, and
    `timelineKindKey` gives it a stable key. The surface needed no change: U12
    already draws a message by its role, and `railMarks` already lists user
    turns - it had simply never been given one.
  - `packages/ereignis/test/model/conversation.test.ts` pins the replayed
    conversation opening with the question.
- [x] U13-04: Re-run the gate: `pnpm typecheck`, the frontend and Rust suites,
  both builds, `pnpm stack:verify`.

**What this unblocks.** The message navigation rail now has destinations, and a
transcript restored after a reload shows what was asked as well as what was
answered. The reference's remaining gap on this surface is the composer row
(model and effort, the send control) rather than the conversation itself.

## Wave U14 - the composer's field

The reference's composer is one rounded field: a placeholder that names it, the
controls it needs at the trailing edge, and a round action that points the way it
sends. Orchester's was a card with a heading over it, a second bordered box
inside it, and a wide labelled button - three rows of chrome the reader pays for
on every prompt.

- [x] U14-01: Let the field own the box.
  - `apps/web/test/composer-field-anatomy.test.ts` pins that the prompt and its
    control row sit in one `data-composer-field`, and that the context row stays
    outside it: which workspace and model a run will use is a fact about the run,
    not a control in the field.
- [x] U14-02: Name the field with its placeholder, as the reference does, and
  keep the visible label as the accessible name instead of losing it.
- [x] U14-03: Make the action a shape - a round send that points the way the
  prompt goes, a square stop while a run is in flight - with its word kept as the
  accessible name.
- [x] U14-04: Show the tally only within a tenth of the limit, because the
  reference prints none and a number on every prompt is a row for information
  that matters at the end.
  - The prompt also gives up its resize grip: the field already grows, and the
    grip sat where the send control is.
- [x] U14-05: Re-run the gate: `pnpm typecheck`, the frontend suites, both
  builds, `pnpm stack:verify`.

**Not taken from the reference.** Its composer also carries a microphone and a
model picker. Orchester has no voice input, and its model and effort are read
from the runtime's catalog rather than chosen in the field, so the readout stays
where it is instead of pretending to be a picker.

## Wave U15 - the answer's own markup

A coding agent writes in markdown, and the reference renders it: the answer in
its screenshot has a linked heading, inline code chips and a mono hash. §4.4
asked for markdown with `CodeBlock` and the surface had been drawing every
answer as one pre-wrapped string, so a fenced block arrived as backticks and a
list as hyphens.

- [x] U15-01: Parse the subset an answer actually uses, into tokens.
  - `packages/design/src/markdown.ts` produces a token tree rather than HTML:
    paragraphs, headings, fenced code, both list kinds, inline code, bold and
    links. `packages/design/test/markdown.test.ts` pins the reading, including
    the two that matter for safety - raw markup stays text, and a link that is
    not an address is refused rather than sanitised.
- [x] U15-02: Render the tokens with elements and never with markup.
  - `MarkdownText` and `MarkdownSpans` draw every block as a Vue element and
    hold no string of markup to interpolate; the test asserts the source has no
    `v-html`, which is what makes text an agent wrote safe to render. Links leave
    the page with `rel="noopener noreferrer"` and say so out loud through the
    caller's words. Every heading is drawn at one level, with the written level
    kept on the element: an `h1` inside a transcript would outrank the page.
- [x] U15-03: Show it on the two surfaces that carry an answer.
  - The live transcript renders markdown **once the answer has settled** and
    keeps the text literal while it arrives - a half-written fence is not a code
    block yet, and formatting line by line would flicker the paragraph apart
    under the reader. A stored session renders it directly.
  - `apps/web/test/chat-message-anatomy.test.ts` pins both the settled reading
    and the arriving one, and that the reader's own turn is still shown exactly
    as they typed it: their words are not the agent's markup to interpret.
- [x] U15-04: Re-run the gate: `pnpm typecheck`, the frontend suites, both
  builds, `pnpm stack:verify`.

**Kept out of the subset.** Tables, images, nested lists and raw HTML are left
as the text they were written as. Each of them is a rendering an answer can live
without; none of them is worth a parser that can be talked into producing an
element.

## Wave U16 - the row of actions under a message

The reference hangs more than copying off a message. Two of its actions cannot
be answered here - the runtime cannot re-run a turn in place and keeps no
feedback channel - and one can, because U13 gave the journal the reader's own
turn: the next prompt. So the row grew a shape rather than a pile of disabled
buttons.

- [x] U16-01: Let a surface add its own actions to the row.
  - `MessageActions` takes `{ id, label, icon }` entries and reports the id it
    was asked for; copying stays first and unconditional, and a control cannot
    join the row without words for what it does.
  - `apps/web/test/message-actions.test.ts` pins the order, the naming and that
    an extra action copies nothing.
- [x] U16-02: Offer each side of the conversation the move that belongs to it.
  - The answer can be **quoted into the composer** as markdown (which is the
    language the field is read in), and the reader's own turn can be **put back
    to run again**. Neither starts work on its own: both end with the caret in
    the field, which is what makes the action a step rather than a decision -
    and what the reference's own row does not promise.
  - `apps/web/test/composer-quote.test.ts` drives both through `RunPanel` and
    asserts the draft and where the caret went.
- [x] U16-03: Re-run the gate: `pnpm typecheck`, the frontend suites, both
  builds, `pnpm stack:verify`.

**What the row still refuses.** Regeneration and feedback. The first is not a
gesture this runtime has - a new answer is a new run - and the second has no
channel to arrive on. Putting the prompt back is the honest half of the first:
it leaves the decision to run with the reader.

## Wave U18 - the rail scrubs, and the jump is a shape

§4.4 asked the rail to scrub while dragged, and the reference draws the jump to
the latest as one round control. Neither was true of the surface: the rail listed
the turns and jumped on a click, but a drag only raised `data-scrubbing` - a flag
that says a drag is happening and moves nothing - and the jump control changed
its whole face when output arrived, so the control a reader had learned vanished
exactly when they had something to catch up on.

- [x] U18-01: Let the drag be the jump, and keep the words up while aiming.
  - The mark reports the turn the finger has reached, so the drag is a sequence
    of jumps; re-reporting the mark already reached is ignored, because
    otherwise the transcript would re-scroll on every pixel of the drag. A drag
    has no hover, so the mark being aimed at keeps its label: aiming at a turn
    means choosing it by its question, and the question is written nowhere else
    on screen.
  - `apps/web/test/message-rail.test.ts` pins the jumps, the dropped repeat, the
    label that survives the drag, and the mark the rail last sent the transcript
    to.
- [x] U18-02: Draw the jump to the latest as the reference's round control.
  - A down arrow in a round button with the count on its corner, and the count
    in the accessible name the face no longer carries; the corner is
    `aria-hidden` so the sentence is not spoken twice. The button keeps the
    `--hit-target-min` box §7 floors it at rather than shrinking to the glyph.
- [x] U18-03: Re-run the gate: `pnpm typecheck`, `pnpm test`, both builds,
  `pnpm stack:verify`.
  - `pnpm typecheck` is clean across all seven projects; `pnpm test` is green at
    26 tooling + 89 protokoll + 292 design + 28 ereignis + 31 website + 575 web
    + 1 desktop-security + 9 desktop tooling tests; the web and website builds
    succeed; `pnpm stack:verify` matches.

**Still refused.** The rail still lists the reader's turns and nothing else: an
answer is not a destination, because the transcript already shows it under the
question it belongs to. And the mark it marks as reached is the one it *sent*
the transcript to rather than one read back from the scroll position - the rail
reports choices, it does not observe them.

## Wave U19 - the menu's own identity

The reference's account menu opens with the account it is about - avatar, name,
and the line under it - and then a rule before the rows. Orchester's menu was a
bare list, so the one thing the surface already knew about itself was the one
thing missing from it.

- [x] U19-01: Give the menu a surface that can carry a header.
  - `AppMenu` renders an optional `header` slot on the popover surface, above the
    `role="menu"` list rather than inside it: a menu may hold items and separators
    and nothing else, and a heading placed inside one would be a list claiming to
    be something it is not.
  - `packages/design/test/app-menu.test.ts` pins the order, that the list still
    holds only items, and that a surface with no header draws none.
- [x] U19-02: Head the account menu with the identity it acts on.
  - The copy is drawn rather than announced: the row that opens the menu already
    carries the same words as its accessible name, so the header is marked
    `aria-hidden` and a screen reader hears the account once, not twice.
  - `apps/web/test/app-rail.test.ts` pins both halves of that.
- [x] U19-03: Re-run the gate: `pnpm typecheck`, the frontend suites, both
  builds, `pnpm stack:verify`.

**Still not taken from the reference's menu.** Remaining usage, invites and
sign-out: there is no account here to meter, invite to or sign out of. The header
is the honest half of that menu - the identity - and the two rows under it are
the two destinations the product actually has.

## Wave U20 - appearance is the theme, personalization is the rest

The reference's settings list carries **Appearance** and **Personalization** as
two destinations, and its appearance screen is the theme and nothing else: the
cards, the preview, the colours each theme resolves to. Orchester had folded
fonts, intensity, motion and the rail's translucency into appearance, so its
appearance screen showed rows the reference does not - and a reader looking for
a font had to know it was filed under the theme.

- [x] U20-01: Add personalization as a section of its own.
  - `settings-search.ts` gains the id and takes over the vocabulary that belongs
    to it (font, type, motion, density, intensity, rail, reading), while
    appearance keeps the words about the theme. The nav renders both under the
    personal group, in that order.
- [x] U20-02: Move the rows that are not about a theme into it.
  - Intensity, reduced motion, both font axes, the rail's translucency and the
    surface readout - and the import/export/reset of the whole appearance
    profile, which covers every axis rather than only the theme.
  - `apps/web/test/settings-view.test.ts` pins the split from both sides: the
    appearance panel holds the three cards and no font or intensity row, the
    personalization panel holds those rows and no card.
  - `settings-search.test.ts` pins that the word which used to land on the theme
    now lands where the fonts went, superseding the U7-01 note.
- [x] U20-03: Re-run the gate: `pnpm typecheck`, the frontend suites, both
  builds, `pnpm stack:verify`.

**What is left in appearance.** The theme cards, the preview, the two theme
editors and the profile actions - which is what the reference's appearance
screen shows, and now nothing besides.

## Wave U21 - choosing the model the next run uses

The reference's composer carries a selector for the model and its effort, and
Orchester could not offer one: the runtime's own selection is session state on a
host, and this server builds **one host per run**, so a choice would be gone
before the run that was meant to use it. The catalog was read-only, which is why
U14 recorded the selector as "not answerable here" - this wave makes it
answerable.

- [x] U21-01: Hold the choice on the server and apply it to every run.
  - `ModelSelection` names the three pieces the CLI's own `/model` command
    names - provider, profile, session effort - and `apply` maps them to the
    runtime's selection calls in one place, so a run and the catalog that
    describes it cannot answer differently. `run.rs` applies it to the run's host
    before the runtime starts, and a choice the configuration no longer supports
    stops the run with an `error` event instead of quietly running on another
    model.
- [x] U21-02: `PUT /api/v1/models/selection`, answered with the catalog.
  - Validated against this workspace's configuration before it is kept (a
    choice that cannot be applied is refused with `validation_failed`, not stored
    and not discovered later), bounded and control-character checked, and
    `GET /models` now describes the model the *next* run will use rather than the
    one the configuration file happens to name.
  - `kisten/netz/tests/model_routes.rs` pins the round trip: a provider and an
    effort are remembered and reported afterwards, a profile names model and
    profile whole, an unusable choice is refused with the model unchanged, and a
    request without a workspace is unavailable.
- [x] U21-03: Carry it to the browser.
  - `ModelSelectionRequestDto` in the protocol, `ModelsApi.select` in the client
    (which reads back the catalog the runtime produced rather than assuming the
    click took effect), and a `select` action on the catalog store that keeps the
    last good catalog when the runtime refuses.
  - `apps/web/test/models-api.test.ts` and `model-catalog-store.test.ts` pin the
    request, the refusal and the state that survives it.
- [x] U21-04: Re-run the gate: `pnpm typecheck`, the frontend and Rust suites,
  both builds, `pnpm stack:verify`.

**What is left for the next wave.** The picker itself: the composer's trailing
row draws a readout today, and turning it into the reference's selector - the
providers, the profiles and the effort levels, wired to `select` - is the half
that makes this visible. The capability is landed and tested; the control is not.

## Wave U22 - the selector the reference draws in the field

U21 landed the capability; this is the control. The composer's trailing row now
carries the model the next run will use, next to the control that starts it,
which is where the reference draws its own selector.

- [x] U22-01: Let a menu say which choice is in force.
  - `AppMenuItem` gains `checked`, and a menu whose items carry it announces
    itself as a set of radios with `aria-checked` and a check beside the current
    one: a menu of actions and a menu of choices are different things, and the
    difference is this field rather than a convention. A long hint is now bounded
    and truncates, because a provider's reason for being unreachable had been
    pushing the provider's own name off the row.
  - `packages/design/test/app-menu.test.ts` pins the role, the state and the
    check.
- [x] U22-02: Make the readout a picker, in the field's trailing row.
  - It offers what the runtime reports: every provider - an unreachable one drawn
    disabled with its reason, because a menu that silently omits a provider
    leaves the reader wondering where it went - the named profiles, and the
    effort names this surface can say, plus the provider default that clears the
    override. A value the catalog reports that has no name here stays visible in
    the trigger rather than being forced into one of them.
  - Each click reports the **whole intent**: picking a provider or a profile
    keeps the effort in force and picking an effort keeps the axis, because the
    runtime takes three axes and a request that omitted one would silently reset
    it. The axis is read off the catalog rather than remembered, so it cannot
    disagree with the runtime after a reload.
- [x] U22-03: Reverse the earlier separation, deliberately.
  - The model had been a *fact* in the context row; it is a *decision*, and the
    reference draws it with the action. `composer-context-bar.test.ts` and
    `run-composer-card.test.ts` are updated to the new anatomy and say why.
- [x] U22-04: Wire the click through to the runtime.
  - Picker → composer → panel → view → the catalog store's `select`, which asks
    the runtime and keeps the catalog it answers with. `workspace-view.test.ts`
    drives that whole chain and asserts the request that leaves the browser.
- [x] U22-05: Re-run the gate: `pnpm typecheck`, the frontend suites, both
  builds, `pnpm stack:verify`.

**What the selector still does not do.** It offers the effort names this product
knows rather than a free field, because the runtime passes any non-empty value
through and a text box for a four-name axis would be a worse control than a
list. A model the provider catalog does not name cannot be typed in either:
choosing is what this surface offers, not configuring.

## Wave U23 - tables, and how far the model choice reaches

Two loose ends, one in each half of the answer.

- [x] U23-01: Read and draw pipe tables.
  - U15 left tables out on the grounds that a table is not worth a parser that
    can be talked into producing an element. That reasoning held for raw HTML and
    still does; a table does not need it, because a cell is the same span list as
    every other text in the answer - already escaped, already parsed for inline
    code and links, and never markup. The delimiter row is what makes a table a
    table: a line with pipes in it is prose a reader wrote with pipes in it.
  - A row with the wrong number of cells is padded or trimmed rather than
    rejected, and a table that is wider than the prose scrolls on its own so the
    transcript never scrolls sideways to show one.
  - `packages/design/test/markdown.test.ts` and `markdown-text.test.ts` pin the
    reading, the alignment, the padding and the scroll wrapper.
- [x] U23-02: Say how far the model choice reaches, where the choice is made.
  - The selection lives on the runtime for as long as it runs and is **not**
    written to the configuration file, so the picker says so in its own menu.
    Silently session-scoped would read as a setting the reader had changed for
    good. Persisting it is possible - `ConfigLoader::edit_user_config` exists for
    exactly this, preserves comments and keeps a `.bak` - but rewriting the file
    a human maintains is a decision to take deliberately rather than as a side
    effect of a click, so it is recorded here as a candidate rather than done.
- [x] U23-03: Re-run the gate: `pnpm typecheck`, the frontend suites, both
  builds, `pnpm stack:verify`.

## Wave U24 - one line per run in the rail's lists

The reference's sidebar lists are a title per entry and nothing else, and
Orchester's rows had grown a second line of agent, model and a resumable badge.
Those are details *of* a run rather than its name, and the pane that opens when
the row is chosen states them in full - so they moved behind the row instead of
sitting on it.

- [x] U24-01: Draw the row as the title, when it ran, and the outcome's dot.
  - The agent, the model and the resumable note are now a tooltip for the pointer
    and hidden text for a screen reader, which is what keeps one line from
    meaning less; the outcome stays where it already was, on the dot, which
    carries it as its own accessible name and is not repeated in the hidden text.
  - `apps/web/test/session-list-item.test.ts` pins the row's children, the
    hidden details, the tooltip, the dot's state per outcome, and the run that
    reported no model.
- [x] U24-02: Re-run the gate: `pnpm typecheck`, the frontend suites, both
  builds, `pnpm stack:verify`.

**What the row deliberately keeps.** The time. The reference's list entries
carry none, but a list of runs is a history, and a history without when is a
list a reader has to open each entry to date.

## Wave U25 - the answer formats while it arrives

U12 rendered markdown only once an answer had settled, on the grounds that "a
fence that is half written is not a code block yet". The reference does the
opposite - it formats an answer while it is still being written - and this
parser already reads an unclosed fence as the code it is, so the block the
reader watches appear is the block they end up with.

- [x] U25-01: Render the answer's markdown from its first token, and keep the
  containment that makes that affordable.
  - The row still carries `contain: layout paint` and `content-visibility: auto`
    while it arrives, which is what keeps a growing row from being measured by
    the transcript's own layout - the thing the virtual window reads. The
    reader's own turn is still not parsed, because their words are not the
    agent's markup.
  - `apps/web/test/chat-message-anatomy.test.ts` now pins the arriving render
    (a fence open mid-stream draws a code block) and the containment style.
- [x] U25-02: Measure what re-reading on every delta actually costs, rather than
  assuming.
  - In headless Chromium on this machine, one parse of a whole 2 000-character
    answer takes **0.86 ms** and one of a 20 000-character answer **2.58 ms**;
    the same 2 000-character answer arriving in 2 000 deltas - re-parsing on each
    - spends **323 ms in total**, spread across the whole stream. A frame is
    16 ms and the last parses of a stream are the ~1 ms ones, so the choice is
    affordable rather than merely convenient. The harness that measured this was
    temporary; the numbers are what it reported.
- [x] U25-03: Re-run the gate: `pnpm typecheck`, the frontend suites, both
  builds, `pnpm stack:verify`.

**What this does not fix.** A half-written `**bold**` still shows its asterisks
until it closes, and a table is a paragraph until its delimiter row arrives - the
same progressive reading the reference has. The alternative is holding
formatting back until the end, which is what this wave stopped doing.

## Wave U26 - the pinned list the reference opens with

The reference's sidebar opens with a list the reader chose: what they kept at the
top. Orchester's rail opened with a list of one - the workspace - which the
product row already names, so the reference's structure could be adopted
honestly: the first list became the reader's own, and the workspace's state moved
to the row that names the product.

- [x] U26-01: Keep the pins where the other preferences live.
  - `usePinnedSessions` reads and writes `orchester:sessions:pinned`, hydrating on
    first use like the appearance store, and keeps only identifiers it can believe
    - the value is a key a reader can edit by hand, and a hand-edited key must
    open the rail rather than break it. Newest pin first, because the rail is read
    from the top.
  - `apps/web/test/pinned-sessions.test.ts` pins the ordering, the unpin, the
    stored round trip and the tolerance for what is not a list of ids.
- [x] U26-02: Give the row the control, beside it rather than inside it.
  - The row is now a container: the open control and the pin are siblings, because
    a button within a button is not something a browser will let a reader press.
    The pin is drawn on hover or focus and stays visible once it is pinned, since
    that is the state it reports; `aria-pressed` and the label carry the state
    rather than the colour alone.
- [x] U26-03: Draw the pinned list from the loaded page, and once.
  - `PinnedSessions` renders the pinned runs in the reader's order; a pinned id
    whose run is not on the loaded page is not drawn, because the rail holds a
    cursor rather than the whole history and a row it cannot name would be worse
    than an absent one. `SessionRail` excludes pinned runs, so a run appears once
    - and its "nothing matched" message is now asked of the whole page, so a
    filter whose only match is pinned cannot contradict the row above it.
  - `ProjectList` is gone: it was one row of state that the product row states.
- [x] U26-04: Re-run the gate: `pnpm typecheck`, the frontend suites, both
  builds, `pnpm stack:verify`.

## Wave U27 - the title row the reference opens with

The reference's top row is one strip: a rail toggle, back and forward, the four
menus, and the caption buttons at the trailing edge. Orchester drew two - a
32 px title bar that carried nothing but the window's name, and a 56 px product
header under it that carried the mark, the workspace, the connection state and
the theme switch. The product header was already a duplicate: the rail's own
product row names the product and the workspace since U26. So the two rows
became the one the reference draws, and what the header had honestly carried
moved to where it can still be read.

- [x] U27-01: Draw one title row for every face.
  - `TitleRow.vue` is region A. It renders in the browser and in the desktop
    window: the rail toggle, back and forward, the four menus, the connection
    state while it is not `ready`, and - on the desktop only - the drag region
    and the caption buttons, at the OS metric and in the native order.
    `WindowChrome.vue` and `WorkspaceHeader.vue` are gone, and with them the
    second row. The row is the shell's one `banner` landmark, which the header
    used to be.
  - The row is 38 px, or 32 px on Windows, and `--app-top-chrome-height` is now
    that height rather than the sum of the two rows, so every full-height
    surface under it reads one number.
  - `title-row.test.ts` moves the chrome's own contract - the caption order, the
    drag region, the macOS traffic lights, the opaque surface, the rejected
    action - onto the row, and pins the new order: toggle, arrows, menus,
    captions.
- [x] U27-02: Fold the rail, and let the row read what it folded.
  - `useRailCollapsed` keeps one boolean under `orchester:rail:collapsed`, read
    on first use and written on every change, as the rail's width already is.
    `AppShell` draws no rail column while it is set, and never draws the resize
    handle for a column that is not there.
  - `shell-actions.ts` is a small registry of the app-chrome actions the mounted
    surface owns: the row asks whether one is available rather than assuming it,
    so a row of a menu that this route cannot answer is drawn disabled with its
    reason instead of doing nothing.
  - The toggle is the reference's leftmost control, `aria-pressed` carries the
    state, and it is disabled - with the reason - on a route that draws no rail.
- [x] U27-03: Open the menus the reference draws, with the actions this product
  can answer.
  - **File**: a new chat, settings, and closing the active tab. **Edit**: focus
    the prompt, and clear it. **View**: the rail, the inspector, the companion,
    and the theme. **Help**: the keybindings and the about screen. Every chord
    printed on a row is read from the live shortcut registry, so a rebinding
    changes the menu with it; a row whose chord is not bound here prints none.
  - The two settings rows deep-link with `?section=`, which the settings route
    now honours, because a Help menu that lands on the top of the settings page
    has not answered "where are the shortcuts".
  - What is *not* taken: undo, redo, cut, copy, paste and select-all. Orchester
    owns no editor; the webview already answers those keys inside a field, and a
    row that called a deprecated clipboard API would be a menu claiming the
    platform's own job. The bottom panel keeps its own toggle rather than
    gaining a `Mod+` row: the panel owns its state, and a chord is a promise to
    flip it from anywhere.
- [x] U27-04: Re-run the gate: `pnpm typecheck`, the frontend suites, both
  builds, `pnpm stack:verify`.

**What the row does not carry.** The reference's title bar also holds the window
title; Orchester's window is named by its rail and its tab strip, and a second
name in the strip would be a third place to keep in step. The drag region
carries the name as its tooltip instead.

## Verification

```text
pnpm --filter @orchester/design test
pnpm --filter @orchester/web test
pnpm typecheck
pnpm --filter @orchester/web build
pnpm --filter @orchester/website build
```

Each wave ends with the focused package test for the packages it touched, plus a
`git diff --check`.
