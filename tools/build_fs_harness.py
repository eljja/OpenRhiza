import argparse
import pathlib
import shutil
import struct

BLOCK_SIZE = 512
SCRATCH_BLOCKS_DEFAULT = 128
MAGIC = b"ORFSHAR1"
VERSION = 1

FS_HINTS = {
    "fat32": 1,
    "exfat": 2,
    "ntfs": 3,
    "ext2": 4,
    "ext3": 5,
    "ext4": 6,
}


def align_up(value: int, alignment: int) -> int:
    return ((value + alignment - 1) // alignment) * alignment


def build_harness(source: pathlib.Path, output: pathlib.Path, fs_family: str, scratch_blocks: int) -> None:
    if fs_family not in FS_HINTS:
        raise ValueError(f"Unsupported fs family '{fs_family}'")

    source_size = source.stat().st_size
    filesystem_bytes = align_up(source_size, BLOCK_SIZE)
    filesystem_blocks = filesystem_bytes // BLOCK_SIZE
    scratch_bytes = scratch_blocks * BLOCK_SIZE

    output.parent.mkdir(parents=True, exist_ok=True)
    with source.open("rb") as src, output.open("wb") as dst:
        shutil.copyfileobj(src, dst)
        if filesystem_bytes > source_size:
            dst.write(b"\x00" * (filesystem_bytes - source_size))
        dst.write(b"\x00" * scratch_bytes)

        footer = bytearray(BLOCK_SIZE)
        footer[0:8] = MAGIC
        struct.pack_into("<I", footer, 8, VERSION)
        struct.pack_into("<I", footer, 12, FS_HINTS[fs_family])
        struct.pack_into("<I", footer, 16, 0)  # fs_start_lba
        struct.pack_into("<I", footer, 20, filesystem_blocks)
        struct.pack_into("<I", footer, 24, filesystem_blocks)  # scratch start lba in virtual view
        struct.pack_into("<I", footer, 28, scratch_blocks)
        label = f"OpenRhiza {fs_family} harness".encode("ascii", "ignore")[:63]
        footer[32:32 + len(label)] = label
        dst.write(footer)

    print(
        f"Built harness {output} from {source} "
        f"(fs_blocks={filesystem_blocks}, scratch_blocks={scratch_blocks}, total_blocks={filesystem_blocks + scratch_blocks + 1})"
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Build an OpenRhiza image-backed filesystem harness disk.")
    parser.add_argument("--source", required=True, help="Path to source filesystem image")
    parser.add_argument("--output", default="fs_harness.img", help="Output harness image path")
    parser.add_argument("--fs", required=True, choices=sorted(FS_HINTS.keys()), help="Filesystem family hint")
    parser.add_argument("--scratch-blocks", type=int, default=SCRATCH_BLOCKS_DEFAULT, help="Number of writable scratch blocks appended after the filesystem image")
    args = parser.parse_args()

    source = pathlib.Path(args.source).resolve()
    output = pathlib.Path(args.output).resolve()
    build_harness(source, output, args.fs, args.scratch_blocks)


if __name__ == "__main__":
    main()
