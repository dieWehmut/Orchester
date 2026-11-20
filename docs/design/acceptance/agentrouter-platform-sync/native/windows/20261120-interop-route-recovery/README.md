# Alternative WSL native route observed on 2026-11-20

A fresh ordinary DrvFs 9P connection read the public `C:\Windows\System32\cmd.exe` while the existing `/mnt/c` mount still returned `EIO` (errno 5). The acquired 339,968 bytes were copied into an owned Linux evidence directory. Through the existing `/run/WSL/357_interop` socket, that copy launched the original Windows PowerShell path and returned the exact ASCII sentinel with process exit 0. This establishes an alternative route at the recorded time. It does not establish repair of `/mnt/c` or acceptance of the default Orchester runtime.

The external Codex command completion at `2026-11-20T03:28:09.715Z` independently records controller exit 0; its exact embedded source matches [the archived native probe](sources/native_windows_readonly_probe.py). [The selected receipt](native-tool-receipt-safe.json) binds that completion, the captured native exit codes, absolute PowerShell path and [exact sentinel bytes](native-sentinel.stdout.txt). The script's display label says `echo`; the executed PowerShell source uses `Write-Output`.

| Observation | Retained evidence | Interpretation |
| --- | --- | --- |
| Inherited `2482_interop` missing; `357_interop` and `628707_interop` accept and identify root Relay peers | [Linux route](linux-route-observation-safe.json) | Distinguishes stale environment from available alternate sockets; root peer ownership does not measure a Windows token. |
| Initial normal `/init` attempts through both available sockets exit 1 | [Initial failures](direct-init-failures-safe.json) | Original Windows-path launch failed before the acquired Linux copy was used. |
| CID 2, ordinary endpoint 50002, `9P2000.L`, complete public path walk and 4,096-byte header read | [Protocol result](9p-public-file-read-safe.json) | A fresh host connection succeeds independently of the failing existing mount. |
| Complete copy SHA-256 `15cd209c995b7d3009c91ca40892d5b8f1fb162f3e5f10716642ea278e8e0d7b` | [Independent byte identity](copied-public-command-identity-safe.json) | Independently rehashed the actual owned Linux copy; PE AMD64 identity is checked. No binary is committed. |
| Native Windows hash matches the acquired copy; Authenticode `Valid`, signer Microsoft Windows | [Selected original system-file identity](native-host-command-identity-selected-safe.json) | Separate native preflight at `03:48:42Z` independently confirms the public System32 cmd.exe identity; its external receipt exited 0 and matches the source stdout digest. |
| Copied `cmd /c ver` exits 0 with MUI errors | [Copy identity and rejected version result](copied-public-command-identity-safe.json) | No usable version output; rejected as a semantic execution test. |
| Original PowerShell prints `ORCHESTER_NATIVE_ROUTE_OK` and exits 0 | [Native receipt](native-tool-receipt-safe.json) | Stronger evidence of native execution through the recovered route. |
| Native `Get-NetTCPConnection` query returns zero rows for 7338 | [Selected native output](native-tcp-query-selected-safe.json) | Point-in-time result only. The original command suppressed query errors; it does not certify an error-free query or reserve the port. |

The wire operation sequence was read-only: version, attach, walk, getattr, open with flags 0, read and clunk. Attach did not enforce a read-only share. The controller did not remount, restart services, signal existing processes or request writes to existing Windows files. Native PowerShell's natural cache effects were not measured, so this archive makes no claim that all Windows writes were zero.

Recorded high-order VMBus allocation failures and relay accept warnings are consistent with a memory/transport failure hypothesis. The archive retains a bounded summary, and does not establish that those events caused this failure or that their cause was repaired.

[Source provenance](source-provenance-safe.json) records official upstream source URLs and downloaded byte digests; [the protocol review](source-review.md) explains the supported method and remaining limitations. These `master` snapshots are not bound to the installed WSL build. Full upstream sources, kernel logs, user profile content, the executable, garbled `wsl --status`, CLIXML and the raw conversation are omitted.

Run `python3 validate.py --self-test` from this directory. Validation reads only the retained archive; it never invokes the native or 9P probes. To additionally rehash the private acquired copy, pass `--private-copy /absolute/path/to/owned-native-cmd.exe`. Offline validation checks retained evidence and its boundaries; it does not repeat the live observations.
