#!/bin/sh

set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
INSTALLER="$SCRIPT_DIR/../install.sh"
TMP_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/orchester-shim-wrapper-test.XXXXXX")

cleanup() {
  if [ -n "${TMP_ROOT:-}" ] && [ -d "$TMP_ROOT" ]; then
    rm -rf -- "$TMP_ROOT"
  fi
}
trap cleanup EXIT HUP INT TERM

fail() {
  printf '%s\n' "install shim wrapper test: $*" >&2
  exit 1
}

mkdir -p "$TMP_ROOT/bin"
FAKE_POWERSHELL="$TMP_ROOT/bin/powershell.exe"
{
  printf '%s\n' '#!/bin/sh'
  printf '%s\n' ': "${FAKE_SHIM_PATH:?}"'
  printf '%s\n' 'printf "%s\r\n" "$FAKE_SHIM_PATH"'
} > "$FAKE_POWERSHELL"
chmod +x "$FAKE_POWERSHELL"

HARNESS="$TMP_ROOT/harness.sh"
{
  printf '%s\n' '#!/bin/sh'
  printf '%s\n' 'set -eu'
  printf '%s\n' 'C_GREEN=""'
  printf '%s\n' 'C_RESET=""'
  printf '%s\n' 'have_cmd() { command -v "$1" >/dev/null 2>&1; }'
  printf '%s\n' 'to_windows_path() { printf "%s\\n" "$1"; }'
  printf '%s\n' 'ok() { printf "[+] %s\\n" "$*"; }'
  sed -n '/^# ORCHESTER_WINDOWS_SHIM_FUNCTION_BEGIN$/,/^# ORCHESTER_WINDOWS_SHIM_FUNCTION_END$/p' "$INSTALLER"
  printf '%s\n' 'result=$(ensure_windows_command_shim "/tmp/orchester.exe")'
  printf '%s\n' 'printf "RESULT=%s\\n" "$result"'
} > "$HARNESS"

SHIM='C:\Users\tester\bin\orchester.cmd'
LOG="$TMP_ROOT/log"
OUTPUT=$(
  PATH="$TMP_ROOT/bin:$PATH" FAKE_SHIM_PATH="$SHIM" sh "$HARNESS" 2> "$LOG"
)

[ "$OUTPUT" = "RESULT=$SHIM" ] || fail "stdout was not the plain shim path: $OUTPUT"
LOG_LINE=$(sed -n '1p' "$LOG")
[ "$LOG_LINE" = "[+] Added Windows command shim: $SHIM" ] \
  || fail "status log was not isolated on stderr: $LOG_LINE"

printf '%s\n' 'install shim wrapper tests passed'
