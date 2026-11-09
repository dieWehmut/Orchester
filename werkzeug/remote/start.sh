#!/usr/bin/env bash
# Build and serve the remote controller from any working directory.
set -euo pipefail
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
if [[ $# -eq 0 ]]; then
  printf 'Usage: %s --workspace /absolute/project [--allow-root /absolute/other-project] [--bind 0.0.0.0] [--port 7337]\n' "$0"
  exit 2
fi
# Resolve a relative override from the caller's directory before entering the
# repository, and build for this host even if a cross-build target is exported.
target_root="${CARGO_TARGET_DIR:-$repo_root/target}"
if [[ "$target_root" != /* ]]; then target_root="$PWD/$target_root"; fi
host_target="$(rustc -vV | sed -n 's/^host: //p')"
if [[ -z "$host_target" ]]; then
  printf 'Cannot determine the native Rust host target.\n' >&2
  exit 1
fi
cd -- "$repo_root"
pnpm --filter @orchester/mobile build
cargo build --locked --target "$host_target" --target-dir "$target_root" -p orchester-konsole
# Keep remote device credentials and tasks separate from desktop state.
export ORCHESTER_HOME="${ORCHESTER_HOME:-${XDG_STATE_HOME:-$HOME/.local/state}/orchester-remote}"
binary="$target_root/$host_target/debug/orchester"
if [[ "$host_target" == *-windows-* ]]; then binary+='.exe'; fi
exec "$binary" remote --assets "$repo_root/apps/mobile/dist" "$@"
