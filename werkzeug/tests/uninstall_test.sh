#!/bin/sh

set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
UNINSTALLER="$SCRIPT_DIR/../uninstall.sh"
BOOTSTRAP="$SCRIPT_DIR/../../uninstall.sh"
UNINSTALL_SHELL=${UNINSTALL_SHELL:-sh}

run_uninstaller() {
  "$UNINSTALL_SHELL" "$UNINSTALLER" "$@"
}

fail() {
  printf '%s\n' "uninstall test: $*" >&2
  exit 1
}

assert_file() {
  [ -f "$1" ] || fail "expected file: $1"
}

assert_missing() {
  [ ! -e "$1" ] || fail "expected path to be absent: $1"
}

assert_contains() {
  grep -F -- "$2" "$1" >/dev/null 2>&1 || fail "expected '$2' in $1"
}

assert_not_contains() {
  if grep -F -- "$2" "$1" >/dev/null 2>&1; then
    fail "did not expect '$2' in $1"
  fi
}

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | sed 's/[[:space:]].*$//'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | sed 's/[[:space:]].*$//'
  else
    fail "sha256sum or shasum is required"
  fi
}

TMP_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/orchester-uninstall-test.XXXXXX")
SYMLINK_TESTS_RUN=0
cleanup() {
  if [ -n "${TMP_ROOT:-}" ] && [ -d "$TMP_ROOT" ]; then
    rm -rf -- "$TMP_ROOT"
  fi
}
trap cleanup EXIT HUP INT TERM

ROOT="$TMP_ROOT/install"
BIN_DIR="$ROOT/bin"
METADATA_DIR="$ROOT/.orchester"
HOME="$TMP_ROOT/home"
export HOME
CONFIG_DIR="$HOME/.orchester"
SHIM_DIR="$HOME/shims"
PROFILE="$HOME/.profile"
OTHER_BIN="$BIN_DIR/other-tool"
BIN="$BIN_DIR/orchester"
SHIM="$SHIM_DIR/orchester.cmd"
RECEIPT="$METADATA_DIR/install.receipt"

mkdir -p "$BIN_DIR" "$METADATA_DIR" "$CONFIG_DIR" "$SHIM_DIR" "$HOME"
printf '%s\n' 'orchester test binary' > "$BIN"
printf '%s\n' 'other cargo binary' > "$OTHER_BIN"
printf '%s\n' 'orchester shim' > "$SHIM"
printf '%s\n' '{"model":"test"}' > "$CONFIG_DIR/orchester.jsonc"
printf '%s\n' 'export PATH="/keep/bin:$PATH"' > "$PROFILE"
printf '%s\n' '' >> "$PROFILE"
printf '%s\n' '# Orchester CLI' >> "$PROFILE"
printf '%s\n' "export PATH=\"$BIN_DIR:\$PATH\"" >> "$PROFILE"
printf '%s\n' 'export PATH="/keep/after:$PATH"' >> "$PROFILE"

BIN_HASH=$(sha256 "$BIN")
SHIM_HASH=$(sha256 "$SHIM")
PATH_LINE="export PATH=\"$BIN_DIR:\$PATH\""

{
  printf 'schema\t1\n'
  printf 'install_root\t%s\n' "$ROOT"
  printf 'bin\t%s\n' "$BIN"
  printf 'binary_hash\t%s\n' "$BIN_HASH"
  printf 'shim\t%s\n' "$SHIM"
  printf 'shim_hash\t%s\n' "$SHIM_HASH"
  printf 'path_profile\t%s\n' "$PROFILE"
  printf 'path_added\t1\n'
  printf 'path_line\t%s\n' "$PATH_LINE"
  printf 'path_marker\t# Orchester CLI\n'
  printf 'version\t0.1.0\n'
  printf 'config_dir\t%s\n' "$CONFIG_DIR"
} > "$RECEIPT"

if run_uninstaller --root "$ROOT"; then
  :
else
  fail 'valid receipt was rejected'
fi

assert_missing "$BIN"
assert_file "$OTHER_BIN"
assert_missing "$SHIM"
assert_missing "$RECEIPT"
assert_contains "$PROFILE" 'export PATH="/keep/bin:$PATH"'
assert_contains "$PROFILE" 'export PATH="/keep/after:$PATH"'
assert_not_contains "$PROFILE" "$PATH_LINE"
assert_not_contains "$PROFILE" '# Orchester CLI'
assert_file "$CONFIG_DIR/orchester.jsonc"

# A second invocation is a successful no-op after a complete uninstall.
run_uninstaller --root "$ROOT" --no-path-update

# Purge removes only the user config owned by the receipt and remains explicit.
mkdir -p "$ROOT/bin" "$ROOT/.orchester" "$CONFIG_DIR"
printf '%s\n' 'orchester test binary' > "$BIN"
BIN_HASH=$(sha256 "$BIN")
{
  printf 'schema\t1\n'
  printf 'install_root\t%s\n' "$ROOT"
  printf 'bin\t%s\n' "$BIN"
  printf 'binary_hash\t%s\n' "$BIN_HASH"
  printf 'shim\t\n'
  printf 'shim_hash\t\n'
  printf 'version\t0.1.0\n'
  printf 'config_dir\t%s\n' "$CONFIG_DIR"
} > "$RECEIPT"
run_uninstaller --root "$ROOT" --purge --no-path-update
assert_missing "$BIN"
assert_missing "$RECEIPT"
assert_missing "$CONFIG_DIR/orchester.jsonc"

# A changed binary must be refused and left untouched.
mkdir -p "$ROOT/bin" "$ROOT/.orchester"
printf '%s\n' 'original' > "$BIN"
BIN_HASH=$(sha256 "$BIN")
{
  printf 'schema\t1\n'
  printf 'install_root\t%s\n' "$ROOT"
  printf 'bin\t%s\n' "$BIN"
  printf 'binary_hash\t%s\n' "$BIN_HASH"
  printf 'shim\t\n'
  printf 'shim_hash\t\n'
  printf 'version\t0.1.0\n'
} > "$RECEIPT"
printf '%s\n' 'user modification' > "$BIN"
if run_uninstaller --root "$ROOT" --no-path-update; then
  fail 'modified binary was removed'
fi
assert_file "$BIN"
assert_file "$RECEIPT"

# A receipt cannot authorize a different root.
OTHER_ROOT="$TMP_ROOT/other"
mkdir -p "$OTHER_ROOT/.orchester"
cp "$RECEIPT" "$OTHER_ROOT/.orchester/install.receipt"
if run_uninstaller --root "$OTHER_ROOT" --no-path-update; then
  fail 'root mismatch was accepted'
fi
assert_file "$BIN"
assert_file "$RECEIPT"

# A foreign binary without a receipt is never guessed or deleted.
FOREIGN_ROOT="$TMP_ROOT/foreign"
FOREIGN_BIN="$FOREIGN_ROOT/bin/orchester"
mkdir -p "$FOREIGN_ROOT/bin"
printf '%s\n' 'foreign binary' > "$FOREIGN_BIN"
if run_uninstaller --root "$FOREIGN_ROOT" --no-path-update; then
  fail 'foreign binary without receipt was accepted'
fi
assert_file "$FOREIGN_BIN"

# A changed shim blocks the whole uninstall before the binary is removed.
SHIM_TEST_ROOT="$TMP_ROOT/modified-shim"
SHIM_TEST_BIN="$SHIM_TEST_ROOT/bin/orchester"
SHIM_TEST_RECEIPT="$SHIM_TEST_ROOT/.orchester/install.receipt"
SHIM_TEST_FILE="$HOME/modified-shim-dir/orchester.cmd"
mkdir -p "$SHIM_TEST_ROOT/bin" "$SHIM_TEST_ROOT/.orchester" "$(dirname "$SHIM_TEST_FILE")"
printf '%s\n' 'owned binary' > "$SHIM_TEST_BIN"
printf '%s\n' 'owned shim' > "$SHIM_TEST_FILE"
SHIM_TEST_BIN_HASH=$(sha256 "$SHIM_TEST_BIN")
SHIM_TEST_HASH=$(sha256 "$SHIM_TEST_FILE")
{
  printf 'schema\t1\n'
  printf 'install_root\t%s\n' "$SHIM_TEST_ROOT"
  printf 'bin\t%s\n' "$SHIM_TEST_BIN"
  printf 'binary_hash\t%s\n' "$SHIM_TEST_BIN_HASH"
  printf 'shim\t%s\n' "$SHIM_TEST_FILE"
  printf 'shim_hash\t%s\n' "$SHIM_TEST_HASH"
} > "$SHIM_TEST_RECEIPT"
printf '%s\n' 'modified shim' > "$SHIM_TEST_FILE"
if run_uninstaller --root "$SHIM_TEST_ROOT" --no-path-update; then
  fail 'modified shim was accepted'
fi
assert_file "$SHIM_TEST_BIN"
assert_file "$SHIM_TEST_FILE"
assert_file "$SHIM_TEST_RECEIPT"

# A PowerShell-created shim may live under USERPROFILE while MSYS uses a
# separate HOME directory.
PROFILE_SHIM_ROOT="$TMP_ROOT/profile-shim"
PROFILE_SHIM_USERPROFILE="$TMP_ROOT/windows-user-profile"
PROFILE_SHIM_BIN="$PROFILE_SHIM_ROOT/bin/orchester"
PROFILE_SHIM_FILE="$PROFILE_SHIM_USERPROFILE/bin/orchester.cmd"
PROFILE_SHIM_RECEIPT="$PROFILE_SHIM_ROOT/.orchester/install.receipt"
mkdir -p "$PROFILE_SHIM_ROOT/bin" "$PROFILE_SHIM_ROOT/.orchester" "$(dirname "$PROFILE_SHIM_FILE")"
printf '%s\n' 'owned binary' > "$PROFILE_SHIM_BIN"
printf '%s\n' 'owned Windows shim' > "$PROFILE_SHIM_FILE"
PROFILE_SHIM_BIN_HASH=$(sha256 "$PROFILE_SHIM_BIN")
PROFILE_SHIM_HASH=$(sha256 "$PROFILE_SHIM_FILE")
{
  printf 'schema\t1\n'
  printf 'install_root\t%s\n' "$PROFILE_SHIM_ROOT"
  printf 'bin\t%s\n' "$PROFILE_SHIM_BIN"
  printf 'binary_hash\t%s\n' "$PROFILE_SHIM_BIN_HASH"
  printf 'shim\t%s\n' "$PROFILE_SHIM_FILE"
  printf 'shim_hash\t%s\n' "$PROFILE_SHIM_HASH"
} > "$PROFILE_SHIM_RECEIPT"
USERPROFILE="$PROFILE_SHIM_USERPROFILE" "$UNINSTALL_SHELL" "$UNINSTALLER" \
  --root "$PROFILE_SHIM_ROOT" --no-path-update
assert_missing "$PROFILE_SHIM_BIN"
assert_missing "$PROFILE_SHIM_FILE"
assert_missing "$PROFILE_SHIM_RECEIPT"

# --no-path-update leaves both Unix and Windows PATH records untouched.
NO_PATH_ROOT="$TMP_ROOT/no-path"
NO_PATH_BIN="$NO_PATH_ROOT/bin/orchester"
NO_PATH_RECEIPT="$NO_PATH_ROOT/.orchester/install.receipt"
NO_PATH_PROFILE="$HOME/.bashrc"
NO_PATH_LINE="export PATH=\"$NO_PATH_ROOT/bin:\$PATH\""
mkdir -p "$NO_PATH_ROOT/bin" "$NO_PATH_ROOT/.orchester"
printf '%s\n' 'owned binary' > "$NO_PATH_BIN"
printf '%s\n' '# Orchester CLI' > "$NO_PATH_PROFILE"
printf '%s\n' "$NO_PATH_LINE" >> "$NO_PATH_PROFILE"
NO_PATH_HASH=$(sha256 "$NO_PATH_BIN")
NO_PATH_WINDOWS_ITEM=''
if command -v cygpath >/dev/null 2>&1; then
  NO_PATH_WINDOWS_ITEM=$(cygpath -aw "$NO_PATH_ROOT/bin")
fi
{
  printf 'schema\t1\n'
  printf 'install_root\t%s\n' "$NO_PATH_ROOT"
  printf 'bin\t%s\n' "$NO_PATH_BIN"
  printf 'binary_hash\t%s\n' "$NO_PATH_HASH"
  printf 'shim\t\n'
  printf 'shim_hash\t\n'
  printf 'path_profile\t%s\n' "$NO_PATH_PROFILE"
  printf 'path_added\t1\n'
  printf 'path_line\t%s\n' "$NO_PATH_LINE"
  printf 'path_marker\t# Orchester CLI\n'
} > "$NO_PATH_RECEIPT"
if [ -n "$NO_PATH_WINDOWS_ITEM" ]; then
  {
    printf 'windows_path_item\t%s\n' "$NO_PATH_WINDOWS_ITEM"
    printf 'windows_path_added\t1\n'
  } >> "$NO_PATH_RECEIPT"
fi
run_uninstaller --root "$NO_PATH_ROOT" --no-path-update
assert_missing "$NO_PATH_BIN"
assert_missing "$NO_PATH_RECEIPT"
assert_contains "$NO_PATH_PROFILE" "$NO_PATH_LINE"
assert_contains "$NO_PATH_PROFILE" '# Orchester CLI'

# PowerShell receipts use CRLF and must remain compatible with the POSIX parser.
CRLF_ROOT="$TMP_ROOT/crlf"
CRLF_BIN="$CRLF_ROOT/bin/orchester"
CRLF_RECEIPT="$CRLF_ROOT/.orchester/install.receipt"
mkdir -p "$CRLF_ROOT/bin" "$CRLF_ROOT/.orchester"
printf '%s\n' 'owned binary' > "$CRLF_BIN"
CRLF_HASH=$(sha256 "$CRLF_BIN")
{
  printf 'schema\t1\r\n'
  printf 'install_root\t%s\r\n' "$CRLF_ROOT"
  printf 'bin\t%s\r\n' "$CRLF_BIN"
  printf 'binary_hash\t%s\r\n' "$CRLF_HASH"
  printf 'shim\t\r\n'
  printf 'shim_hash\t\r\n'
} > "$CRLF_RECEIPT"
run_uninstaller --root "$CRLF_ROOT" --no-path-update
assert_missing "$CRLF_BIN"
assert_missing "$CRLF_RECEIPT"

# A PowerShell receipt stores native Windows paths; MSYS must normalize them.
if command -v cygpath >/dev/null 2>&1; then
  WINDOWS_ROOT="$TMP_ROOT/windows-receipt"
  WINDOWS_BIN="$WINDOWS_ROOT/bin/orchester.exe"
  WINDOWS_RECEIPT="$WINDOWS_ROOT/.orchester/install.receipt"
  mkdir -p "$WINDOWS_ROOT/bin" "$WINDOWS_ROOT/.orchester"
  printf '%s\n' 'owned Windows binary' > "$WINDOWS_BIN"
  WINDOWS_HASH=$(sha256 "$WINDOWS_BIN")
  WINDOWS_ROOT_NATIVE=$(cygpath -aw "$WINDOWS_ROOT")
  WINDOWS_BIN_NATIVE=$(cygpath -aw "$WINDOWS_BIN")
  {
    printf 'schema\t1\r\n'
    printf 'install_root\t%s\r\n' "$WINDOWS_ROOT_NATIVE"
    printf 'bin\t%s\r\n' "$WINDOWS_BIN_NATIVE"
    printf 'binary_hash\t%s\r\n' "$WINDOWS_HASH"
    printf 'shim\t\r\n'
    printf 'shim_hash\t\r\n'
  } > "$WINDOWS_RECEIPT"
  run_uninstaller --root "$WINDOWS_ROOT" --no-path-update
  assert_missing "$WINDOWS_BIN"
  assert_missing "$WINDOWS_RECEIPT"
fi

# Unknown fields make the receipt untrusted and preserve the installation.
UNKNOWN_ROOT="$TMP_ROOT/unknown-field"
UNKNOWN_BIN="$UNKNOWN_ROOT/bin/orchester"
UNKNOWN_RECEIPT="$UNKNOWN_ROOT/.orchester/install.receipt"
mkdir -p "$UNKNOWN_ROOT/bin" "$UNKNOWN_ROOT/.orchester"
printf '%s\n' 'owned binary' > "$UNKNOWN_BIN"
UNKNOWN_HASH=$(sha256 "$UNKNOWN_BIN")
{
  printf 'schema\t1\n'
  printf 'install_root\t%s\n' "$UNKNOWN_ROOT"
  printf 'bin\t%s\n' "$UNKNOWN_BIN"
  printf 'binary_hash\t%s\n' "$UNKNOWN_HASH"
  printf 'shim\t\n'
  printf 'shim_hash\t\n'
  printf 'unexpected\tvalue\n'
} > "$UNKNOWN_RECEIPT"
if run_uninstaller --root "$UNKNOWN_ROOT" --no-path-update; then
  fail 'unknown receipt field was accepted'
fi
assert_file "$UNKNOWN_BIN"
assert_file "$UNKNOWN_RECEIPT"

# A receipt cannot use PATH cleanup to modify an arbitrary file.
UNSAFE_ROOT="$TMP_ROOT/unsafe-profile"
UNSAFE_BIN="$UNSAFE_ROOT/bin/orchester"
UNSAFE_RECEIPT="$UNSAFE_ROOT/.orchester/install.receipt"
UNSAFE_PROFILE="$TMP_ROOT/not-a-user-profile"
UNSAFE_LINE="export PATH=\"$UNSAFE_ROOT/bin:\$PATH\""
mkdir -p "$UNSAFE_ROOT/bin" "$UNSAFE_ROOT/.orchester"
printf '%s\n' 'owned binary' > "$UNSAFE_BIN"
printf '%s\n' "$UNSAFE_LINE" > "$UNSAFE_PROFILE"
UNSAFE_HASH=$(sha256 "$UNSAFE_BIN")
{
  printf 'schema\t1\n'
  printf 'install_root\t%s\n' "$UNSAFE_ROOT"
  printf 'bin\t%s\n' "$UNSAFE_BIN"
  printf 'binary_hash\t%s\n' "$UNSAFE_HASH"
  printf 'shim\t\n'
  printf 'shim_hash\t\n'
  printf 'path_profile\t%s\n' "$UNSAFE_PROFILE"
  printf 'path_added\t1\n'
  printf 'path_line\t%s\n' "$UNSAFE_LINE"
} > "$UNSAFE_RECEIPT"
if run_uninstaller --root "$UNSAFE_ROOT"; then
  fail 'unsafe profile path was accepted'
fi
assert_file "$UNSAFE_BIN"
assert_file "$UNSAFE_RECEIPT"
assert_contains "$UNSAFE_PROFILE" "$UNSAFE_LINE"

# Even a supported profile cannot authorize removal of an arbitrary line.
UNSAFE_LINE_ROOT="$TMP_ROOT/unsafe-line"
UNSAFE_LINE_BIN="$UNSAFE_LINE_ROOT/bin/orchester"
UNSAFE_LINE_RECEIPT="$UNSAFE_LINE_ROOT/.orchester/install.receipt"
UNSAFE_LINE_PROFILE="$HOME/.zshrc"
UNSAFE_OTHER_LINE='export KEEP_THIS_VALUE=1'
mkdir -p "$UNSAFE_LINE_ROOT/bin" "$UNSAFE_LINE_ROOT/.orchester"
printf '%s\n' 'owned binary' > "$UNSAFE_LINE_BIN"
printf '%s\n' "$UNSAFE_OTHER_LINE" > "$UNSAFE_LINE_PROFILE"
UNSAFE_LINE_HASH=$(sha256 "$UNSAFE_LINE_BIN")
{
  printf 'schema\t1\n'
  printf 'install_root\t%s\n' "$UNSAFE_LINE_ROOT"
  printf 'bin\t%s\n' "$UNSAFE_LINE_BIN"
  printf 'binary_hash\t%s\n' "$UNSAFE_LINE_HASH"
  printf 'shim\t\n'
  printf 'shim_hash\t\n'
  printf 'path_profile\t%s\n' "$UNSAFE_LINE_PROFILE"
  printf 'path_added\t1\n'
  printf 'path_line\t%s\n' "$UNSAFE_OTHER_LINE"
} > "$UNSAFE_LINE_RECEIPT"
if run_uninstaller --root "$UNSAFE_LINE_ROOT"; then
  fail 'arbitrary profile line was accepted'
fi
assert_file "$UNSAFE_LINE_BIN"
assert_file "$UNSAFE_LINE_RECEIPT"
assert_contains "$UNSAFE_LINE_PROFILE" "$UNSAFE_OTHER_LINE"

# A symlinked bin directory must not turn the receipt into authority over a
# file outside the install root.
LINK_ROOT="$TMP_ROOT/symlink-bin"
LINK_TARGET="$TMP_ROOT/symlink-bin-target"
LINK_RECEIPT="$LINK_ROOT/.orchester/install.receipt"
LINK_TARGET_BIN="$LINK_TARGET/orchester"
mkdir -p "$LINK_ROOT/.orchester" "$LINK_TARGET"
if ln -s "$LINK_TARGET" "$LINK_ROOT/bin" 2>/dev/null && [ -L "$LINK_ROOT/bin" ]; then
  SYMLINK_TESTS_RUN=$((SYMLINK_TESTS_RUN + 1))
  printf '%s\n' 'outside binary' > "$LINK_TARGET_BIN"
  LINK_HASH=$(sha256 "$LINK_TARGET_BIN")
  {
    printf 'schema\t1\n'
    printf 'install_root\t%s\n' "$LINK_ROOT"
    printf 'bin\t%s\n' "$LINK_ROOT/bin/orchester"
    printf 'binary_hash\t%s\n' "$LINK_HASH"
    printf 'shim\t\n'
    printf 'shim_hash\t\n'
  } > "$LINK_RECEIPT"
  if run_uninstaller --root "$LINK_ROOT" --no-path-update; then
    fail 'symlinked bin directory was accepted'
  fi
  assert_file "$LINK_TARGET_BIN"
  assert_file "$LINK_RECEIPT"
else
  printf '%s\n' 'uninstall test: SKIP symlinked bin directory (host denied symlink creation)'
fi

# A shim's lexical HOME prefix is not enough when an intermediate directory
# redirects outside HOME.
SHIM_LINK_ROOT="$TMP_ROOT/symlink-shim"
SHIM_LINK_TARGET="$TMP_ROOT/symlink-shim-target"
SHIM_LINK_DIR="$HOME/symlink-shim-dir"
SHIM_LINK_BIN="$SHIM_LINK_ROOT/bin/orchester"
SHIM_LINK_RECEIPT="$SHIM_LINK_ROOT/.orchester/install.receipt"
SHIM_LINK_FILE="$SHIM_LINK_DIR/orchester.cmd"
mkdir -p "$SHIM_LINK_ROOT/bin" "$SHIM_LINK_ROOT/.orchester" "$SHIM_LINK_TARGET"
if ln -s "$SHIM_LINK_TARGET" "$SHIM_LINK_DIR" 2>/dev/null && [ -L "$SHIM_LINK_DIR" ]; then
  SYMLINK_TESTS_RUN=$((SYMLINK_TESTS_RUN + 1))
  printf '%s\n' 'owned binary' > "$SHIM_LINK_BIN"
  printf '%s\n' 'outside shim' > "$SHIM_LINK_TARGET/orchester.cmd"
  SHIM_LINK_BIN_HASH=$(sha256 "$SHIM_LINK_BIN")
  SHIM_LINK_HASH=$(sha256 "$SHIM_LINK_FILE")
  {
    printf 'schema\t1\n'
    printf 'install_root\t%s\n' "$SHIM_LINK_ROOT"
    printf 'bin\t%s\n' "$SHIM_LINK_BIN"
    printf 'binary_hash\t%s\n' "$SHIM_LINK_BIN_HASH"
    printf 'shim\t%s\n' "$SHIM_LINK_FILE"
    printf 'shim_hash\t%s\n' "$SHIM_LINK_HASH"
  } > "$SHIM_LINK_RECEIPT"
  if run_uninstaller --root "$SHIM_LINK_ROOT" --no-path-update; then
    fail 'symlinked shim parent was accepted'
  fi
  assert_file "$SHIM_LINK_BIN"
  assert_file "$SHIM_LINK_RECEIPT"
  assert_file "$SHIM_LINK_TARGET/orchester.cmd"
else
  printf '%s\n' 'uninstall test: SKIP symlinked shim directory (host denied symlink creation)'
fi

# Purge validation must happen before any binary, shim, or receipt mutation.
PURGE_LINK_ROOT="$TMP_ROOT/purge-symlink"
PURGE_LINK_BIN="$PURGE_LINK_ROOT/bin/orchester"
PURGE_LINK_RECEIPT="$PURGE_LINK_ROOT/.orchester/install.receipt"
PURGE_LINK_CONFIG="$HOME/purge-config"
PURGE_LINK_TARGET="$TMP_ROOT/purge-config-target.jsonc"
mkdir -p "$PURGE_LINK_ROOT/bin" "$PURGE_LINK_ROOT/.orchester" "$PURGE_LINK_CONFIG"
printf '%s\n' 'owned binary' > "$PURGE_LINK_BIN"
printf '%s\n' 'outside config' > "$PURGE_LINK_TARGET"
if ln -s "$PURGE_LINK_TARGET" "$PURGE_LINK_CONFIG/orchester.jsonc" 2>/dev/null && [ -L "$PURGE_LINK_CONFIG/orchester.jsonc" ]; then
  SYMLINK_TESTS_RUN=$((SYMLINK_TESTS_RUN + 1))
  PURGE_LINK_HASH=$(sha256 "$PURGE_LINK_BIN")
  {
    printf 'schema\t1\n'
    printf 'install_root\t%s\n' "$PURGE_LINK_ROOT"
    printf 'bin\t%s\n' "$PURGE_LINK_BIN"
    printf 'binary_hash\t%s\n' "$PURGE_LINK_HASH"
    printf 'shim\t\n'
    printf 'shim_hash\t\n'
    printf 'config_dir\t%s\n' "$PURGE_LINK_CONFIG"
  } > "$PURGE_LINK_RECEIPT"
  if ORCHESTER_HOME="$PURGE_LINK_CONFIG" "$UNINSTALL_SHELL" "$UNINSTALLER" \
    --root "$PURGE_LINK_ROOT" --purge --no-path-update; then
    fail 'symlinked purge file was accepted'
  fi
  assert_file "$PURGE_LINK_BIN"
  assert_file "$PURGE_LINK_RECEIPT"
  assert_file "$PURGE_LINK_CONFIG/orchester.jsonc"
  assert_file "$PURGE_LINK_TARGET"
else
  printf '%s\n' 'uninstall test: SKIP symlinked purge file (host denied symlink creation)'
fi

# The repository-level bootstrap delegates to the receipt-aware uninstaller
# and preserves arguments without requiring a network connection.
BOOTSTRAP_ROOT="$TMP_ROOT/bootstrap"
BOOTSTRAP_BIN="$BOOTSTRAP_ROOT/bin/orchester"
BOOTSTRAP_RECEIPT="$BOOTSTRAP_ROOT/.orchester/install.receipt"
mkdir -p "$BOOTSTRAP_ROOT/bin" "$BOOTSTRAP_ROOT/.orchester"
printf '%s\n' 'bootstrap binary' > "$BOOTSTRAP_BIN"
BOOTSTRAP_HASH=$(sha256 "$BOOTSTRAP_BIN")
{
  printf 'schema\t1\n'
  printf 'install_root\t%s\n' "$BOOTSTRAP_ROOT"
  printf 'bin\t%s\n' "$BOOTSTRAP_BIN"
  printf 'binary_hash\t%s\n' "$BOOTSTRAP_HASH"
  printf 'shim\t\n'
  printf 'shim_hash\t\n'
} > "$BOOTSTRAP_RECEIPT"
ORCHESTER_UNINSTALL_SCRIPT_URL='http://127.0.0.1:1/must-not-download' \
  "$UNINSTALL_SHELL" "$BOOTSTRAP" --root "$BOOTSTRAP_ROOT" --no-path-update
assert_missing "$BOOTSTRAP_BIN"
assert_missing "$BOOTSTRAP_RECEIPT"

# Calling the bootstrap by basename from its directory must still select the
# repository-local implementation rather than falling back to the network.
BOOTSTRAP_BASENAME_ROOT="$TMP_ROOT/bootstrap-basename"
BOOTSTRAP_BASENAME_BIN="$BOOTSTRAP_BASENAME_ROOT/bin/orchester"
BOOTSTRAP_BASENAME_RECEIPT="$BOOTSTRAP_BASENAME_ROOT/.orchester/install.receipt"
mkdir -p "$BOOTSTRAP_BASENAME_ROOT/bin" "$BOOTSTRAP_BASENAME_ROOT/.orchester"
printf '%s\n' 'basename bootstrap binary' > "$BOOTSTRAP_BASENAME_BIN"
BOOTSTRAP_BASENAME_HASH=$(sha256 "$BOOTSTRAP_BASENAME_BIN")
{
  printf 'schema\t1\n'
  printf 'install_root\t%s\n' "$BOOTSTRAP_BASENAME_ROOT"
  printf 'bin\t%s\n' "$BOOTSTRAP_BASENAME_BIN"
  printf 'binary_hash\t%s\n' "$BOOTSTRAP_BASENAME_HASH"
  printf 'shim\t\n'
  printf 'shim_hash\t\n'
} > "$BOOTSTRAP_BASENAME_RECEIPT"
(
  cd "$SCRIPT_DIR/../.."
  ORCHESTER_UNINSTALL_SCRIPT_URL='http://127.0.0.1:1/must-not-download' \
    "$UNINSTALL_SHELL" uninstall.sh --root "$BOOTSTRAP_BASENAME_ROOT" --no-path-update
)
assert_missing "$BOOTSTRAP_BASENAME_BIN"
assert_missing "$BOOTSTRAP_BASENAME_RECEIPT"

if [ "${ORCHESTER_REQUIRE_SYMLINK_TESTS:-false}" = true ] && [ "$SYMLINK_TESTS_RUN" -ne 3 ]; then
  fail "required 3 symlink tests, ran $SYMLINK_TESTS_RUN"
fi

printf '%s\n' 'uninstall tests passed'
