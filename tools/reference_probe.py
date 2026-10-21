#!/usr/bin/env python3
"""Observe a supplied cartridge in MAME without opening a desktop window.

Generator mode uses an explicitly identified seven-byte trainer derivative;
flight mode uses the unmodified canonical cartridge. No ROM is distributed.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
from reference import inspect

TRAINER_SHA256 = "97f6ad5d457ecb787d2d17d6379258d67cf00ec7a9aeaf37b57c357a00c0733d"
PATCHES = [(0x2e8, 2, 243), (0x2ee, 4, 243), (0x2f4, 7, 243),
           (0x463, 2, 243), (0x483, 36, 165), (0x484, 7, 243), (0x63e, 32, 0)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rom", type=Path)
    parser.add_argument("--mame", default="mame")
    parser.add_argument("--mode", choices=("motion", "generator"), default="motion")
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    identity, canonical = inspect(args.rom)
    image = bytearray(canonical)
    if args.mode == "generator":
        for offset, before, after in PATCHES:
            assert image[offset] == before
            image[offset] = after
        assert hashlib.sha256(image).hexdigest() == TRAINER_SHA256
    output = args.output_dir.resolve()
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="riverraid-reference-") as name:
        temporary = Path(name)
        cartridge = temporary / "reference.bin"
        cartridge.write_bytes(image)
        environment = {**os.environ, "DISPLAY": "", "WAYLAND_DISPLAY": "", "SDL_VIDEODRIVER": "dummy",
                       "RIVERRAID_PROBE_OUT": str(output), "RIVERRAID_PROBE_MODE": args.mode}
        command = [args.mame, "a2600", "-cart", str(cartridge), "-video", "none", "-sound", "none",
                   "-skip_gameinfo", "-seconds_to_run", "80" if args.mode == "generator" else "6",
                   "-nothrottle", "-noreadconfig", "-nowriteconfig", "-autoboot_delay", "0",
                   "-autoboot_script", str(Path(__file__).with_name("mame_probe.lua").resolve()),
                   "-cfg_directory", str(temporary / "cfg"), "-nvram_directory", str(temporary / "nvram"),
                   "-snapshot_directory", str(output)]
        subprocess.run(command, cwd=temporary, env=environment, check=True, timeout=120)
    report = {"mode": args.mode, "canonical_rom_sha256": identity["canonical_sha256"],
              "executed_image_sha256": hashlib.sha256(image).hexdigest(),
              "changed_bytes": PATCHES if args.mode == "generator" else [],
              "desktop": "none: MAME video none, dummy SDL, no DISPLAY"}
    (output / "provenance.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
