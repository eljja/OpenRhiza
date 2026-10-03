from __future__ import annotations

import pathlib

from font_import_skill import (
    DEFAULT_BASELINE_OFFSET_Y,
    DEFAULT_FONT_SIZE,
    DEFAULT_GLYPH_HEIGHT,
    DEFAULT_GLYPH_WIDTH,
    ROOT,
    build_atlas,
)


ASSET_DIR = ROOT / "assets" / "fonts"
SOURCE_FONT = ASSET_DIR / "NotoSansKR-Regular.ttf"
ATLAS_PATH = ASSET_DIR / "noto_sans_kr_ui_16x24.bin"
MANIFEST_PATH = ASSET_DIR / "noto_sans_kr_ui_16x24.json"


def main() -> None:
    build_atlas(
        pathlib.Path(SOURCE_FONT),
        pathlib.Path(ATLAS_PATH),
        pathlib.Path(MANIFEST_PATH),
        charset="gui_kr",
        glyph_width=DEFAULT_GLYPH_WIDTH,
        glyph_height=DEFAULT_GLYPH_HEIGHT,
        font_size=DEFAULT_FONT_SIZE,
        baseline_offset_y=DEFAULT_BASELINE_OFFSET_Y,
        collection_index=0,
    )
    print(f"wrote {ATLAS_PATH} and {MANIFEST_PATH}")


if __name__ == "__main__":
    main()
