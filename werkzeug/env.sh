#!/usr/bin/env bash
# Orchester build environment.
#
# The host is ARM64 Windows with no MSVC toolchain, and C: is full, so we build
# with the GNU host toolchain + MinGW linker, and keep rustup/cargo homes on D:.
# Source this before any cargo/rustup command:  `source werkzeug/env.sh`
export RUSTUP_HOME="D:/rust/rustup"
export CARGO_HOME="D:/rust/cargo"
export PATH="/c/Users/30119/.cargo/bin:/d/software/gcc/mingw64/bin:$PATH"

# Build scripts and proc macros always compile for the *host* triple, so
# CARGO_BUILD_TARGET alone cannot rescue a host toolchain that cannot link.
# Pin the toolchain instead: this makes the host x86_64-pc-windows-gnu, and
# .cargo/config.toml then routes it at the MinGW linker.
export RUSTUP_TOOLCHAIN="stable-x86_64-pc-windows-gnu"

# Sanity echo (only when run directly, not when sourced silently).
if [ "${BASH_SOURCE[0]}" = "${0}" ]; then
  cargo --version
  rustc --version --verbose | grep host
fi
