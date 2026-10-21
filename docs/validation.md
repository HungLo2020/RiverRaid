# Release validation

Version 0.1.0 was checked with the supplied NTSC cartridge and a Linux x64 release build.
These results establish the specific comparisons below, rather than complete console equivalence.

| Check | Result |
|---|---|
| Reference identity | Four identical 4 KiB banks; independently assembled reference matches all 4,096 bytes. |
| River generation | 289 consecutive generated parts across levels 1–10 match the runtime fixture; 17 independent first-level observations also match. |
| Flight | All eight recorded motion/fuel/scroll observables match on 124 consecutive frames with collisions enabled; helicopter collision follows on original frame 285. |
| Rust | Formatting and strict all-target Clippy pass; 28 library and three frontend tests pass. |
| Packaging | Eight Python tests pass; release ZIP passes layout, executable-permission, and cover validation. |
| Replay | 12,000 recorded input frames reproduce the exact final framebuffer and state summary. |
| Relocation | Packaged executable works from a path containing spaces, with read-only `app/` and working directory `/`. Headless verification creates no installation data. |
| Native window | Private Xvfb test passes application class, PID, normal window type, full opacity, opaque region at three sizes, help, pause/resume, normal quit, and install-relative saves. |
| Host ALSA | Executable links host `libasound.so.2`; private-display run opens the host ALSA `null` PCM and exits normally. This verifies the playback path without playing sound on the user's speakers. |

Machine-readable local results are in [window checks](evidence/window-check.json),
[ALSA checks](evidence/alsa-window-check.json), and [package checks](evidence/package-check.json).
The [native frame](evidence/native-frame240.png) uses the recorded inputs through original
frame 240. It is a renderer capture, with no desktop screenshot involved.

CI repeats source checks, tests, the optimized build, private-display checks, and packaging
on Ubuntu 22.04. Tagged releases repeat these checks before publishing a Linux x64 ZIP.
See [Basalt integration](basalt.md) for launcher verification and installation.

The exact source and runtime evidence, including the explicitly identified generator trainer,
is described in [the reconstruction record](reconstruction.md). Remaining sound, timing,
patrol, and checkpoint differences are listed in [mechanics](mechanics.md).
