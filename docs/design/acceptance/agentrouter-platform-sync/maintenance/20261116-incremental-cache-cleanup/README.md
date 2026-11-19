# Linux incremental cache cleanup, 16 November 2026

The authorized directory was removed once at 10:36:54–10:36:57 UTC. The original
unified execution session `90014` completed with exit code 0. Its original source,
result and process observations are preserved byte for byte in this archive.

The exact scope was
`/home/han/project/Orchester/.worktrees/main-integration/target/debug/incremental`,
bound before deletion to device `2096`, inode `2901617`, owner UID `1000`.
All ancestors were ordinary directories. Two independent metadata inventory
reads were identical, with 42,657 regular files, 1,473 directories and no symlinks.
The native fd-based `shutil.rmtree` removed only this authorized directory.

| Measurement | Value |
|---|---:|
| Regular-file logical bytes removed | 11,441,931,494 |
| Unique `(device,inode)` logical bytes | 10,981,446,886 |
| Unique regular-file identities | 38,779 |
| Files having multiple hard links | 7,835 |

These are logical-file measurements. Physical disk release, VHDX reclamation and
C-drive net free space were not measured.

The pre-delete observation inspected 742 readable file descriptors and 36 process
maps. It found no cache path usage and no readable cargo/rustc/sccache/rust-analyzer
process. Forty-three process observations were inaccessible or exited; one fd
readlink raced with closure. These boundaries remain in the original result.

The original Linux7337 listener retained PID `427`, start ticks `342`, and the
236,091,496-byte ELF with SHA-256
`d983bceea7c0797c995c7ffb038eaa984323ee1106293da18433f2a3f22d6649`.
Its file identity, bytes and process binding matched before and after deletion.
Other entries directly under this main worktree's `target/debug` retained their
metadata. The [independent read-only observation](current-readonly-safe.json),
executed at 10:46:05 UTC with exit code 0, again verified directory absence and
the same ELF and live process binding.

The full 9,268,086-byte private inventory is retained outside the archive. Its
SHA-256 is `f70fbd325979052b62d4c5c7c6bf59ccf2d6f63ea72079680eaf2fb9bc48a447`;
the canonical metadata-row SHA-256 is
`b06d3b175e5d33a96cbe82de79eae46fd9922b62d7d503e1a3481221ff49138a`.
Neither hash is a digest of the cached file contents.

[Source mappings](source-mappings-safe.json) bind original bytes and new observer
artifacts. References in the original execution provenance to the private full
inventory and the prepared Windows PDB plan remain external source references.
This operation performed no Git mutation, Windows action, process signal or
service lifecycle action.

The current check can be repeated with:

```sh
python3 docs/design/acceptance/agentrouter-platform-sync/maintenance/20261116-incremental-cache-cleanup/observe-current-readonly.py
```

On 19 November, the same read-only observer again returned exit code 0 and
verified cache-directory absence plus the original PID/start-ticks/ELF identity.
The [fresh result](continuation-20261119-current-readonly-safe.json) retains its
actual observation time. The deletion was not rerun.
