#!/usr/bin/env bash
# Regenerate the embedded startup portrait.
#
#   sample/picture/icon.png  ->  kisten/konsole/assets/logo.ansi
#
# The portrait is greyscale half-block ANSI (U+2580 with truecolour fg/bg, so
# each cell carries two vertical pixels). Run this after replacing icon.png:
#   bash werkzeug/logo.sh
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_image="$repo/sample/picture/icon.png"
target="$repo/kisten/konsole/assets/logo.ansi"

# chafa ships via winget on this host; allow an override for other machines.
CHAFA="${CHAFA:-/c/Users/30119/AppData/Local/Microsoft/WinGet/Packages/hpjansson.Chafa_Microsoft.Winget.Source_8wekyb3d8bbwe/chafa-1.18.2-1-x86_64-win/Chafa.exe}"
PYTHON="${PYTHON:-/d/software/python3.10/python}"
NODE="${NODE:-node}"

for tool in "$CHAFA" "$PYTHON" "$NODE"; do
  command -v "$tool" >/dev/null 2>&1 || [ -x "$tool" ] || {
    echo "missing required tool: $tool" >&2
    exit 1
  }
done

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
grey="$scratch/icon-grey.png"
raw="$scratch/logo.txt"

# Drop chroma, keep alpha. Compositing onto an opaque background would make
# chafa paint every cell and lose the spaces the panel renders through.
"$PYTHON" - "$source_image" "$grey" <<'PY'
import sys
from PIL import Image

src = Image.open(sys.argv[1]).convert("RGBA")
Image.merge("LA", (src.convert("L"), src.getchannel("A"))).save(sys.argv[2])
PY

# --size is a maximum: the square source yields 30 rows x 60 columns, which
# avatar.rs centres inside its 66-column canvas.
# --relative/--polite/--passthrough stop chafa emitting cursor controls that a
# baked asset must not contain.
"$CHAFA" \
  --size 66x30 \
  --symbols vhalf \
  --colors full \
  --format symbols \
  --relative=off \
  --polite=on \
  --animate=off \
  --passthrough=none \
  "$grey" >"$raw"

# normalize.mjs owns the asset contract: it strips CRLF, the BOM, cursor
# controls and codepage mojibake, then rejects output that is not exactly
# 30 rows of spaces and U+2580 within 66 columns.
"$NODE" "$repo/werkzeug/logo/normalize.mjs" "$raw" "$target"

echo "verify with: cargo test -p orchester-konsole avatar"
