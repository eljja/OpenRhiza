from __future__ import annotations

import argparse
import json
import pathlib
import tempfile
from contextlib import contextmanager
from typing import Iterator

from fontTools.ttLib import TTFont
from PIL import Image, ImageDraw, ImageFont


ROOT = pathlib.Path(__file__).resolve().parent.parent

DEFAULT_GLYPH_WIDTH = 16
DEFAULT_GLYPH_HEIGHT = 24
DEFAULT_FONT_SIZE = 18
DEFAULT_BASELINE_OFFSET_Y = 2

ASCII_RANGE = range(0x20, 0x7F)
COMPAT_JAMO_RANGE = range(0x3131, 0x3164)
HANGUL_SYLLABLE_RANGE = range(0xAC00, 0xD7A4)


def build_charset(name: str) -> list[str]:
    normalized = name.strip().lower()
    if normalized == "ascii":
        return [chr(code) for code in ASCII_RANGE]
    if normalized == "gui_kr":
        chars = [chr(code) for code in ASCII_RANGE]
        chars.extend(chr(code) for code in COMPAT_JAMO_RANGE)
        chars.extend(chr(code) for code in HANGUL_SYLLABLE_RANGE)
        return chars
    raise ValueError(f"unsupported charset preset: {name}")


@contextmanager
def normalized_font_path(source: pathlib.Path) -> Iterator[pathlib.Path]:
    suffix = source.suffix.lower()
    if suffix not in {".woff", ".woff2"}:
        yield source
        return

    with tempfile.TemporaryDirectory(prefix="openrhiza-font-") as temp_dir:
        temp_path = pathlib.Path(temp_dir) / (source.stem + ".ttf")
        font = TTFont(str(source))
        font.flavor = None
        font.save(str(temp_path))
        yield temp_path


def load_font(source: pathlib.Path, size: int, collection_index: int) -> ImageFont.FreeTypeFont:
    with normalized_font_path(source) as usable_path:
        return ImageFont.truetype(str(usable_path), size, index=collection_index)


def render_glyph(
    font: ImageFont.FreeTypeFont,
    ch: str,
    glyph_width: int,
    glyph_height: int,
    baseline_offset_y: int,
) -> bytes:
    canvas = Image.new("L", (glyph_width, glyph_height), 0)
    draw = ImageDraw.Draw(canvas)
    bbox = draw.textbbox((0, 0), ch, font=font)
    glyph_w = bbox[2] - bbox[0]
    glyph_h = bbox[3] - bbox[1]
    x = max(0, (glyph_width - glyph_w) // 2 - bbox[0])
    y = max(0, (glyph_height - glyph_h) // 2 - bbox[1] + baseline_offset_y)
    draw.text((x, y), ch, fill=255, font=font)
    return canvas.tobytes()


def build_atlas(
    source: pathlib.Path,
    output_bin: pathlib.Path,
    manifest_path: pathlib.Path,
    *,
    charset: str,
    glyph_width: int,
    glyph_height: int,
    font_size: int,
    baseline_offset_y: int,
    collection_index: int,
) -> None:
    chars = build_charset(charset)
    font = load_font(source, font_size, collection_index)

    atlas = bytearray()
    for ch in chars:
        atlas.extend(
            render_glyph(
                font,
                ch,
                glyph_width=glyph_width,
                glyph_height=glyph_height,
                baseline_offset_y=baseline_offset_y,
            )
        )

    output_bin.parent.mkdir(parents=True, exist_ok=True)
    output_bin.write_bytes(atlas)

    manifest = {
        "source_font": str(source),
        "charset": charset,
        "glyph_width": glyph_width,
        "glyph_height": glyph_height,
        "font_size": font_size,
        "baseline_offset_y": baseline_offset_y,
        "collection_index": collection_index,
        "glyph_count": len(chars),
        "atlas_bytes": len(atlas),
        "supported_source_extensions": [
            ".ttf",
            ".otf",
            ".ttc",
            ".otc",
            ".woff",
            ".woff2",
        ],
    }
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="OpenRhiza font import skill: build a GUI atlas from an existing font file."
    )
    parser.add_argument("--source", required=True, help="Path to source font (.ttf/.otf/.ttc/.otc/.woff/.woff2)")
    parser.add_argument("--output-bin", required=True, help="Output atlas binary path")
    parser.add_argument("--manifest", required=True, help="Output manifest JSON path")
    parser.add_argument("--charset", default="gui_kr", help="Charset preset (ascii, gui_kr)")
    parser.add_argument("--glyph-width", type=int, default=DEFAULT_GLYPH_WIDTH)
    parser.add_argument("--glyph-height", type=int, default=DEFAULT_GLYPH_HEIGHT)
    parser.add_argument("--font-size", type=int, default=DEFAULT_FONT_SIZE)
    parser.add_argument("--baseline-offset-y", type=int, default=DEFAULT_BASELINE_OFFSET_Y)
    parser.add_argument("--collection-index", type=int, default=0)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    source = pathlib.Path(args.source)
    output_bin = pathlib.Path(args.output_bin)
    manifest = pathlib.Path(args.manifest)

    build_atlas(
        source,
        output_bin,
        manifest,
        charset=args.charset,
        glyph_width=args.glyph_width,
        glyph_height=args.glyph_height,
        font_size=args.font_size,
        baseline_offset_y=args.baseline_offset_y,
        collection_index=args.collection_index,
    )
    print(f"wrote {output_bin} and {manifest}")


if __name__ == "__main__":
    main()
