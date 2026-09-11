# Build Toolchain Requirements

This repository targets Rust's MSVC ABI on Windows. A Unix/MSYS linker named
`link.exe` is not interchangeable with the Microsoft linker, even though both
commands have the same filename.

## Required Windows setup

For local Rust tests and release builds, install:

- Rust stable with the repository's MSVC target (`aarch64-pc-windows-msvc` on
  ARM64 Windows, or `x86_64-pc-windows-msvc` on x64 Windows).
- Visual Studio Build Tools or Visual Studio with **Desktop development with
  C++**, the matching MSVC toolset, and the Windows SDK.
- An architecture-matched Developer PowerShell/Command Prompt so `link.exe`,
  `rc.exe`, and the SDK library paths are exported by the environment.

Verify the shell before running Cargo:

```powershell
rustc -vV
rustup show active-toolchain
Get-Command link.exe
where.exe link.exe
cargo test --workspace
```

The frontend workspace exposes the same checks in a machine-readable doctor:

```powershell
pnpm --dir apps doctor:desktop
node werkzeug/frontend/doctor.mjs desktop --json
```

Required failures return a non-zero exit code and block `pnpm --dir apps
dev:desktop`. The stable Windows failure IDs include:

| ID | Meaning | Repair |
| --- | --- | --- |
| `windows-linker-shadowed` | An MSYS/MinGW `link.exe` appears before Microsoft's linker. | Open an architecture-matched Visual Studio Developer PowerShell and remove the MSYS/MinGW bin directory from that shell's `PATH`. |
| `windows-msvc-compiler-missing` | `cl.exe` is absent from `PATH`. | Install Visual Studio Build Tools with **Desktop development with C++**, the matching ARM64/x64 tools, and Windows SDK, then reopen Developer PowerShell. |
| `windows-msvc-linker-missing` | No `link.exe` is available. | Install the matching MSVC toolset and use its Developer PowerShell. |
| `windows-rust-abi` | Rust is not using a `*-pc-windows-msvc` host/target. | Install and select the matching stable MSVC toolchain with rustup. |

`where.exe link.exe` must resolve to a Visual Studio installation, for example
`...\VC\Tools\MSVC\...\bin\Hostarm64\arm64\link.exe` or the corresponding
x64 host/target path. If it resolves to `msys*\usr\bin\link.exe`, remove that
directory from PATH for the Developer shell. Do not work around this by changing
Rust source or by using a MinGW linker for an MSVC target.

## Current development machine audit

The ARM64 development machine used for the frontend work currently reports:

```text
rustc 1.96.1 ... host: aarch64-pc-windows-msvc
active toolchain: stable-aarch64-pc-windows-msvc
target installed: aarch64-pc-windows-msvc
link.exe: D:\software\msys\msys2\usr\bin\link.exe
Visual Studio 2022: not installed
Windows SDK libraries: installed
```

That `link.exe` is the MSYS GNU linker. It rejects rustc's MSVC arguments with
`link: extra operand`, so `cargo test -p orchester-protokoll` cannot reach the
test binaries on this host. `cargo fmt --all -- --check` and all TypeScript
protocol typecheck/unit tests remain runnable and must still be used for local
feedback.

The doctor confirms this host as `win32/arm64`, reports Node.js and pnpm as
usable for the WebUI, and reports `windows-linker-shadowed` plus
`windows-msvc-compiler-missing` for the desktop profile. Do not mark a native
Tauri build successful until those failures are gone and the actual Rust build
command completes.

## GitHub Actions

The release workflow builds Windows artifacts on GitHub-hosted Windows runners,
where the Microsoft linker and SDK are provided by the runner image. Frontend
deployment must likewise run through a pinned GitHub Actions workflow; GitHub
Pages is not a local `gh-pages` branch or a manually uploaded directory.

Any future Windows build job should fail early with a linker provenance check:

```powershell
$linker = (Get-Command link.exe -ErrorAction Stop).Source
if ($linker -match 'msys|mingw') {
  throw "MSVC build selected a non-MSVC linker: $linker"
}
```
