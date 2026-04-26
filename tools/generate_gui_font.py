from __future__ import annotations

import io
import pathlib
import urllib.request

from PIL import Image, ImageDraw, ImageFont


ROOT = pathlib.Path(__file__).resolve().parent.parent
ASSET_DIR = ROOT / "assets" / "fonts"
SOURCE_FONT = ASSET_DIR / "NotoSansKR-Regular.ttf"
ATLAS_PATH = ASSET_DIR / "noto_sans_kr_ui_16x24.bin"
LICENSE_PATH = ASSET_DIR / "NotoSansKR-OFL.txt"

FONT_URL = "https://raw.githubusercontent.com/google/fonts/main/ofl/notosanskr/NotoSansKR%5Bwght%5D.ttf"
LICENSE_URL = "https://raw.githubusercontent.com/google/fonts/main/ofl/notosanskr/OFL.txt"

GLYPH_WIDTH = 16
GLYPH_HEIGHT = 24
FONT_SIZE = 18
BASELINE_OFFSET_Y = 2

CHARS = [chr(code) for code in range(0x20, 0x7F)]
CHARS.extend(chr(code) for code in range(0xAC00, 0xD7A4))


def ensure_download(path: pathlib.Path, url: str) -> None:
    if path.exists():
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    with urllib.request.urlopen(url, timeout=60) as response:
        data = response.read()
    path.write_bytes(data)


def render_glyph(font: ImageFont.FreeTypeFont, ch: str) -> bytes:
    canvas = Image.new("L", (GLYPH_WIDTH, GLYPH_HEIGHT), 0)
    draw = ImageDraw.Draw(canvas)

    bbox = draw.textbbox((0, 0), ch, font=font)
    glyph_w = bbox[2] - bbox[0]
    glyph_h = bbox[3] - bbox[1]

    x = max(0, (GLYPH_WIDTH - glyph_w) // 2 - bbox[0])
    y = max(0, (GLYPH_HEIGHT - glyph_h) // 2 - bbox[1] + BASELINE_OFFSET_Y)
    draw.text((x, y), ch, fill=255, font=font)
    return canvas.tobytes()


def main() -> None:
    ensure_download(SOURCE_FONT, FONT_URL)
    ensure_download(LICENSE_PATH, LICENSE_URL)

    font = ImageFont.truetype(str(SOURCE_FONT), FONT_SIZE)
    atlas = bytearray()
    for ch in CHARS:
        atlas.extend(render_glyph(font, ch))

    ATLAS_PATH.write_bytes(atlas)
    print(f"wrote {ATLAS_PATH} ({len(atlas)} bytes)")


if __name__ == "__main__":
    main()
