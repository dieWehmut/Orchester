#!/usr/bin/env python3
"""Generate mobile branding from the desktop icon. Requires Pillow."""

from pathlib import Path
from shutil import copyfile

from PIL import Image, ImageDraw


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "apps/desktop/src-tauri/icons/icon.png"
MOBILE = ROOT / "apps/mobile"
RES = MOBILE / "android/app/src/main/res"
ASSETS = MOBILE / "ios/App/App/Assets.xcassets"
BACKGROUND = "#e9ebf2"


def save_image(image: Image.Image, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    image.save(path, optimize=True)


def rounded_icon(source: Image.Image, size: int) -> Image.Image:
    icon = source.resize((size, size), Image.Resampling.LANCZOS)
    mask = Image.new("L", icon.size)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, size - 1, size - 1), radius=size * 0.23, fill=255)
    icon.putalpha(mask)
    return icon


def main() -> None:
    source = Image.open(SOURCE).convert("RGBA")
    for obsolete in RES.glob("drawable*/splash.png"):
        obsolete.unlink()
    for obsolete in ("drawable-v24/ic_launcher_foreground.xml", "drawable/ic_launcher_background.xml"):
        (RES / obsolete).unlink(missing_ok=True)
    web_icon = MOBILE / "src/assets/brand-icon.png"
    web_icon.parent.mkdir(parents=True, exist_ok=True)
    copyfile(SOURCE, web_icon)
    public_icon = MOBILE / "public/favicon.png"
    save_image(source.resize((64, 64), Image.Resampling.LANCZOS), public_icon)

    # Preserve the full desktop composition for legacy launchers. Adaptive
    # launchers need the face within their 66 dp safe area on a 108 dp layer.
    for density, scale in (("mdpi", 1), ("hdpi", 1.5), ("xhdpi", 2), ("xxhdpi", 3), ("xxxhdpi", 4)):
        folder = RES / f"mipmap-{density}"
        size = int(48 * scale)
        legacy = source.resize((size, size), Image.Resampling.LANCZOS)
        save_image(legacy, folder / "ic_launcher.png")
        save_image(legacy, folder / "ic_launcher_round.png")
        canvas_size, icon_size = int(108 * scale), int(72 * scale)
        foreground = Image.new("RGBA", (canvas_size, canvas_size))
        inset = (canvas_size - icon_size) // 2
        foreground.alpha_composite(rounded_icon(source, icon_size), (inset, inset))
        save_image(foreground, folder / "ic_launcher_foreground.png")

    # Android's splash icon uses a 288 dp canvas with a 192 dp safe circle.
    # The avatar is inset so the platform's circle mask keeps the face visible.
    splash = Image.new("RGBA", (1152, 1152))
    splash.alpha_composite(rounded_icon(source, 704), (224, 224))
    save_image(splash, RES / "drawable-nodpi/brand_splash_icon.png")

    # iOS app icons must be opaque. The LaunchScreen image is a separate small
    # asset constrained by the storyboard, instead of a full-screen template.
    ios_icon = Image.new("RGB", (1024, 1024), BACKGROUND)
    scaled = source.resize((1024, 1024), Image.Resampling.LANCZOS)
    ios_icon.paste(scaled, mask=scaled.getchannel("A"))
    save_image(ios_icon, ASSETS / "AppIcon.appiconset/AppIcon-512@2x.png")
    for filename, size in (("splash-2732x2732-2.png", 112), ("splash-2732x2732-1.png", 224), ("splash-2732x2732.png", 336)):
        save_image(rounded_icon(source, size), ASSETS / "Splash.imageset" / filename)
    print(f"Mobile branding generated from {SOURCE.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
