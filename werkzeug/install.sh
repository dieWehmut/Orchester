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

have_cmd() {
  command -v "$1" >/dev/null 2>&1
}

run_as_root() {
  if [ "$(id -u 2>/dev/null || echo 1)" = "0" ]; then
    "$@"
  elif have_cmd sudo; then
    sudo "$@"
  else
    die "root privileges are required to install missing system packages; install sudo or rerun as root"
  fi
}

winget_install() {
  pkg="$1"
  if have_cmd powershell.exe; then
    powershell.exe -NoProfile -ExecutionPolicy Bypass -Command \
      "if (Get-Command winget -ErrorAction SilentlyContinue) { winget install --id '$pkg' -e --silent --accept-package-agreements --accept-source-agreements; exit \$LASTEXITCODE } else { exit 127 }" \
      >/dev/null 2>&1
  else
    return 127
  fi
}

install_system_deps() {
  info "Checking/installing system dependencies"

  case "$(uname -s 2>/dev/null || echo unknown)" in
    MINGW*|MSYS*|CYGWIN*)
      if have_cmd pacman; then
        pacman -Sy --needed --noconfirm git curl mingw-w64-x86_64-gcc >/dev/null
        prepend_dir "/mingw64/bin"
      else
        have_cmd git || winget_install "Git.Git" || true
        if ! have_cmd gcc; then
          winget_install "MSYS2.MSYS2" || true
        fi
      fi
      ;;
    Darwin)
      if have_cmd brew; then
        brew install git curl >/dev/null
      else
        xcode-select --install >/dev/null 2>&1 || true
      fi
      ;;
    Linux|*)
      if have_cmd apt-get; then
        run_as_root apt-get update -y >/dev/null
        run_as_root apt-get install -y git curl ca-certificates build-essential >/dev/null
      elif have_cmd dnf; then
        run_as_root dnf install -y git curl ca-certificates gcc gcc-c++ make >/dev/null
      elif have_cmd yum; then
        run_as_root yum install -y git curl ca-certificates gcc gcc-c++ make >/dev/null
      elif have_cmd pacman; then
        run_as_root pacman -Sy --needed --noconfirm git curl ca-certificates base-devel >/dev/null
      elif have_cmd apk; then
        run_as_root apk add --no-cache git curl ca-certificates build-base >/dev/null
      elif have_cmd zypper; then
        run_as_root zypper --non-interactive install git curl ca-certificates gcc gcc-c++ make >/dev/null
      elif have_cmd brew; then
        brew install git curl >/dev/null
      else
        warn "No supported package manager found; continuing with existing tools"
      fi
      ;;
  esac
}

download_to_file() {
  url="$1"
  dest="$2"
  if have_cmd curl; then
    curl -fsSL "$url" -o "$dest"
  elif have_cmd wget; then
    wget -qO "$dest" "$url"
  else
    return 127
  fi
}

install_rustup() {
  info "Installing Rust toolchain"

  case "$(uname -s 2>/dev/null || echo unknown)" in
    MINGW*|MSYS*|CYGWIN*)
      rustup_init="${TMPDIR:-/tmp}/rustup-init.exe"
      download_to_file "https://win.rustup.rs/x86_64" "$rustup_init" || die "failed to download rustup-init.exe"
      "$rustup_init" -y --profile minimal
      ;;
    *)
      rustup_sh="${TMPDIR:-/tmp}/rustup-init.sh"
      download_to_file "https://sh.rustup.rs" "$rustup_sh" || die "failed to download rustup-init.sh"
      sh "$rustup_sh" -y --profile minimal
      ;;
  esac

  [ -n "${HOME:-}" ] && prepend_dir "$HOME/.cargo/bin"
  [ -n "${USERNAME:-}" ] && prepend_dir "/c/Users/$USERNAME/.cargo/bin"
  prepend_dir "/c/Users/30119/.cargo/bin"
  [ -n "${CARGO_HOME:-}" ] && prepend_dir "$CARGO_HOME/bin"
  export PATH
}

ensure_dependencies() {
  missing=false
  have_cmd git || missing=true
  have_cmd curl || have_cmd wget || missing=true

  case "$(uname -s 2>/dev/null || echo unknown)" in
    MINGW*|MSYS*|CYGWIN*)
      have_cmd gcc || [ -x /d/software/gcc/mingw64/bin/gcc.exe ] || missing=true
      ;;
    *)
      have_cmd cc || have_cmd gcc || missing=true
      ;;
  esac

  if [ "$missing" = true ]; then
    install_system_deps
  fi

  if ! have_cmd cargo; then
    install_rustup
  fi

  if have_cmd rustup; then
    rustup default stable >/dev/null 2>&1 || true
    case "$(uname -s 2>/dev/null || echo unknown)" in
      MINGW*|MSYS*|CYGWIN*)
        rustup toolchain install stable-x86_64-pc-windows-gnu --profile minimal --force-non-host >/dev/null 2>&1 || true
        export RUSTUP_TOOLCHAIN="stable-x86_64-pc-windows-gnu"
        export CARGO_BUILD_TARGET="x86_64-pc-windows-gnu"
        rustup target add x86_64-pc-windows-gnu >/dev/null 2>&1 || true
        ;;
    esac
  fi

  have_cmd git || die "git is still missing after dependency installation"
  have_cmd cargo || die "cargo is still missing after Rust installation"
  have_cmd curl || have_cmd wget || die "curl or wget is still missing after dependency installation"
}

configure_windows_gnu_linker() {
  case "$(uname -s 2>/dev/null || echo unknown)" in
    MINGW*|MSYS*|CYGWIN*)
      if have_cmd gcc; then
        gcc_path="$(command -v gcc)"
        ar_path="$(command -v ar 2>/dev/null || true)"
        if have_cmd cygpath; then
          gcc_path="$(cygpath -w "$gcc_path")"
          [ -n "$ar_path" ] && ar_path="$(cygpath -w "$ar_path")"
        fi
        export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="$gcc_path"
        [ -n "$ar_path" ] && export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_AR="$ar_path"
      fi
      ;;
  esac
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

ensure_dependencies
configure_windows_gnu_linker

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
