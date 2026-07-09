#!/bin/sh
# Orchester one-line installer.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/dieWehmut/Orchester/main/werkzeug/install.sh | sh
#
# Useful overrides:
#   ORCHESTER_REPO=https://github.com/dieWehmut/Orchester.git
#   ORCHESTER_REF=main
#   ORCHESTER_INSTALL_ROOT="$HOME/.cargo"

set -eu

REPO_URL="${ORCHESTER_REPO:-https://github.com/dieWehmut/Orchester.git}"
REF="${ORCHESTER_REF:-main}"
if [ -n "${ORCHESTER_INSTALL_ROOT:-}" ]; then
  INSTALL_ROOT="$ORCHESTER_INSTALL_ROOT"
elif [ -n "${HOME:-}" ]; then
  INSTALL_ROOT="$HOME/.cargo"
elif [ -n "${USERPROFILE:-}" ]; then
  INSTALL_ROOT="$USERPROFILE/.cargo"
else
  INSTALL_ROOT=""
fi
NO_PATH_UPDATE="${ORCHESTER_NO_PATH_UPDATE:-false}"
KEEP_TMP="${ORCHESTER_KEEP_TMP:-false}"

if [ -t 1 ]; then
  C_RED="$(printf '\033[31m')"
  C_GREEN="$(printf '\033[32m')"
  C_YELLOW="$(printf '\033[33m')"
  C_BLUE="$(printf '\033[36m')"
  C_DIM="$(printf '\033[2m')"
  C_RESET="$(printf '\033[0m')"
else
  C_RED=""
  C_GREEN=""
  C_YELLOW=""
  C_BLUE=""
  C_DIM=""
  C_RESET=""
fi

info() { printf '%s[*]%s %s\n' "$C_BLUE" "$C_RESET" "$*"; }
ok() { printf '%s[+]%s %s\n' "$C_GREEN" "$C_RESET" "$*"; }
warn() { printf '%s[!]%s %s\n' "$C_YELLOW" "$C_RESET" "$*" >&2; }
err() { printf '%s[x]%s %s\n' "$C_RED" "$C_RESET" "$*" >&2; }
die() { err "$*"; exit 1; }

prepend_dir() {
  if [ -n "${1:-}" ] && [ -d "$1" ]; then
    PATH="$1:$PATH"
  fi
  return 0
}

usage() {
  cat <<EOF
Orchester installer

Usage:
  curl -fsSL https://raw.githubusercontent.com/dieWehmut/Orchester/main/werkzeug/install.sh | sh
  curl -fsSL https://raw.githubusercontent.com/dieWehmut/Orchester/main/werkzeug/install.sh | sh -s -- --ref main

Options:
  --repo <url>       Git repository to clone. Default: $REPO_URL
  --ref <ref>        Branch, tag, or commit to install. Default: $REF
  --root <dir>       Cargo install root. Default: $INSTALL_ROOT
  --no-path-update   Do not append the install bin directory to shell profiles.
  --keep-tmp         Keep the temporary cloned source directory.
  -h, --help         Show this help.

Environment:
  ORCHESTER_REPO
  ORCHESTER_REF
  ORCHESTER_INSTALL_ROOT
  ORCHESTER_NO_PATH_UPDATE=true
  ORCHESTER_KEEP_TMP=true
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --repo)
      [ "$#" -ge 2 ] || die "--repo requires a value"
      REPO_URL="$2"
      shift 2
      ;;
    --ref)
      [ "$#" -ge 2 ] || die "--ref requires a value"
      REF="$2"
      shift 2
      ;;
    --root)
      [ "$#" -ge 2 ] || die "--root requires a value"
      INSTALL_ROOT="$2"
      shift 2
      ;;
    --no-path-update)
      NO_PATH_UPDATE=true
      shift
      ;;
    --keep-tmp)
      KEEP_TMP=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      usage >&2
      die "unknown option: $1"
      ;;
  esac
done

[ -n "$INSTALL_ROOT" ] || die "could not determine install root; set ORCHESTER_INSTALL_ROOT or HOME"

case "$(uname -s 2>/dev/null || echo unknown)" in
  MINGW*|MSYS*|CYGWIN*)
    EXE_SUFFIX=".exe"
    # Match this repo's current Windows development setup when those paths exist.
    if [ -z "${RUSTUP_HOME:-}" ] && [ -d /d/rust/rustup ]; then
      export RUSTUP_HOME="D:/rust/rustup"
    fi
    if [ -z "${CARGO_HOME:-}" ] && [ -d /d/rust/cargo ]; then
      export CARGO_HOME="D:/rust/cargo"
    fi
    if [ -d /d/software/gcc/mingw64/bin ]; then
      PATH="/d/software/gcc/mingw64/bin:$PATH"
    fi
    prepend_dir "/d/software/git/Git/cmd"
    prepend_dir "/d/software/git/Git/bin"
    prepend_dir "/c/Program Files/Git/cmd"
    prepend_dir "/c/Program Files/Git/bin"
    [ -n "${HOME:-}" ] && prepend_dir "$HOME/.cargo/bin"
    [ -n "${USERNAME:-}" ] && prepend_dir "/c/Users/$USERNAME/.cargo/bin"
    prepend_dir "/c/Users/30119/.cargo/bin"
    ;;
  *)
    EXE_SUFFIX=""
    ;;
esac

[ -n "${CARGO_HOME:-}" ] && prepend_dir "$CARGO_HOME/bin"
prepend_dir "$INSTALL_ROOT/bin"
export PATH

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "$1 was not found; install it and rerun this installer"
}

need_cmd cargo
need_cmd git

SCRIPT_DIR=""
case "${0:-}" in
  */*)
    SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" 2>/dev/null && pwd || true)"
    ;;
esac

LOCAL_ROOT=""
if [ -n "$SCRIPT_DIR" ] && [ -f "$SCRIPT_DIR/../Cargo.toml" ] && [ -d "$SCRIPT_DIR/../kisten/konsole" ]; then
  LOCAL_ROOT="$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)"
fi

TMP_DIR=""
cleanup() {
  if [ -n "$TMP_DIR" ] && [ "$KEEP_TMP" != "true" ]; then
    rm -rf "$TMP_DIR"
  elif [ -n "$TMP_DIR" ]; then
    warn "kept temporary source at $TMP_DIR"
  fi
}
trap cleanup EXIT INT TERM

if [ -n "$LOCAL_ROOT" ]; then
  SRC_DIR="$LOCAL_ROOT"
  info "Using local source: $SRC_DIR"
else
  TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/orchester-install.XXXXXX")"
  SRC_DIR="$TMP_DIR/src"
  info "Cloning Orchester: $REPO_URL ($REF)"
  git clone --depth 1 --branch "$REF" "$REPO_URL" "$SRC_DIR" >/dev/null 2>&1 || {
    warn "shallow branch clone failed; trying full clone and checkout"
    git clone "$REPO_URL" "$SRC_DIR" >/dev/null 2>&1
    (cd "$SRC_DIR" && git checkout "$REF" >/dev/null 2>&1)
  }
fi

[ -f "$SRC_DIR/Cargo.toml" ] || die "source directory does not look like Orchester: $SRC_DIR"
[ -d "$SRC_DIR/kisten/konsole" ] || die "orchester-konsole crate not found in $SRC_DIR"

BIN_DIR="$INSTALL_ROOT/bin"
mkdir -p "$BIN_DIR"

info "Installing orchester to $BIN_DIR"
(
  cd "$SRC_DIR"
  cargo install --path kisten/konsole --force --root "$INSTALL_ROOT"
)

BIN="$BIN_DIR/orchester$EXE_SUFFIX"
[ -x "$BIN" ] || [ -f "$BIN" ] || die "install completed but $BIN was not found"

append_path_line() {
  profile="$1"
  line="export PATH=\"$BIN_DIR:\$PATH\""
  [ -f "$profile" ] || : > "$profile"
  if ! grep -F "$BIN_DIR" "$profile" >/dev/null 2>&1; then
    printf '\n# Orchester CLI\n%s\n' "$line" >> "$profile"
    ok "Added $BIN_DIR to $profile"
  fi
}

if [ "$NO_PATH_UPDATE" != "true" ]; then
  case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *)
      if [ -n "${HOME:-}" ]; then
        append_path_line "$HOME/.profile"
        if [ -f "$HOME/.bashrc" ]; then
          append_path_line "$HOME/.bashrc"
        fi
        if [ -f "$HOME/.zshrc" ]; then
          append_path_line "$HOME/.zshrc"
        fi
        warn "$BIN_DIR was added to your shell profile. Open a new terminal or run: export PATH=\"$BIN_DIR:\$PATH\""
      else
        warn "$BIN_DIR is not on PATH. Add it manually before running orchester globally."
      fi
      ;;
  esac
fi

ok "Installed $BIN"
info "Version check:"
"$BIN" --version
ok "Done. Try: orchester doctor"
