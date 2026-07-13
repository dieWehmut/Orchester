#!/bin/sh

set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)
TMP_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/orchester-install-bootstrap-test.XXXXXX")

cleanup() {
  if [ -n "${TMP_ROOT:-}" ] && [ -d "$TMP_ROOT" ]; then
    rm -rf -- "$TMP_ROOT"
  fi
}
trap cleanup EXIT HUP INT TERM

fail() {
  printf '%s\n' "install bootstrap test: $*" >&2
  exit 1
}

mkdir -p "$TMP_ROOT/werkzeug"
cp "$REPO_ROOT/install.sh" "$TMP_ROOT/install.sh"

FAKE_INSTALLER="$TMP_ROOT/werkzeug/install.sh"
{
  printf '%s\n' '#!/bin/sh'
  printf '%s\n' 'set -eu'
  printf '%s\n' ': "${BOOTSTRAP_TEST_OUTPUT:?}"'
  printf '%s\n' 'printf "%s\\n" "$@" > "$BOOTSTRAP_TEST_OUTPUT"'
} > "$FAKE_INSTALLER"

OUTPUT="$TMP_ROOT/arguments"
(
  cd "$TMP_ROOT"
  BOOTSTRAP_TEST_OUTPUT="$OUTPUT" \
  ORCHESTER_INSTALL_SCRIPT_URL='http://127.0.0.1:1/must-not-download' \
    sh install.sh --root "$TMP_ROOT/install root" --no-path-update
)

[ -f "$OUTPUT" ] || fail 'repository-local installer was not invoked'
{
  IFS= read -r first || fail 'missing first bootstrap argument'
  IFS= read -r second || fail 'missing second bootstrap argument'
  IFS= read -r third || fail 'missing third bootstrap argument'
  [ "$first" = '--root' ] || fail 'bootstrap changed --root'
  [ "$second" = "$TMP_ROOT/install root" ] || fail 'bootstrap changed the root value'
  [ "$third" = '--no-path-update' ] || fail 'bootstrap changed --no-path-update'
  if IFS= read -r extra; then
    fail "bootstrap added an argument: $extra"
  fi
} < "$OUTPUT"

printf '%s\n' 'install bootstrap tests passed'
