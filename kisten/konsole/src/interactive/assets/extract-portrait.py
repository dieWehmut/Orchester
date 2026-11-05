"""Regenerate the portrait matte; development-only, no runtime dependencies.

Use an isolated environment with rembg[cpu]==2.0.86 and onnxruntime==1.31.1.
Model: https://github.com/danielgatis/rembg/releases/download/v0.0.0/isnet-anime.onnx
Model SHA256: f15622d853e8260172812b657053460e20806f04b9e05147d49af7bed31a6e99
The semantic model predicts alpha only. Original image RGB is never modified.
"""
import os
from pathlib import Path

os.environ.setdefault("OMP_NUM_THREADS", "1")
from PIL import Image
from rembg import new_session, remove

assets = Path(__file__).resolve().parent
repo = assets.parents[4]
source = Image.open(repo / "apps/web/src/assets/orchester-mark.png").convert("RGB")
mask = remove(source, session=new_session("isnet-anime"), only_mask=True).convert("L")
mask.save(assets / "orchester-alpha.png", optimize=True)
(assets / "orchester.alpha").write_bytes(
    mask.resize((256, 256), Image.Resampling.LANCZOS).tobytes()
)
