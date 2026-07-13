#!/bin/sh
# Thin bootstrapper for the receipt-aware Orchester uninstaller.
#
# One-line uninstall:
#   curl -fsSL https://raw.githubusercontent.com/dieWehmut/Orchester/main/uninstall.sh | sh

set -eu
umask 077

UNINSTALLER_URL="${ORCHESTER_UNINSTALL_SCRIPT_URL:-https://raw.githubusercontent.com/dieWehmut/Orchester/main/werkzeug/uninstall.sh}"
SCRIPT_DIR=''
TEMP_DIR=''

cleanup() {
  if [ -n "${TEMP_DIR:-}" ] && [ -d "$TEMP_DIR" ]; then
    rm -f -- "$TEMP_DIR/uninstall.sh" 2>/dev/null || true
    rmdir -- "$TEMP_DIR" 2>/dev/null || true
  fi
}
trap cleanup EXIT HUP INT TERM

SCRIPT_NAME=${0##*/}
case "$SCRIPT_NAME" in
  sh|dash|bash|zsh|ksh|ash)
    ;;
  *)
    SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "${0:-}")" 2>/dev/null && pwd || true)
    ;;
esac

if [ -n "$SCRIPT_DIR" ] && [ -f "$SCRIPT_DIR/werkzeug/uninstall.sh" ]; then
  exec sh "$SCRIPT_DIR/werkzeug/uninstall.sh" "$@"
fi

TEMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/orchester-uninstall-bootstrap.XXXXXX") || {
  echo "error: could not create a private temporary directory" >&2
  exit 1
}
DOWNLOADED_UNINSTALLER="$TEMP_DIR/uninstall.sh"

if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$UNINSTALLER_URL" -o "$DOWNLOADED_UNINSTALLER"
elif command -v wget >/dev/null 2>&1; then
  wget -qO "$DOWNLOADED_UNINSTALLER" "$UNINSTALLER_URL"
else
  echo "error: curl or wget is required to fetch $UNINSTALLER_URL" >&2
  exit 1
fi

sh "$DOWNLOADED_UNINSTALLER" "$@"
