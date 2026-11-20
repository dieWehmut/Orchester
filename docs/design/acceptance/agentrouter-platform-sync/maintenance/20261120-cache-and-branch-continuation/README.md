# Cache and branch continuation, 20 November 2026

This archive preserves a derived summary of previously unarchived 16 November
cleanup receipts, records one additional npm download-cache cleanup, and provides
a bounded current observer. Private receipt bytes, command lines, application
profiles, capability keys and raw logs are excluded. Seven original receipt files
are bound by byte count and SHA-256 in
[original-source-bindings-safe.json](original-source-bindings-safe.json).

The [historical summary](historical-cleanup-derived-safe.json) records these
separate measurements:

| Operation | Actual completed scope | Measurement and boundary |
|---|---|---|
| 16 November Linux caches | 5 recorded directories removed | Planned reclaimable 22,649,397,248 B; observed filesystem available-space increase 22,649,364,480 B; external hard-linked allocation 763,928,576 B retained |
| 16 November Windows caches | 8 recorded directories removed after native preflight | Audited allocation 1,131,077,632 B; observed C-volume available-space increase 1,131,835,392 B |
| 16 November branch cleanup | 4 local branch refs and 3 worktree paths removed; delivery summary reports 2 remote branch deletions | Fresh absence and Git patch-equivalence checks are in the current observer result |
| 20 November Linux npm cache | 6 regular files and 16 child directories removed | 191,672,408 logical B; 191,688,704 regular-file allocated B; observed Linux filesystem available-space increase 191,758,336 B |

Available-space differences are observations during each interval. Concurrent
filesystem activity can affect them. Planned reclaimable bytes, regular-file
allocated bytes and logical bytes are distinct measurements. They are not added
together as a physical-space total. Linux space release does not establish host
C-volume or VHDX reclamation.

The 20 November cleanup targeted only `/home/han/.npm/_cacache`, with root identity
device `2096`, inode `34873`, UID `1000`. Two metadata inventories matched, all
entries belonged to the current user, and there were no symlinks or multiply
linked regular files. No readable package-manager process or cache-path reference
was observed. The receipt retains unavailable `/proc` reads and the fd-readlink
race as visibility limits. The fd-based removal command completed with tool exit
code 0 and verified directory absence. The exact executed source and emitted
result are [cleanup-owned-npm-cache.py](cleanup-owned-npm-cache.py) and
[npm-cleanup-result-safe.json](npm-cleanup-result-safe.json).

The separately archived
[11,441,931,494-byte incremental cleanup](../20261116-incremental-cache-cleanup/README.md)
is cross-referenced for current path absence only. Its 42,657 regular files and
10,981,446,886 unique-identity logical bytes are not counted again here. Earlier
selected installer, PDB, package-cache and branch cleanups already archived
elsewhere are also outside this additional accounting.

The [current read-only result](current-readonly-safe.json), observed at
02:22:31 UTC, verifies all six bounded Linux cache paths absent, four historical
local refs absent and three historical worktree paths absent. The historical
branch tips are not main ancestors; `git cherry` reports 4, 2, 3 and 2 equivalent
commits, with zero non-equivalent commits. The current remote query verifies the
two historically deleted remote refs absent. `feat/rail-reference-alignment`
still has 18 non-equivalent commits and was retained. The active work branch is
retained for the ongoing main delivery. The detached `monitor-validation`
worktree retains its 20 tracked changes; no filenames or content are published.

Windows `/mnt/c` returned `OSError` errno 5. Current Windows cache absence and
C-volume free space are therefore unverified. No Windows cache, old PDB, SDK,
AVD, profile, rollback file, installed artifact, `node_modules`, unmerged branch
or dirty worktree was removed by this continuation. The historical native cache
preflight and Windows deletion remain dated 16 November evidence. The initial
current observer's remote query timed out after 30 seconds; its original source
and failed result remain as `initial-observer-source.py` and
`initial-current-readonly-failed-safe.json`. The later bounded query succeeded.

Repeat only the read-only current check with:

```sh
python3 docs/design/acceptance/agentrouter-platform-sync/maintenance/20261120-cache-and-branch-continuation/observe-current-readonly.py
```

Validate the saved archive with:

```sh
python3 docs/design/acceptance/agentrouter-platform-sync/maintenance/20261120-cache-and-branch-continuation/validate.py
```

The observer performs no Git mutation, deletion, service action, Windows native
probe or process signal. The cleanup source defaults to a read-only preflight;
its deletion mode is pinned to the original cache identity and metadata and
refuses a different or regenerated cache.
