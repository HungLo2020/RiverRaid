#!/usr/bin/env python3
"""Check a release ZIP against the Basalt standard without extracting it."""

from __future__ import annotations

import argparse
from pathlib import Path, PurePosixPath
import stat
import struct
import zipfile


def validate_archive(archive: Path, executable: str = "app/bin/river-raid") -> None:
    """Raise ValueError for an unsafe or incomplete Basalt release."""
    with zipfile.ZipFile(archive) as package:
        entries = package.infolist()
        if not entries:
            raise ValueError("archive is empty")
        seen: set[str] = set()
        files: set[PurePosixPath] = set()
        paths: set[PurePosixPath] = set()
        for entry in entries:
            path = PurePosixPath(entry.filename)
            if (
                path.is_absolute()
                or ".." in path.parts
                or "\\" in entry.filename
                or not path.parts
                or path.parts[0] != "app"
                or entry.filename != path.as_posix() + ("/" if entry.is_dir() else "")
            ):
                raise ValueError(f"unsafe or non-app archive path: {entry.filename!r}")
            if entry.filename in seen:
                raise ValueError(f"duplicate archive path: {entry.filename!r}")
            seen.add(entry.filename)
            paths.add(path)
            if not entry.is_dir():
                files.add(path)
            mode = entry.external_attr >> 16
            if stat.S_ISLNK(mode):
                raise ValueError(f"symlinks are not allowed: {entry.filename!r}")
            if entry.flag_bits & 1:
                raise ValueError("encrypted entries are not supported")
        for path in paths:
            if any(parent in files for parent in path.parents):
                raise ValueError(f"archive file also used as a directory: {path}")
        if executable not in seen:
            raise ValueError(f"missing executable: {executable}")
        binary = package.getinfo(executable)
        if binary.is_dir() or binary.file_size == 0:
            raise ValueError("executable is empty or is a directory")
        if not (binary.external_attr >> 16) & stat.S_IXUSR:
            raise ValueError("executable does not have its execute bit set")
        if "app/README.md" not in seen:
            raise ValueError("missing app/README.md")
        if "app/LICENSE" not in seen:
            raise ValueError("missing app/LICENSE")
        if "app/basalt/cover.png" not in seen:
            raise ValueError("missing app/basalt/cover.png")
        cover = package.read("app/basalt/cover.png")
        if len(cover) >= 2_000_000 or len(cover) < 33 or cover[:8] != b"\x89PNG\r\n\x1a\n":
            raise ValueError("cover must be a PNG smaller than 2 MB")
        if cover[12:16] != b"IHDR":
            raise ValueError("cover has no PNG image header")
        width, height, depth, color = struct.unpack(">IIBB", cover[16:26])
        if width < 600 or height < 800 or width * 4 != height * 3:
            raise ValueError("cover must be 3:4 portrait and at least 600x800")
        if depth != 8 or color not in (0, 2):
            raise ValueError("cover must be an opaque 8-bit grayscale or RGB PNG")
        offset = 8
        end_seen = False
        while offset + 12 <= len(cover):
            size = struct.unpack(">I", cover[offset:offset + 4])[0]
            kind = cover[offset + 4:offset + 8]
            if offset + 12 + size > len(cover):
                raise ValueError("cover contains a truncated PNG chunk")
            if kind == b"tRNS":
                raise ValueError("cover must not have transparency")
            offset += 12 + size
            if kind == b"IEND":
                end_seen = True
                break
        if not end_seen:
            raise ValueError("cover has no PNG end chunk")
        corrupt = package.testzip()
        if corrupt is not None:
            raise ValueError(f"corrupt ZIP member: {corrupt}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--executable", default="app/bin/river-raid")
    args = parser.parse_args()
    try:
        validate_archive(args.archive, args.executable)
    except (OSError, ValueError, zipfile.BadZipFile) as error:
        parser.exit(1, f"Invalid release: {error}\n")
    print(f"Validated {args.archive}")


if __name__ == "__main__":
    main()
