#!/bin/sh
# Thin bootstrapper for the Orchester installer.
#
# One-line install:
#   curl -fsSL https://raw.githubusercontent.com/dieWehmut/Orchester/main/install.sh | sh

set -eu

INSTALLER_URL="${ORCHESTER_INSTALL_SCRIPT_URL:-https://raw.githubusercontent.com/dieWehmut/Orchester/main/werkzeug/install.sh}"

SCRIPT_DIR=""
SCRIPT_NAME=${0##*/}
case "$SCRIPT_NAME" in
  sh|dash|bash|zsh|ksh|ash)
    ;;
  *)
    SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "${0:-}")" 2>/dev/null && pwd || true)"
    ;;
esac

if [ -n "$SCRIPT_DIR" ] && [ -f "$SCRIPT_DIR/werkzeug/install.sh" ]; then
  exec sh "$SCRIPT_DIR/werkzeug/install.sh" "$@"
fi

if command -v curl >/dev/null 2>&1; then
  exec sh -s -- "$@" <<EOF
$(curl -fsSL "$INSTALLER_URL")
EOF
elif command -v wget >/dev/null 2>&1; then
  exec sh -s -- "$@" <<EOF
$(wget -qO- "$INSTALLER_URL")
EOF
else
  echo "error: curl or wget is required to fetch $INSTALLER_URL" >&2
  exit 1
fi
