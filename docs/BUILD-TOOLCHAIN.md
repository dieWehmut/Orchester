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
pnpm doctor:desktop
node werkzeug/frontend/doctor.mjs desktop --json
```

Required failures return a non-zero exit code and block `pnpm dev:desktop`.
The stable Windows failure IDs include:

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
target installed: aarch64-pc-windows-msvc, x86_64-pc-windows-msvc
Visual Studio Build Tools 2022 at D:\software\vs_buildTools\BuildTools
MSVC toolsets: 14.44.35207 and 14.50.35717, x64 and x86 host/target only
Windows SDK libraries: installed, including arm64
```

The default `PATH` still resolves `link.exe` to
`D:\software\msys\msys2\usr\bin\link.exe`, which rejects rustc's MSVC
arguments with `link: extra operand`. This host therefore builds the desktop
shell through the x64 toolchain rather than natively:

```powershell
# From a Developer shell whose PATH has MSVC first:
cmd /c \"call \"D:\software\vs_buildTools\BuildTools\VC\Auxiliary\Build\vcvars64.bat\" && rustup run stable-x86_64-pc-windows-msvc cargo check --locked --target x86_64-pc-windows-msvc\"
```

That command compiles the whole shell, including `orchester-desktop`,
`orchester-native-chrome` and `orchester-netz`, and
`werkzeug/desktop/build-installer.mjs --arch x64` then produces the NSIS
installer. x64 binaries run on this ARM64 machine under emulation, which is how
the local installation was refreshed.

A native `aarch64-pc-windows-msvc` build is still unavailable locally: the
installed MSVC toolsets ship only x64 and x86 host/target binaries, and
`lib\arm64` carries the Clang runtimes without `msvcrt.lib`. Add the ARM64
MSVC toolset (or use the `windows-11-arm` runner in the release workflow)
before claiming a native ARM64 build.

The doctor confirms this host as `win32/arm64`, reports Node.js and pnpm as
usable for the WebUI, and reports `windows-linker-shadowed` plus
`windows-msvc-compiler-missing` for the desktop profile whenever the MSVC
Developer environment has not been entered. Do not mark a native Tauri build
successful until those failures are gone and the actual Rust build command
completes.

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
