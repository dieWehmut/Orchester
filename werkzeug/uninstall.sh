#!/bin/sh
# Remove an Orchester installation created by the receipt-aware installer.
#
# The installer must write a TSV receipt at:
#   <install-root>/.orchester/install.receipt
#
# No receipt is an error when an Orchester binary is present. A missing
# receipt and missing binary are an idempotent no-op.

set -eu
umask 077

TAB=$(printf '\t')
CR=$(printf '\r')
STATE_DIR=''
TEMP_PROFILE=''

cleanup() {
  if [ -n "${TEMP_PROFILE:-}" ] && [ -e "$TEMP_PROFILE" ]; then
    rm -f "$TEMP_PROFILE" 2>/dev/null || true
  fi
  if [ -n "${STATE_DIR:-}" ] && [ -d "$STATE_DIR" ]; then
    rm -rf "$STATE_DIR" 2>/dev/null || true
  fi
}
trap cleanup EXIT HUP INT TERM

err() {
  printf '%s\n' "orchester uninstall: $*" >&2
}

die() {
  err "$*"
  exit 1
}

info() {
  printf '%s\n' "orchester uninstall: $*"
}

usage() {
  cat <<'EOF'
Orchester uninstaller

Usage:
  werkzeug/uninstall.sh [--root <dir>] [--purge] [--no-path-update]

Options:
  --root <dir>       Cargo install root. Defaults to ORCHESTER_INSTALL_ROOT,
                     $HOME/.cargo, or $USERPROFILE/.cargo.
  --purge             Remove known user configuration files and empty config
                      directories after the installation is removed.
  --no-path-update    Do not modify shell profiles or the Windows user PATH.
  -h, --help          Show this help.
EOF
}

has_control_bytes() {
  # Keep UTF-8/user-selected path bytes valid, but reject ASCII controls. A
  # tab is handled separately because it is the receipt field separator.
  HCB_CLEANED=$(printf '%s' "$1" | LC_ALL=C tr -d '\001-\037\177')
  [ "$HCB_CLEANED" != "$1" ]
}

validate_text() {
  if has_control_bytes "$1"; then
    die "receipt contains control characters"
  fi
  case "$1" in
    *"$TAB"*) die "receipt value contains a tab" ;;
  esac
}

validate_abs_path() {
  VAP_PATH=$1
  validate_text "$VAP_PATH"
  case "$VAP_PATH" in
    /*) ;;
    *) die "path is not absolute: $VAP_PATH" ;;
  esac
  case "$VAP_PATH" in
    /|*/../*|*/..|../*|..) die "path contains an unsafe parent component: $VAP_PATH" ;;
  esac
}

canonical_dir() {
  directory=$1
  CDPATH= cd -P -- "$directory" 2>/dev/null && pwd -P
}

normalize_local_path() {
  candidate=$1
  case "$candidate" in
    [A-Za-z]:/*|[A-Za-z]:\\*)
      command -v cygpath >/dev/null 2>&1 || die "Windows receipt path requires cygpath: $candidate"
      candidate=$(cygpath -u -- "$candidate" | tr -d '\r') || die "cannot convert receipt path: $candidate"
      ;;
  esac
  validate_abs_path "$candidate"
  printf '%s\n' "$candidate"
}

normalize_windows_path() {
  value=$1
  printf '%s' "$value" | tr '\\/' '//' | sed 's#/*$##' | tr '[:upper:]' '[:lower:]'
}

hash_file() {
  path=$1
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$path" | sed 's/[[:space:]].*$//'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$path" | sed 's/[[:space:]].*$//'
  else
    die "sha256sum or shasum is required to verify installed files"
  fi
}

validate_hash() {
  hash=$1
  case "$hash" in
    ''|*[!0123456789abcdef]*) die "receipt hash is not lowercase SHA-256" ;;
  esac
  [ "${#hash}" -eq 64 ] || die "receipt hash must contain 64 hexadecimal characters"
}

is_link() {
  [ -L "$1" ] 2>/dev/null || [ -h "$1" ] 2>/dev/null
}

remove_owned_file() {
  path=$1
  expected_hash=$2
  label=$3
  expected_parent=$4

  if is_link "$path"; then
    die "$label is a symlink; refusing to remove it: $path"
  fi
  if [ ! -e "$path" ]; then
    return 0
  fi
  [ -f "$path" ] || die "$label is not a regular file: $path"
  actual_parent=$(canonical_dir "$(dirname "$path")") \
    || die "could not resolve $label parent directory"
  [ "$actual_parent" = "$expected_parent" ] \
    || die "$label parent directory changed; refusing to remove it: $path"
  actual_hash=$(hash_file "$path")
  [ "$actual_hash" = "$expected_hash" ] || die "$label was modified; refusing to remove it: $path"
  actual_parent=$(canonical_dir "$(dirname "$path")") \
    || die "could not revalidate $label parent directory"
  [ "$actual_parent" = "$expected_parent" ] \
    || die "$label parent directory changed; refusing to remove it: $path"
  rm -f "$path" || die "could not remove $label: $path"
}

profile_is_supported() {
  profile=$1
  home_dir=${HOME:-}
  [ -n "$home_dir" ] || return 1
  case "$home_dir" in
    /*) ;;
    *) return 1 ;;
  esac
  [ "$profile" = "$home_dir/.profile" ] \
    || [ "$profile" = "$home_dir/.bashrc" ] \
    || [ "$profile" = "$home_dir/.zshrc" ]
}

profile_mode() {
  profile=$1
  mode=''
  if command -v stat >/dev/null 2>&1; then
    mode=$(stat -c '%a' "$profile" 2>/dev/null || true)
    if [ -z "$mode" ]; then
      mode=$(stat -f '%Lp' "$profile" 2>/dev/null || true)
    fi
  fi
  printf '%s\n' "$mode"
}

rewrite_profile() {
  profile=$1
  path_line=$2
  marker=$3

  if is_link "$profile"; then
    die "profile is a symlink; refusing to modify it: $profile"
  fi
  [ -e "$profile" ] || return 0
  [ -f "$profile" ] || die "profile is not a regular file: $profile"

  # If the exact line is already gone, do not rewrite the profile or touch an
  # unrelated marker.
  if ! grep -F -x "$path_line" "$profile" >/dev/null 2>&1; then
    return 0
  fi

  profile_dir=$(dirname "$profile")
  profile_name=$(basename "$profile")
  TEMP_PROFILE=$(mktemp "$profile_dir/.${profile_name}.orchester-uninstall.XXXXXX") \
    || die "could not create a temporary profile in $profile_dir"
  [ ! -L "$TEMP_PROFILE" ] || die "temporary profile is a symlink"

  mode=$(profile_mode "$profile")
  if [ -n "$mode" ]; then
    chmod "$mode" "$TEMP_PROFILE" || die "could not preserve profile permissions"
  fi

  changed=0
  pending_marker=''
  while IFS= read -r current || [ -n "$current" ]; do
    if [ "$current" = "$path_line" ]; then
      pending_marker=''
      changed=1
      continue
    fi
    if [ -n "$pending_marker" ]; then
      printf '%s\n' "$pending_marker" >> "$TEMP_PROFILE"
      pending_marker=''
    fi
    if [ -n "$marker" ] && [ "$current" = "$marker" ]; then
      pending_marker=$current
    else
      printf '%s\n' "$current" >> "$TEMP_PROFILE"
    fi
  done < "$profile"
  if [ -n "$pending_marker" ]; then
    printf '%s\n' "$pending_marker" >> "$TEMP_PROFILE"
  fi

  if [ "$changed" -eq 1 ]; then
    mv -f "$TEMP_PROFILE" "$profile" || die "could not update profile: $profile"
  else
    rm -f "$TEMP_PROFILE"
  fi
  TEMP_PROFILE=''
}

remove_windows_path_item() {
  item=$1
  command -v powershell.exe >/dev/null 2>&1 \
    || die "powershell.exe is required to remove the recorded Windows PATH entry"
  result=$(
    WIN_PATH_ITEM=$item powershell.exe -NoProfile -ExecutionPolicy Bypass -Command '
$ErrorActionPreference = "Stop"
$item = $env:WIN_PATH_ITEM
function Normalize-AbsolutePathText([string]$Path) {
    if ([string]::IsNullOrWhiteSpace($Path)) { return $null }
    if (-not [System.IO.Path]::IsPathRooted($Path) -or $Path.Contains("%")) { return $null }
    try {
        return ([System.IO.Path]::GetFullPath($Path)).TrimEnd("\").ToLowerInvariant()
    } catch {
        return $null
    }
}
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$parts = if ($null -eq $userPath) { @() } else { @($userPath.Split([char]";")) }
$needle = Normalize-AbsolutePathText $item
if ($null -eq $needle) { throw "recorded PATH item is not an absolute path" }
$kept = @($parts | Where-Object {
    $candidate = Normalize-AbsolutePathText $_
    $null -eq $candidate -or $candidate -ne $needle
})
if ($kept.Count -eq $parts.Count) {
    Write-Output "absent"
    exit 0
}
[Environment]::SetEnvironmentVariable("Path", ($kept -join ";"), "User")
Write-Output "removed"
' 2>/dev/null | tr -d '\r'
  ) || die "could not update the Windows user PATH"
  case "$result" in
    removed|absent) ;;
    *) die "unexpected result while updating the Windows user PATH" ;;
  esac
}

purge_config() {
  config_dir=$1
  validate_abs_path "$config_dir"
  [ "$config_dir" != "/" ] || die "refusing to purge the filesystem root"
  if is_link "$config_dir"; then
    die "configuration directory is a symlink; refusing to purge it: $config_dir"
  fi
  if [ ! -e "$config_dir" ]; then
    return 0
  fi
  [ -d "$config_dir" ] || die "configuration path is not a directory: $config_dir"

  # Keep unknown files. A future release can add receipt fields for other
  # owned files without making this command delete unrelated user data.
  for name in orchester.jsonc sessions.jsonl; do
    candidate="$config_dir/$name"
    if is_link "$candidate"; then
      die "configuration file is a symlink; refusing to purge it: $candidate"
    fi
    if [ -e "$candidate" ]; then
      [ -f "$candidate" ] || die "configuration path is not a regular file: $candidate"
      rm -f "$candidate" || die "could not remove configuration file: $candidate"
    fi
  done
  rmdir "$config_dir" 2>/dev/null || true
}

validate_purge_config() {
  config_dir=$1
  validate_abs_path "$config_dir"
  [ "$config_dir" != "/" ] || die "refusing to purge the filesystem root"
  if is_link "$config_dir"; then
    die "configuration directory is a symlink; refusing to purge it: $config_dir"
  fi
  if [ ! -e "$config_dir" ]; then
    return 0
  fi
  [ -d "$config_dir" ] || die "configuration path is not a directory: $config_dir"
  for name in orchester.jsonc sessions.jsonl; do
    candidate="$config_dir/$name"
    if is_link "$candidate"; then
      die "configuration file is a symlink; refusing to purge it: $candidate"
    fi
    if [ -e "$candidate" ]; then
      [ -f "$candidate" ] || die "configuration path is not a regular file: $candidate"
    fi
  done
}

NO_PATH_UPDATE=false
PURGE=false
ROOT_INPUT=''

while [ "$#" -gt 0 ]; do
  case "$1" in
    --root)
      [ "$#" -ge 2 ] || die "--root requires a value"
      ROOT_INPUT=$2
      shift 2
      ;;
    --purge)
      PURGE=true
      shift
      ;;
    --no-path-update)
      NO_PATH_UPDATE=true
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

if [ -z "$ROOT_INPUT" ]; then
  if [ -n "${ORCHESTER_INSTALL_ROOT:-}" ]; then
    ROOT_INPUT=$ORCHESTER_INSTALL_ROOT
  elif [ -n "${HOME:-}" ]; then
    ROOT_INPUT="$HOME/.cargo"
  elif [ -n "${USERPROFILE:-}" ]; then
    ROOT_INPUT="$USERPROFILE/.cargo"
  else
    die "could not determine install root; set ORCHESTER_INSTALL_ROOT or use --root"
  fi
fi

ROOT_INPUT=$(normalize_local_path "$ROOT_INPUT")
if [ -d "$ROOT_INPUT" ]; then
  is_link "$ROOT_INPUT" && die "install root is a symlink; refusing to use it"
  ROOT=$(canonical_dir "$ROOT_INPUT") || die "could not resolve install root: $ROOT_INPUT"
else
  ROOT=$ROOT_INPUT
fi
[ "$ROOT" != "/" ] || die "refusing to operate on filesystem root"

RECEIPT="$ROOT/.orchester/install.receipt"
if [ ! -e "$RECEIPT" ]; then
  if is_link "$RECEIPT"; then
    die "receipt is a symlink; refusing to use it: $RECEIPT"
  fi
  for candidate in "$ROOT/bin/orchester" "$ROOT/bin/orchester.exe"; do
    if is_link "$candidate" || [ -e "$candidate" ]; then
      die "installation receipt is missing; refusing to remove $candidate"
    fi
  done
  info "no Orchester receipt or installed binary found; nothing to do"
  exit 0
fi

is_link "$ROOT/.orchester" && die "receipt directory is a symlink; refusing to use it"
is_link "$RECEIPT" && die "receipt is a symlink; refusing to use it"
[ -f "$RECEIPT" ] || die "receipt is not a regular file: $RECEIPT"

STATE_DIR=$(mktemp -d "${TMPDIR:-/tmp}/orchester-uninstall.XXXXXX") \
  || die "could not create a private temporary directory"
PROFILES_FILE="$STATE_DIR/profiles"
: > "$PROFILES_FILE"

SCHEMA=''
RECEIPT_ROOT=''
BIN=''
BINARY_HASH=''
SHIM=''
SHIM_HASH=''
VERSION=''
RECEIPT_CONFIG_DIR=''
WINDOWS_PATH_ITEM=''
WINDOWS_PATH_ADDED=''
SEEN_SCHEMA=0
SEEN_ROOT=0
SEEN_BIN=0
SEEN_BINARY_HASH=0
SEEN_SHIM=0
SEEN_SHIM_HASH=0
SEEN_VERSION=0
SEEN_CONFIG_DIR=0
SEEN_WINDOWS_PATH_ITEM=0
SEEN_WINDOWS_PATH_ADDED=0

CURRENT_PROFILE=''
CURRENT_LINE=''
CURRENT_ADDED=''
CURRENT_MARKER=''
CURRENT_HAS_LINE=0
CURRENT_HAS_ADDED=0
CURRENT_HAS_MARKER=0

flush_profile() {
  [ -n "$CURRENT_PROFILE" ] || return 0
  validate_abs_path "$CURRENT_PROFILE"
  if [ "$CURRENT_HAS_ADDED" -ne 1 ]; then
    die "path_profile is missing path_added"
  fi
  case "$CURRENT_ADDED" in
    0|1) ;;
    *) die "path_added must be 0 or 1" ;;
  esac
  if [ "$CURRENT_ADDED" = 1 ] && [ "$CURRENT_HAS_LINE" -ne 1 ]; then
    die "path_added=1 is missing path_line"
  fi
  if [ "$CURRENT_HAS_LINE" -eq 1 ]; then
    validate_text "$CURRENT_LINE"
  fi
  if [ "$CURRENT_HAS_MARKER" -eq 1 ]; then
    validate_text "$CURRENT_MARKER"
    [ "$CURRENT_ADDED" = 1 ] || die "path_marker requires path_added=1"
  fi
  printf '%s\t%s\t%s\t%s\n' \
    "$CURRENT_PROFILE" "$CURRENT_LINE" "$CURRENT_ADDED" "$CURRENT_MARKER" >> "$PROFILES_FILE"
  CURRENT_PROFILE=''
  CURRENT_LINE=''
  CURRENT_ADDED=''
  CURRENT_MARKER=''
  CURRENT_HAS_LINE=0
  CURRENT_HAS_ADDED=0
  CURRENT_HAS_MARKER=0
}

while IFS= read -r line || [ -n "$line" ]; do
  # PowerShell may emit CRLF; strip only the record terminator, then reject
  # any remaining control byte through validate_text.
  case "$line" in
    *"$CR") line=${line%"$CR"} ;;
  esac
  [ -n "$line" ] || continue
  case "$line" in
    *"$TAB"*)
      key=${line%%"$TAB"*}
      value=${line#*"$TAB"}
      [ "$key" != "$line" ] || die "receipt key is missing"
      case "$value" in
        *"$TAB"*) die "receipt value contains an extra tab" ;;
      esac
      ;;
    *) die "receipt line is not a TSV key/value pair" ;;
  esac
  validate_text "$key"
  validate_text "$value"
  case "$key" in
    schema)
      flush_profile
      [ "$SEEN_SCHEMA" -eq 0 ] || die "duplicate schema field"
      SCHEMA=$value
      SEEN_SCHEMA=1
      ;;
    install_root)
      flush_profile
      [ "$SEEN_ROOT" -eq 0 ] || die "duplicate install_root field"
      RECEIPT_ROOT=$value
      SEEN_ROOT=1
      ;;
    bin)
      flush_profile
      [ "$SEEN_BIN" -eq 0 ] || die "duplicate bin field"
      BIN=$value
      SEEN_BIN=1
      ;;
    binary_hash)
      flush_profile
      [ "$SEEN_BINARY_HASH" -eq 0 ] || die "duplicate binary_hash field"
      BINARY_HASH=$value
      SEEN_BINARY_HASH=1
      ;;
    shim)
      flush_profile
      [ "$SEEN_SHIM" -eq 0 ] || die "duplicate shim field"
      SHIM=$value
      SEEN_SHIM=1
      ;;
    shim_hash)
      flush_profile
      [ "$SEEN_SHIM_HASH" -eq 0 ] || die "duplicate shim_hash field"
      SHIM_HASH=$value
      SEEN_SHIM_HASH=1
      ;;
    version)
      flush_profile
      [ "$SEEN_VERSION" -eq 0 ] || die "duplicate version field"
      VERSION=$value
      SEEN_VERSION=1
      ;;
    config_dir)
      flush_profile
      [ "$SEEN_CONFIG_DIR" -eq 0 ] || die "duplicate config_dir field"
      RECEIPT_CONFIG_DIR=$value
      SEEN_CONFIG_DIR=1
      ;;
    windows_path_item)
      flush_profile
      [ "$SEEN_WINDOWS_PATH_ITEM" -eq 0 ] || die "duplicate windows_path_item field"
      WINDOWS_PATH_ITEM=$value
      SEEN_WINDOWS_PATH_ITEM=1
      ;;
    windows_path_added)
      flush_profile
      [ "$SEEN_WINDOWS_PATH_ADDED" -eq 0 ] || die "duplicate windows_path_added field"
      WINDOWS_PATH_ADDED=$value
      SEEN_WINDOWS_PATH_ADDED=1
      ;;
    path_profile)
      flush_profile
      [ -n "$value" ] || die "path_profile cannot be empty"
      CURRENT_PROFILE=$value
      ;;
    path_line)
      [ -n "$CURRENT_PROFILE" ] || die "path_line has no path_profile"
      [ "$CURRENT_HAS_LINE" -eq 0 ] || die "duplicate path_line field"
      CURRENT_LINE=$value
      CURRENT_HAS_LINE=1
      ;;
    path_added)
      [ -n "$CURRENT_PROFILE" ] || die "path_added has no path_profile"
      [ "$CURRENT_HAS_ADDED" -eq 0 ] || die "duplicate path_added field"
      CURRENT_ADDED=$value
      CURRENT_HAS_ADDED=1
      ;;
    path_marker)
      [ -n "$CURRENT_PROFILE" ] || die "path_marker has no path_profile"
      [ "$CURRENT_HAS_MARKER" -eq 0 ] || die "duplicate path_marker field"
      CURRENT_MARKER=$value
      CURRENT_HAS_MARKER=1
      ;;
    *)
      die "unknown receipt field: $key"
      ;;
  esac
done < "$RECEIPT"
flush_profile

[ "$SEEN_SCHEMA" -eq 1 ] && [ "$SCHEMA" = 1 ] || die "unsupported or missing receipt schema"
[ "$SEEN_ROOT" -eq 1 ] || die "receipt is missing install_root"
[ "$SEEN_BIN" -eq 1 ] || die "receipt is missing bin"
[ "$SEEN_BINARY_HASH" -eq 1 ] || die "receipt is missing binary_hash"
validate_hash "$BINARY_HASH"
if [ "$SEEN_SHIM" -eq 1 ] || [ "$SEEN_SHIM_HASH" -eq 1 ]; then
  [ "$SEEN_SHIM" -eq 1 ] && [ "$SEEN_SHIM_HASH" -eq 1 ] \
    || die "shim and shim_hash must be written together"
else
  SHIM=''
  SHIM_HASH=''
fi
if [ -n "$SHIM_HASH" ]; then
  validate_hash "$SHIM_HASH"
  [ -n "$SHIM" ] || die "non-empty shim_hash requires shim"
elif [ -n "$SHIM" ]; then
  die "shim requires a matching shim_hash"
fi

RECEIPT_ROOT=$(normalize_local_path "$RECEIPT_ROOT")
RECEIPT_ROOT_CANON=$(canonical_dir "$RECEIPT_ROOT") \
  || die "receipt install_root is not an existing directory"
[ "$RECEIPT_ROOT_CANON" = "$ROOT" ] \
  || die "receipt install_root does not match --root"

if is_link "$ROOT/bin"; then
  die "install bin directory is a symlink; refusing to use it: $ROOT/bin"
fi
BIN_PARENT_CANON=''
if [ -e "$ROOT/bin" ]; then
  [ -d "$ROOT/bin" ] || die "install bin path is not a directory: $ROOT/bin"
  BIN_PARENT_CANON=$(canonical_dir "$ROOT/bin") \
    || die "could not resolve the install bin directory"
  [ "$BIN_PARENT_CANON" = "$ROOT/bin" ] \
    || die "install bin directory resolves outside the install root"
fi

BIN=$(normalize_local_path "$BIN")
[ "$BIN" = "$ROOT/bin/orchester" ] || [ "$BIN" = "$ROOT/bin/orchester.exe" ] \
  || die "receipt bin is outside the expected Orchester bin path"
if [ -n "$SHIM" ]; then
  SHIM=$(normalize_local_path "$SHIM")
  [ "$(basename "$SHIM")" = "orchester.cmd" ] \
    || die "receipt shim is not an Orchester command shim"
  [ "$SHIM" != "$BIN" ] || die "receipt shim and binary paths collide"
  SHIM_PARENT=$(dirname "$SHIM")
  SHIM_PROBE=$SHIM_PARENT
  while [ ! -e "$SHIM_PROBE" ] && [ "$SHIM_PROBE" != "/" ]; do
    SHIM_NEXT=$(dirname "$SHIM_PROBE")
    [ "$SHIM_NEXT" != "$SHIM_PROBE" ] || break
    SHIM_PROBE=$SHIM_NEXT
  done
  SHIM_PARENT_CANON=$(canonical_dir "$SHIM_PROBE") \
    || die "could not resolve the command shim directory"
  SHIM_ALLOWED=0
  if [ -n "${HOME:-}" ]; then
    HOME_LOCAL=$(normalize_local_path "$HOME")
    HOME_CANON=$(canonical_dir "$HOME_LOCAL") \
      || die "could not resolve the current user's home directory"
    case "$SHIM_PARENT_CANON" in
      "$HOME_CANON"|"$HOME_CANON"/*) SHIM_ALLOWED=1 ;;
    esac
  fi
  if [ "$SHIM_ALLOWED" -eq 0 ] && [ -n "${USERPROFILE:-}" ]; then
    USERPROFILE_LOCAL=$(normalize_local_path "$USERPROFILE")
    USERPROFILE_CANON=$(canonical_dir "$USERPROFILE_LOCAL") \
      || die "could not resolve the current Windows user profile"
    case "$SHIM_PARENT_CANON" in
      "$USERPROFILE_CANON"|"$USERPROFILE_CANON"/*) SHIM_ALLOWED=1 ;;
    esac
  fi
  [ "$SHIM_ALLOWED" -eq 1 ] \
    || die "receipt shim is outside the current user's home directories: $SHIM"
fi

if [ "$SEEN_WINDOWS_PATH_ITEM" -eq 1 ] || [ "$SEEN_WINDOWS_PATH_ADDED" -eq 1 ]; then
  [ "$SEEN_WINDOWS_PATH_ITEM" -eq 1 ] && [ "$SEEN_WINDOWS_PATH_ADDED" -eq 1 ] \
    || die "windows_path_item and windows_path_added must be written together"
  case "$WINDOWS_PATH_ADDED" in
    0|1) ;;
    *) die "windows_path_added must be 0 or 1" ;;
  esac
  if [ "$WINDOWS_PATH_ADDED" = 1 ]; then
    [ -n "$WINDOWS_PATH_ITEM" ] || die "windows_path_item cannot be empty when added"
    command -v cygpath >/dev/null 2>&1 \
      || die "cygpath is required to validate the recorded Windows PATH entry"
    expected_windows_path=$(cygpath -aw "$ROOT/bin" 2>/dev/null | tr -d '\r') \
      || die "could not convert the install bin directory to a Windows path"
    [ "$(normalize_windows_path "$WINDOWS_PATH_ITEM")" = "$(normalize_windows_path "$expected_windows_path")" ] \
      || die "recorded Windows PATH entry does not point at this install root"
  fi
else
  WINDOWS_PATH_ITEM=''
  WINDOWS_PATH_ADDED=0
fi

if [ "$SEEN_CONFIG_DIR" -eq 1 ]; then
  CONFIG_DIR=$(normalize_local_path "$RECEIPT_CONFIG_DIR")
elif [ -n "${ORCHESTER_HOME:-}" ]; then
  CONFIG_DIR=$(normalize_local_path "$ORCHESTER_HOME")
elif [ -n "${HOME:-}" ]; then
  CONFIG_DIR=$(normalize_local_path "$HOME/.orchester")
else
  CONFIG_DIR=''
fi
if [ -n "$CONFIG_DIR" ]; then
  EXPECTED_CONFIG_DIR="${ORCHESTER_HOME:-${HOME:-}/.orchester}"
  EXPECTED_CONFIG_DIR=$(normalize_local_path "$EXPECTED_CONFIG_DIR")
  [ "$CONFIG_DIR" = "$EXPECTED_CONFIG_DIR" ] \
    || die "receipt config_dir is outside the current Orchester home"
fi

# Validate all artifacts before changing PATH or deleting anything.
if [ "$PURGE" = true ]; then
  validate_purge_config "$CONFIG_DIR"
fi
if is_link "$BIN"; then
  die "binary is a symlink; refusing to remove it: $BIN"
fi
if [ -e "$BIN" ]; then
  [ -f "$BIN" ] || die "binary is not a regular file: $BIN"
  [ "$(hash_file "$BIN")" = "$BINARY_HASH" ] \
    || die "binary was modified; refusing to remove it: $BIN"
fi
if [ -n "$SHIM" ]; then
  if is_link "$SHIM"; then
    die "shim is a symlink; refusing to remove it: $SHIM"
  fi
  if [ -e "$SHIM" ]; then
    [ -f "$SHIM" ] || die "shim is not a regular file: $SHIM"
    [ "$(hash_file "$SHIM")" = "$SHIM_HASH" ] \
      || die "shim was modified; refusing to remove it: $SHIM"
  fi
fi

if [ "$NO_PATH_UPDATE" = false ]; then
  EXPECTED_PATH_LINE="export PATH=\"$ROOT/bin:\$PATH\""
  while IFS= read -r record || [ -n "$record" ]; do
    SAVED_IFS=$IFS
    IFS=$TAB
    read -r profile path_line path_added marker extra <<EOF
$record
EOF
    IFS=$SAVED_IFS
    [ -z "$extra" ] || die "internal profile record is malformed"
    [ "$path_added" = 1 ] || continue
    profile_is_supported "$profile" \
      || die "receipt profile is outside the supported user profile set: $profile"
    [ "$path_line" = "$EXPECTED_PATH_LINE" ] \
      || die "receipt PATH line does not match this install root"
    case "$marker" in
      ''|'# Orchester CLI') ;;
      *) die "receipt PATH marker is not recognized" ;;
    esac
    if [ -e "$profile" ]; then
      is_link "$profile" && die "profile is a symlink; refusing to modify it: $profile"
      [ -f "$profile" ] || die "profile is not a regular file: $profile"
    fi
  done < "$PROFILES_FILE"
  if [ "$WINDOWS_PATH_ADDED" = 1 ]; then
    remove_windows_path_item "$WINDOWS_PATH_ITEM"
  fi
  while IFS= read -r record || [ -n "$record" ]; do
    SAVED_IFS=$IFS
    IFS=$TAB
    read -r profile path_line path_added marker extra <<EOF
$record
EOF
    IFS=$SAVED_IFS
    [ -z "$extra" ] || die "internal profile record is malformed"
    [ "$path_added" = 0 ] && continue
    profile_is_supported "$profile" \
      || die "receipt profile is outside the supported user profile set: $profile"
    rewrite_profile "$profile" "$path_line" "$marker"
  done < "$PROFILES_FILE"
else
  info "--no-path-update set; leaving recorded PATH entries unchanged"
fi

remove_owned_file "$BIN" "$BINARY_HASH" "binary" "$BIN_PARENT_CANON"
if [ -n "$SHIM" ]; then
  remove_owned_file "$SHIM" "$SHIM_HASH" "shim" "$SHIM_PARENT_CANON"
fi

if [ "$PURGE" = true ]; then
  [ -n "$CONFIG_DIR" ] || die "cannot determine configuration directory for --purge"
  purge_config "$CONFIG_DIR"
fi

rm -f "$RECEIPT" || die "could not remove install receipt"
rmdir "$ROOT/.orchester" 2>/dev/null || true
info "Orchester installation removed"
