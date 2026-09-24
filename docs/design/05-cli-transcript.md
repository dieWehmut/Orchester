# The CLI's transcript, against the reference

> The operator's note against a screenshot of the Codex CLI's transcript: "修复
> cli 界面 ui 应该改成像 codex 类似的". The CLI is `kisten/konsole`, and what the
> reference actually shows is a *transcript vocabulary* - how a line of prose, a
> step the program took, and its output are drawn - so that is what this starts
> with. The web surface has its own record in
> [`04-reference-alignment.md`](./04-reference-alignment.md).

## What the reference draws

```text
• Worktrees are 13 dirty — I won't touch those. Final round on …
• Ran $s = @'
  │ $ErrorActionPreference = "SilentlyContinue"
  │ function DirMB([string]$p) { … }
  └ … +22 lines (ctrl+t to view transcript)
  CLEARED      0.0 MB  C:\Windows\SoftwareDistribution\Download (now 368.1 MB)
  └ +4 lines (ctrl+t to view transcript)
• Working (1h 01m 03s • esc to interrupt)
› Ask Codex to do anything
~ · deepseek-v4.1-flash · medium · Full Access · Fast off        ⚠ 3 warnings · f2 to view
```

Three idioms: a **bullet** opens a line of prose and its continuation hangs under
it; a **step** is a bullet, its verb, a body on a rail (`│`) closed by a corner
(`└`) that counts what was left out; and the **chrome** - the chevron prompt, the
working line with a real clock, the status bar joined by dots.

## Wave CLI-1 - the transcript's vocabulary

- [x] CLI1-01: Prose and steps read as the reference reads them.
  - `kisten/konsole/src/self_agent/render.rs` writes a bullet before prose, hangs
    its continuation under the bullet, and draws a step as
    `• <Verb>` / `  │ <body>` / `  └ <summary>`, bounding the body at eight lines
    and counting the rest in the corner as `… +N lines · <summary>`.
  - This replaced `tool: read_file` followed by the summary and then the content
    with no structure at all: the summary is now the corner that closes the step,
    which is where the reference puts its own note.
- [x] CLI1-02: The chrome uses the reference's glyphs.
  - The composer and the echoed turn share the chevron `›`; the working line and
    the panel chatter share the bullet `•`; the status bar joins its segments
    with ` · ` rather than `  |  `.
  - The working line now carries a **real** clock and the way out:
    `⠋ Working (3m 20s · esc to interrupt)`. Both halves are true here - the loop
    counts the interval it sleeps on, and Esc while a turn is in flight cancels
    the turn (`main.rs` turns it into a cancellation, not a quit) - so the hint
    promises what the key does.
- [x] CLI1-03: Tests, then the suite.
  - `self_agent/render.rs` gains four cases: the bullet and hanging indent, a
    step's verb/rail/corner, a bounded body with its count, and a list body that
    keeps its columns. `main.rs` pins the working line in all four shapes of the
    clock. `interactive.rs` gains the clock's own test.
  - `cargo test -p orchester-konsole`: 253 tests across the unit and integration
    suites, no failures; `cargo check -p orchester-konsole --all-targets` is
    clean with no warnings.

**What is deliberately not in it.** The reference's `(ctrl+t to view transcript)`
- this product has no transcript view to open, so the corner counts the lines and
promises no key. `⚠ 3 warnings · f2 to view` on the right of the status bar needs
a warnings count the CLI does not assemble yet, and the subject of a step
(`• Read src/lib.rs` rather than `• Read`) needs the action's path to travel with
the observation, which is a change to a protocol-visible payload rather than to
the renderer. Both are recorded here as the next waves rather than approximated.

**What was cost.** A dozen assertions in `interactive.rs` pinned the old `> `
glyph; one of them compared byte offsets where it meant display columns, and now
measures columns, because the chevron is three bytes wide and one column wide.