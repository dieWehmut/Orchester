# Actual WSL reboot, automatic startup, and retained original state

The original named user service automatically started on two later observed WSL
kernel boots. The historical audit at 07:28 UTC records boot `ce7a729b…`; the
current independent lifecycle audit and state audit record boot `04a4390b…`.
Both differ from the archived November 12 boot `d67ae493…`. The current journal
contains typed `start` / `done` records from the same live user manager: Basic
System at 2,846,295 microseconds, the named service at 2,846,345, and default.target
at 2,846,357. Current PID ancestry is manager 266 → controller 293 → listener 427
(birth ticks 342). The original enabled unit, controller source, runtime binary,
and private environment snapshot remain hash-identical to their archived bindings.
Raw wall-clock and kernel btime observations are retained independently; the causal
claim uses boot identity and monotonic journal ordering.

The state audit binds its original private consistent backup to the SHA-256 of
the November 13 task's producer receipt already referenced by the public archive.
It compares that baseline, all six existing consistent backups, and a stable copy
of the current database. The five original run rows remain byte-identical; all
five persisted authentication entries and every remote-state JSON value remain
logically equal. Raw JSON serialization and whole SQLite file bytes differ; those
differences are preserved and are not reported as data loss or byte preservation.
SQLite opened only new private scratch copies. Original DB/WAL bytes stayed
stable, original SHM was not read or copied, and the private scratch was removed.
The archive contains aggregate digests and counts, without database files, rows,
tokens, environment values, or private database paths in the safe results.

All 17 public runtime resources returned HTTP 200 and matched the original build
hashes and current mobile dist bytes. Anonymous `/remote/v1/info` returned 401.
The first readonly observer accidentally used `/info` and exited 1 at the HTTP
gate before state comparison. Its unchanged source and failure are retained.
The corrected observer changed only that endpoint and completed with actual exit
0. `execution-provenance-safe.json` records both outcomes and the independent
lifecycle observer's actual exit 0; no retroactive exit code is assigned to the
older 07:28 evidence.

This proves the WSL reboot/autostart and original Linux logical-state gates. It
does not prove a Windows OS reboot or the original Windows phone's current
NativeCredentials.get and authenticated request to the new Linux instance.
No reboot, service lifecycle operation, signal, pairing, or model call was issued
by these readonly audits. Scripts are archived as executed sources; their output
directories were private task-owned directories, not this public evidence folder.
