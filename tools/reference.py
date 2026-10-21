#!/usr/bin/env python3
"""Identify a locally supplied River Raid ROM; optionally write its canonical image.

Accepts the supplied four-times-mirrored cartridge archive or a single 4KiB image.
Never downloads a ROM. Runtime Rust code does not use the image or this tool.
"""
import argparse
import hashlib
import json
from pathlib import Path
import zipfile

EXPECTED = "4c6842b8af64fc75e599b773821b3e6bee8e5028f5d63414821b0d63f0b2dfe9"


def inspect(path):
    carrier = path.read_bytes()
    member = None
    if zipfile.is_zipfile(path):
        with zipfile.ZipFile(path) as archive:
            candidates = [i for i in archive.infolist() if not i.is_dir()]
            if len(candidates) != 1 or candidates[0].file_size > 65536:
                raise ValueError("expected exactly one bounded cartridge image")
            member = candidates[0].filename
            image = archive.read(candidates[0])
    else:
        image = carrier
    if len(image) not in (4096, 8192, 16384):
        raise ValueError("unsupported cartridge length")
    canonical = image[:4096]
    if any(image[i:i+4096] != canonical for i in range(0, len(image), 4096)):
        raise ValueError("cartridge banks differ; do not assume a mirrored 4KiB image")
    digest = hashlib.sha256(canonical).hexdigest()
    if digest != EXPECTED:
        raise ValueError("cartridge is not the analyzed NTSC River Raid revision")
    evidence = {
        "format": "RiverRaid cartridge identity v1",
        "carrier_sha256": hashlib.sha256(carrier).hexdigest(),
        "member": member, "image_length": len(image),
        "image_sha256": hashlib.sha256(image).hexdigest(),
        "identical_4096_byte_copies": len(image)//4096,
        "canonical_sha256": digest, "canonical_md5": hashlib.md5(canonical).hexdigest(),
        "canonical_length": 4096, "mapped_at": "0xf000", "reset_vector": "0xf000",
        "tables": {
            "lfsr_routine": {"address": "0xfac6", "bytes": canonical[0xac6:0xad3].hex()},
            "score_encoding": {"address": "0xfe53", "bytes": canonical[0xe53:0xe5e].hex()},
        },
        "limitations": ["Static identity and table reads do not establish full runtime equivalence."],
    }
    return evidence, canonical


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rom", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--image", type=Path, help="Write canonical image for external REA/emulator analysis")
    parser.add_argument("--reassembled", type=Path, help="Compare an independently assembled reference image")
    args = parser.parse_args()
    try:
        evidence, canonical = inspect(args.rom)
        if args.reassembled:
            assembled = args.reassembled.read_bytes()
            evidence["source_reassembly_matches_all_4096_bytes"] = assembled == canonical
            evidence["source_reassembly_sha256"] = hashlib.sha256(assembled).hexdigest()
            if assembled != canonical:
                raise ValueError("reassembled source differs from the selected cartridge")
        text = json.dumps(evidence, indent=2) + "\n"
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(text)
        else:
            print(text, end="")
        if args.image:
            args.image.write_bytes(canonical)
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as error:
        parser.exit(1, f"Reference inspection failed: {error}\n")


if __name__ == "__main__":
    main()
