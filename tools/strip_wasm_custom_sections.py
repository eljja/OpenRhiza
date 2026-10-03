from __future__ import annotations

import sys
from pathlib import Path


def read_u32_leb(data: bytes, offset: int) -> tuple[int, int]:
    result = 0
    shift = 0
    for _ in range(5):
        if offset >= len(data):
            raise ValueError("unexpected EOF while reading LEB128")
        byte = data[offset]
        offset += 1
        result |= (byte & 0x7F) << shift
        if (byte & 0x80) == 0:
            return result, offset
        shift += 7
    raise ValueError("invalid LEB128 sequence")


def strip_custom_sections(data: bytes) -> bytes:
    if len(data) < 8 or data[:4] != b"\0asm":
        raise ValueError("input is not a WebAssembly module")

    output = bytearray(data[:8])
    offset = 8

    while offset < len(data):
        section_id = data[offset]
        section_start = offset
        offset += 1
        payload_len, offset = read_u32_leb(data, offset)
        payload_end = offset + payload_len
        if payload_end > len(data):
            raise ValueError(
                f"section {section_id} exceeds file length: end={payload_end}, len={len(data)}"
            )

        if section_id != 0:
            output.extend(data[section_start:payload_end])

        offset = payload_end

    return bytes(output)


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: strip_wasm_custom_sections.py <src> <dst>", file=sys.stderr)
        return 2

    src = Path(sys.argv[1])
    dst = Path(sys.argv[2])
    stripped = strip_custom_sections(src.read_bytes())
    dst.write_bytes(stripped)
    print(f"stripped custom sections: {src} -> {dst} ({len(stripped)} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
