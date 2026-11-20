# Protocol and source review

Two independent reviews inspected the small observed client against the saved Microsoft WSL and kernel 9P sources. No review invoked native commands, sockets or mount operations. The method uses the original `/init` process creation path rather than constructing an undocumented process-creation message.

| Saved upstream source | Relevant lines | Supported method |
| --- | --- | --- |
| `lxinitshared.h` | 117–118, 129 | Ordinary DrvFs port 50002, administrative port 50003 and 65,536-byte buffer size. |
| `drvfs.cpp` | 585–586, 657 | Selects the DrvFs endpoint and constructs `drvfs;path=` attach information. |
| `util.cpp` | 453–526, 631–633 | Uses `WSL_INTEROP`/ancestor sockets and host CID for vsock connection. |
| `init.cpp` | 176 onward | Dispatches `/init` entry points according to executable arguments. |
| `binfmt.cpp` | 140–167 | Creates a native process request through the existing interop channel. |
| `interop.md` | 13–30 | Describes `/init`, interop server sockets and `WSL_INTEROP`. |
| `kernel9p-client.c` | 910 onward, 1107 onward, 1154 onward, 1229 onward, 1545 onward, 1782 onward | Cross-checks version, attach, walk, open, read and getattr operation layouts. |

Official source locations are in [the provenance file](source-provenance-safe.json). The documented [WSL interop route](https://github.com/microsoft/WSL/blob/master/doc/docs/technical-documentation/interop.md) and [DrvFs implementation](https://github.com/microsoft/WSL/blob/master/src/linux/init/drvfs.cpp) explain the method. Source version is not asserted to match the installed WSL runtime.

The observed operation sequence is `Tversion(100) → Tattach(104) → Twalk(110) → Tgetattr(24) → Tlopen(12, flags=0) → Tread(116) → Tclunk(120)`. All observed reply types and tags match their requests. `Rgetattr` mode at payload offset 21 and size at 49 match `9P2000.L`. The initial response negotiated a 65,536-byte message size, and the recorded header read returned 4,096 bytes.

The complete reader validates nonempty chunks and each returned count against that response's payload length. It reads until the advertised 339,968 bytes, checks the pre/post QID and size, requires MZ, and writes only the owned Linux copy. This sequence contains no write, create, setattr, rename, remove or mount request. Tattach itself is not a server-enforced read-only capability; the original `readOnlyRootAttachSucceeded` field was therefore renamed to `attachSucceeded` in the selected result. The numeric uid/gid are literal attach input; separate Linux process status supplies the measured UID.

This is a bounded diagnostic client. It does not strictly check every payload layout or getattr valid mask, apply a returned `iounit`, adapt its reads to a smaller negotiated `msize`, enforce returned count ≤ requested count, or explicitly require final length equals advertised size. The observed negotiated size, saved file length, independent SHA-256 and PE checks support this run; these checks do not turn it into a general production 9P implementation. The archival 9P templates replace the Linux username, and are explicitly not byte-identical executed source.

Pre/post QID and size equality does not exclude a same-size content change. A later native preflight independently measured the same public System32 cmd.exe digest and reported Authenticode `Valid`, with Microsoft Windows as signer; [its selected receipt](native-host-command-identity-selected-safe.json) retains only these public image fields and execution bindings. This corroborates acquired byte identity without attesting the earlier copy instant. Ordinary endpoint selection does not independently measure Windows elevation. The initial `ver` exit code is rejected because MUI failure makes its semantics unclear. Exact PowerShell ASCII output and the real captured process return code are retained instead. The external completion receipt establishes controller completion, and is not a retained native Windows process handle.
