# OpenRhiza Font Import Skill

This document defines the current OpenRhiza path for importing existing fonts into the GUI without moving font parsing into the kernel core.

## Purpose

OpenRhiza should be able to reuse existing font files directly, but the font parsing and atlas generation logic must stay outside the minimal survival core.

The correct place for this capability is a skill or workflow that:

1. accepts an existing source font
2. normalizes the source if needed
3. generates an OpenRhiza atlas and manifest
4. validates the atlas in a sandbox or bootstrap session
5. promotes the atlas for GUI use only after successful validation

## Core rule

- Do not put TTF, OTF, TTC, OTC, WOFF, or WOFF2 parsing into the core.
- Keep only the minimum text rendering handoff and atlas consumption path in the core.
- Treat font import as a host-side skill or workflow capability.

## Current implementation

OpenRhiza now includes a host-side font import skill tool:

- [D:\python\github\OpenRhiza\OpenRhiza\tools\font_import_skill.py](D:\python\github\OpenRhiza\OpenRhiza\tools\font_import_skill.py)

Supported source extensions:

- `.ttf`
- `.otf`
- `.ttc`
- `.otc`
- `.woff`
- `.woff2`

Current preset charsets:

- `ascii`
- `gui_kr`

The `gui_kr` preset currently includes:

- printable ASCII
- Hangul Compatibility Jamo
- Hangul syllables

## Example usage

Generate a GUI atlas from an existing font:

```powershell
python .\tools\font_import_skill.py `
  --source .\assets\fonts\NotoSansKR-Regular.ttf `
  --output-bin .\assets\fonts\noto_sans_kr_ui_16x24.bin `
  --manifest .\assets\fonts\noto_sans_kr_ui_16x24.json `
  --charset gui_kr `
  --glyph-width 16 `
  --glyph-height 24 `
  --font-size 18 `
  --baseline-offset-y 2
```

## Default GUI atlas build path

The existing convenience wrapper remains:

- [D:\python\github\OpenRhiza\OpenRhiza\tools\generate_gui_font.py](D:\python\github\OpenRhiza\OpenRhiza\tools\generate_gui_font.py)

It now delegates to the font import skill tool rather than owning a separate font pipeline.

## Next direction

This should evolve into a true OpenRhiza capability workflow:

1. query registry for font import skill/workflow
2. ingest existing font asset
3. build atlas and manifest
4. validate GUI rendering
5. cache locally
6. upload atlas metadata and evaluation to OpenRhiza.com when useful

The font import pipeline should eventually be invocable from inside OpenRhiza through the console and the internal LLM, even if the bootstrap tool currently lives outside the guest runtime.
