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

sha256_file() {
  file="$1"
  if have_cmd sha256sum; then
    sha256sum "$file" | cut -d ' ' -f 1
  elif have_cmd shasum; then
    shasum -a 256 "$file" | cut -d ' ' -f 1
  elif have_cmd openssl; then
    openssl dgst -sha256 "$file" | awk '{print $NF}'
  else
    return 127
  fi
}

receipt_value() {
  case "$1" in
    *[![:print:]]*) return 1 ;;
    *) return 0 ;;
  esac
}

prepend_dir() {
  if [ -n "${1:-}" ] && [ -d "$1" ]; then
    PATH="$1:$PATH"
  fi
  return 0
}

have_cmd() {
  command -v "$1" >/dev/null 2>&1
}

is_windows_shell() {
  case "$(uname -s 2>/dev/null || echo unknown)" in
    MINGW*|MSYS*|CYGWIN*) return 0 ;;
    *) return 1 ;;
  esac
}

to_windows_path() {
  path="$1"
  if have_cmd cygpath; then
    cygpath -aw "$path"
  else
    printf '%s\n' "$path"
  fi
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
          for pacman in /c/msys64/usr/bin/pacman.exe /c/msys/usr/bin/pacman.exe; do
            if [ -x "$pacman" ]; then
              "$pacman" -Sy --needed --noconfirm mingw-w64-x86_64-gcc >/dev/null
              break
            fi
          done
          prepend_dir "/c/msys64/mingw64/bin"
          prepend_dir "/c/msys/mingw64/bin"
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

  (
    rustup_tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/orchester-rustup.XXXXXX")" || exit 1
    trap 'rm -rf "$rustup_tmp_dir"' EXIT INT TERM
    case "$(uname -s 2>/dev/null || echo unknown)" in
      MINGW*|MSYS*|CYGWIN*)
        rustup_init="$rustup_tmp_dir/rustup-init.exe"
        download_to_file "https://win.rustup.rs/x86_64" "$rustup_init" || exit 1
        "$rustup_init" -y --profile minimal
        ;;
      *)
        rustup_sh="$rustup_tmp_dir/rustup-init.sh"
        download_to_file "https://sh.rustup.rs" "$rustup_sh" || exit 1
        sh "$rustup_sh" -y --profile minimal
        ;;
    esac
  ) || die "Rust toolchain installation failed"

  [ -n "${HOME:-}" ] && prepend_dir "$HOME/.cargo/bin"
  if [ -n "${USERPROFILE:-}" ] && have_cmd cygpath; then
    user_profile_unix="$(cygpath -u "$USERPROFILE" 2>/dev/null || true)"
    [ -n "$user_profile_unix" ] && prepend_dir "$user_profile_unix/.cargo/bin"
  fi
  [ -n "${CARGO_HOME:-}" ] && prepend_dir "$CARGO_HOME/bin"
  export PATH
}

ensure_dependencies() {
  missing=false
  have_cmd git || missing=true
  have_cmd curl || have_cmd wget || missing=true

  case "$(uname -s 2>/dev/null || echo unknown)" in
    MINGW*|MSYS*|CYGWIN*)
      have_cmd gcc || missing=true
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

configure_cargo_build_target() {
  case "$(uname -s 2>/dev/null || echo unknown)" in
    MINGW*|MSYS*|CYGWIN*)
      export CARGO_BUILD_TARGET="${CARGO_BUILD_TARGET:-x86_64-pc-windows-gnu}"
      ;;
    *)
      host_triple="$(rustc -vV 2>/dev/null | sed -n 's/^host: //p' | head -n 1)"
      [ -n "$host_triple" ] || die "could not detect Rust host target"
      export CARGO_BUILD_TARGET="$host_triple"
      case "${RUSTUP_TOOLCHAIN:-}" in
        *windows*) unset RUSTUP_TOOLCHAIN ;;
      esac
      if have_cmd rustup; then
        rustup target add "$host_triple" >/dev/null 2>&1 || true
      fi
      ;;
  esac
}

ensure_windows_user_path() {
  bin_dir_win="$(to_windows_path "$1")" || return 1
  have_cmd powershell.exe || return 1

  result="$(
    WIN_PATH_ITEM="$bin_dir_win" powershell.exe -NoProfile -ExecutionPolicy Bypass -Command '
$ErrorActionPreference = "Stop"
$item = $env:WIN_PATH_ITEM
function Normalize-PathText([string]$Path) {
    if ([string]::IsNullOrWhiteSpace($Path)) { return "" }
    return $Path.Trim().TrimEnd("\").ToLowerInvariant()
}
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$parts = @()
if (-not [string]::IsNullOrWhiteSpace($userPath)) {
    $parts = $userPath -split ";" | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
}
$exists = $false
foreach ($part in $parts) {
    if ((Normalize-PathText $part) -eq (Normalize-PathText $item)) {
        $exists = $true
        break
    }
}
if ($exists) {
    Write-Output "exists"
    exit 0
}
$newPath = if ([string]::IsNullOrWhiteSpace($userPath)) { $item } else { "$userPath;$item" }
[Environment]::SetEnvironmentVariable("Path", $newPath, "User")
Write-Output "added"
' 2>/dev/null | tr -d '\r'
  )" || return 1

  case "$result" in
    added)
      WINDOWS_PATH_ITEM="$bin_dir_win"
      WINDOWS_PATH_ADDED="1"
      ok "Added $bin_dir_win to Windows user PATH"
      ;;
    exists)
      ok "Windows user PATH already includes $bin_dir_win"
      ;;
    *)
      return 1
      ;;
  esac
}

ensure_windows_command_shim() {
  target_win="$(to_windows_path "$1")" || return 1
  shim_dir_win=""
  if [ -n "${ORCHESTER_WINDOWS_SHIM_DIR:-}" ]; then
    shim_dir_win="$(to_windows_path "$ORCHESTER_WINDOWS_SHIM_DIR")" || return 1
  fi
  have_cmd powershell.exe || return 1

  result="$(
    WIN_ORCHESTER_TARGET="$target_win" WIN_ORCHESTER_SHIM_DIR="$shim_dir_win" powershell.exe -NoProfile -ExecutionPolicy Bypass -Command '
$ErrorActionPreference = "Stop"
$target = $env:WIN_ORCHESTER_TARGET
$requestedDir = $env:WIN_ORCHESTER_SHIM_DIR

function Normalize-PathText([string]$Path) {
    if ([string]::IsNullOrWhiteSpace($Path)) { return "" }
    try {
        return ([System.IO.Path]::GetFullPath($Path)).TrimEnd("\").ToLowerInvariant()
    } catch {
        return $Path.Trim().TrimEnd("\").ToLowerInvariant()
    }
}

function Test-PathInProcessPath([string]$Dir) {
    $needle = Normalize-PathText $Dir
    $processPath = [Environment]::GetEnvironmentVariable("Path", "Process")
    foreach ($part in ($processPath -split ";")) {
        if ((Normalize-PathText $part) -eq $needle) {
            return $true
        }
    }
    return $false
}

function Test-WritableDirectory([string]$Dir) {
    try {
        if (-not (Test-Path -LiteralPath $Dir)) {
            New-Item -ItemType Directory -Force -Path $Dir | Out-Null
        }
        $probe = Join-Path $Dir ".orchester-write-test-$PID"
        Set-Content -LiteralPath $probe -Value "" -Encoding ASCII
        Remove-Item -LiteralPath $probe -Force
        return $true
    } catch {
        return $false
    }
}

$shimDir = $null
if (-not [string]::IsNullOrWhiteSpace($requestedDir)) {
    $shimDir = $requestedDir
} else {
    $candidates = @()
    $localAppData = $env:LOCALAPPDATA
    if ([string]::IsNullOrWhiteSpace($localAppData) -and -not [string]::IsNullOrWhiteSpace($env:USERPROFILE)) {
        $localAppData = Join-Path $env:USERPROFILE "AppData\Local"
    }
    if (-not [string]::IsNullOrWhiteSpace($localAppData)) {
        $windowsApps = Join-Path $localAppData "Microsoft\WindowsApps"
        if (Test-WritableDirectory $windowsApps) {
            $shimDir = $windowsApps
        }
    }
    if ([string]::IsNullOrWhiteSpace($shimDir)) {
        if (-not [string]::IsNullOrWhiteSpace($env:USERPROFILE)) {
            $candidates += Join-Path $env:USERPROFILE "bin"
        }
        $processPath = [Environment]::GetEnvironmentVariable("Path", "Process")
        foreach ($part in ($processPath -split ";")) {
            if ([string]::IsNullOrWhiteSpace($part) -or [string]::IsNullOrWhiteSpace($env:USERPROFILE)) {
                continue
            }
            if ((Normalize-PathText $part).StartsWith((Normalize-PathText $env:USERPROFILE))) {
                $candidates += $part
            }
        }

        $seen = @{}
        foreach ($candidate in $candidates) {
            $key = Normalize-PathText $candidate
            if ([string]::IsNullOrWhiteSpace($key) -or $seen.ContainsKey($key)) {
                continue
            }
            $seen[$key] = $true
            if ((Test-Path -LiteralPath $candidate) -and (Test-PathInProcessPath $candidate) -and (Test-WritableDirectory $candidate)) {
                $shimDir = $candidate
                break
            }
        }
    }
}

if ([string]::IsNullOrWhiteSpace($shimDir) -or -not (Test-WritableDirectory $shimDir)) {
    Write-Output "SKIPPED"
    exit 0
}

$shim = Join-Path $shimDir "orchester.cmd"
$commandLine = """" + $target + """ %*"
Set-Content -LiteralPath $shim -Value @("@echo off", $commandLine) -Encoding ASCII
Write-Output $shim
' 2>/dev/null | tr -d '\r'
  )" || return 1

  if [ "$result" = "SKIPPED" ] || [ -z "$result" ]; then
    return 1
  fi

  ok "Added Windows command shim: $result"
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
  ORCHESTER_WINDOWS_SHIM_DIR
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
mkdir -p "$INSTALL_ROOT"
INSTALL_ROOT="$(CDPATH= cd -- "$INSTALL_ROOT" && pwd)" || die "could not resolve install root"

case "$(uname -s 2>/dev/null || echo unknown)" in
  MINGW*|MSYS*|CYGWIN*)
    EXE_SUFFIX=".exe"
    [ -n "${ORCHESTER_GCC_BIN_DIR:-}" ] && prepend_dir "$ORCHESTER_GCC_BIN_DIR"
    [ -n "${ORCHESTER_GIT_BIN_DIR:-}" ] && prepend_dir "$ORCHESTER_GIT_BIN_DIR"
    [ -n "${HOME:-}" ] && prepend_dir "$HOME/.cargo/bin"
    if [ -n "${USERPROFILE:-}" ] && have_cmd cygpath; then
      user_profile_unix="$(cygpath -u "$USERPROFILE" 2>/dev/null || true)"
      [ -n "$user_profile_unix" ] && prepend_dir "$user_profile_unix/.cargo/bin"
    fi
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
configure_cargo_build_target

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
PATH_RECORDS_FILE=""
RECEIPT_TMP_FILE=""
cleanup() {
  if [ -n "$TMP_DIR" ] && [ "$KEEP_TMP" != "true" ]; then
    rm -rf "$TMP_DIR"
  elif [ -n "$TMP_DIR" ]; then
    warn "kept temporary source at $TMP_DIR"
  fi
  [ -z "$PATH_RECORDS_FILE" ] || rm -f "$PATH_RECORDS_FILE"
  [ -z "$RECEIPT_TMP_FILE" ] || rm -f "$RECEIPT_TMP_FILE"
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
  cargo install --locked --path kisten/konsole --force --root "$INSTALL_ROOT" --target "$CARGO_BUILD_TARGET"
)

BIN="$BIN_DIR/orchester$EXE_SUFFIX"
[ -x "$BIN" ] || [ -f "$BIN" ] || die "install completed but $BIN was not found"

PATH_RECORDS_FILE="$(mktemp "${TMPDIR:-/tmp}/orchester-paths.XXXXXX")" || die "could not create receipt staging file"
SHIM=""
WINDOWS_PATH_ITEM=""
WINDOWS_PATH_ADDED="0"

append_path_line() {
  profile="$1"
  line="export PATH=\"$BIN_DIR:\$PATH\""
  [ -f "$profile" ] || : > "$profile"
  if ! grep -F "$line" "$profile" >/dev/null 2>&1; then
    printf '\n# Orchester CLI\n%s\n' "$line" >> "$profile"
    receipt_value "$profile" || die "profile path contains unsupported control characters: $profile"
    receipt_value "$line" || die "profile PATH line contains unsupported control characters"
    printf 'path_profile\t%s\npath_added\t1\npath_line\t%s\npath_marker\t# Orchester CLI\n' \
      "$profile" "$line" >> "$PATH_RECORDS_FILE"
    ok "Added $BIN_DIR to $profile"
  fi
}

if [ "$NO_PATH_UPDATE" != "true" ]; then
  if is_windows_shell; then
    ensure_windows_user_path "$BIN_DIR" || warn "Could not update Windows user PATH automatically; add $(to_windows_path "$BIN_DIR") manually."
    if shim_result="$(ensure_windows_command_shim "$BIN")"; then
      SHIM="$shim_result"
    else
      warn "Open a new Windows terminal before running 'orchester' if this terminal cannot find it."
    fi
  fi

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

RECEIPT_DIR="$INSTALL_ROOT/.orchester"
mkdir -p "$RECEIPT_DIR"
BINARY_HASH="$(sha256_file "$BIN")" || die "sha256 tool is required to write the install receipt"
[ "${#BINARY_HASH}" -eq 64 ] || die "could not calculate a valid binary hash"
case "$BINARY_HASH" in
  *[!0-9a-f]*) die "could not calculate a lowercase binary hash" ;;
esac
SHIM_HASH=""
if [ -n "$SHIM" ]; then
  SHIM_HASH="$(sha256_file "$SHIM")" || die "could not calculate the command shim hash"
fi
receipt_value "$INSTALL_ROOT" || die "install root contains unsupported control characters"
receipt_value "$BIN" || die "binary path contains unsupported control characters"
RECEIPT_TMP_FILE="$(mktemp "$RECEIPT_DIR/.install.receipt.XXXXXX")" || die "could not stage install receipt"
{
  printf 'schema\t1\n'
  printf 'install_root\t%s\n' "$INSTALL_ROOT"
  printf 'bin\t%s\n' "$BIN"
  printf 'binary_hash\t%s\n' "$BINARY_HASH"
  printf 'shim\t%s\n' "$SHIM"
  printf 'shim_hash\t%s\n' "$SHIM_HASH"
  if [ "$WINDOWS_PATH_ADDED" = "1" ]; then
    printf 'windows_path_item\t%s\nwindows_path_added\t1\n' "$WINDOWS_PATH_ITEM"
  fi
  cat "$PATH_RECORDS_FILE"
} > "$RECEIPT_TMP_FILE"
chmod 600 "$RECEIPT_TMP_FILE"
mv -f "$RECEIPT_TMP_FILE" "$RECEIPT_DIR/install.receipt"
RECEIPT_TMP_FILE=""

ok "Installed $BIN"
info "Version check:"
"$BIN" --version
if [ "$NO_PATH_UPDATE" = "true" ]; then
  ok "Done. Try: $BIN doctor"
else
  ok "Done. Try: orchester doctor"
fi
