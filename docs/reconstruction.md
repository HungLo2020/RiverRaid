# Reconstruction record

The target is the supplied Atari 2600 NTSC River Raid cartridge. The executable is
an independent native Rust implementation, with no ROM dependency, CPU emulator,
assembly interpreter, or runtime disassembler.

## Identity and source agreement

The Downloads ZIP has SHA-256
`1e477ada84990302d8ab2c040a9361084f77de186f6aa74d0c48432ad8a003db`.
Its 16,384-byte member consists of four identical 4,096-byte copies. The canonical
image has SHA-256 `4c6842b8af64fc75e599b773821b3e6bee8e5028f5d63414821b0d63f0b2dfe9`
and MD5 `393948436d1f4cc3192410bb918f9724`. See [identity evidence](evidence/cartridge.json).

Thomas Jentzsch's [annotated disassembly](https://github.com/johnidm/asm-atari-2600/blob/6ac91715a4a837987f545c6172489dd6a96849a3/riverraid.asm)
(version 0.9, 2001-08-13) was assembled with DASM 2.20.14.1, NTSC enabled, trainer
disabled, and the compatible read-register base (`-DTIA_BASE_READ_ADDRESS=48`).
The result matched **every byte** of the canonical cartridge. This identifies the
listing as an authoritative explanation of the selected executable, while its
comments still require checking against instructions and runtime behavior.

For example, the loop described as three scrolling updates actually performs two.
The nominal sixteen river blocks each contain two generated 32-line parts, so a
section spans 1,024 lines. The horizontal accumulator and its carry on a direction
change affect the first frame of steering; a generic acceleration approximation
would not reproduce that behavior.

## REA evidence

[Reverse Engineer Anything](https://github.com/morluto/rea) provided ZIP inventory
and Ghidra 12.1.4 analysis. The canonical image was explicitly imported as NMOS
6502 bytes at `$f000`. This raw mapping does not model console hardware or execute
the game. Ghidra's automatic function discovery was incomplete in the display
kernel; a rejected generator-procedure lookup was followed by a bounded data read
at the known source address rather than presenting it as a recovered function.

| Retained evidence | Observation |
|---|---|
| [Archive ledger](evidence/rom-inventory.json) | Bounded archive inventory and original carrier identity. |
| [Native ledger](evidence/rea-rom.json) | Function inventory, initialization assembly, generator and scoring bytes. |
| `ev_fbe44697a48c35b00dfd9c3e152b91a62e9532e1519b4feea6efeca1934cd6e8` | Initialization routine at `$fa8f`, including source initialization table and six block buffers. |
| `ev_e83edb5b87ec23064fe814a5b7f5ed65c74e43c531b603852c35f38c0dda7d3e` | Complete 24-byte read at `$fac6`, containing the 16-bit river shift-register routine. |
| `ev_d63052b27b0a62e2e57430006b6626b1a2845a006e85b85e3d7482933e8db250` | Complete 11-byte scoring-encoding read at `$fe53`. Entries are display-pointer increments, not literal point values. |

## Runtime comparisons

MAME 0.285 ran the cartridge entirely offscreen (`-video none`, dummy SDL,
no host DISPLAY). Inputs were sent to the emulated machine's ports through its
Lua API. RAM snapshots and screen PNGs did not involve desktop input automation.

The [generator fixture](evidence/generator-vectors.json) contains 289 consecutive
observations across ten levels. It checks the shift-register state, section and
part counters, island state, pattern IDs, both full playfield masks, road/color
flags, object types, positions and reflection. The long observation uses a
separately hashed source-derived trainer with seven changed bytes to disable
player collision deaths and fuel loss. The generator instructions remain
unchanged, and only the normal maximum speed is forced in RAM. This instrumented
capture establishes generator behavior; it does not establish original death or
fuel behavior. Fresh plane position sampling has an explicitly checked one-frame
movement phase. A separate first-level fixture uses the unmodified cartridge.

The [flight fixture](evidence/motion-vectors.json) uses the **unmodified** ROM with
ordinary reset, fire, left/right, and up/down inputs. It records per-frame position,
horizontal accumulator and acceleration, vertical speed, fuel, scroll offset and
fractional remainder. All 124 consecutive native frames match these observables
with collisions enabled, followed by the same helicopter collision on frame 285.
This establishes the observed sequence; it does not prove every TIA collision
latch edge case.

Sprite occupancy is shared between rendering and game collisions. Original frame
captures exposed the B/A interleave order and the jet's leading blank scanline;
those bitmap details now have regression tests.

## Reproduce

Static inspection needs Python only:

```sh
python3 tools/reference.py /path/to/cartridge.zip --output /tmp/riverraid-identity.json --image /tmp/riverraid.bin
```

Reference execution needs a local MAME build with Atari 2600 support. These tools
never download a cartridge, open a desktop window, or become part of the release:

```sh
python3 tools/reference_probe.py /path/to/cartridge.zip --mode motion --output-dir /tmp/rr-motion
python3 tools/reference_probe.py /path/to/cartridge.zip --mode generator --output-dir /tmp/rr-generator
```

Native window automation is separately isolated in `tools/window_smoke.py`. It
always starts its own private Xvfb server and provides that DISPLAY to every
client. Its game PID must match before any input is addressed to the owned window.
No host desktop settings or other game repositories are changed.

See [mechanics](mechanics.md) for the bounded conformance claims and remaining
presentation, timing, and hardware differences, and [Basalt integration](basalt.md)
for packaging and data placement.
