# River Raid

A native Rust reconstruction of **Activision's 1982 Atari 2600 River Raid**, designed by Carol Shaw. The game implements its own simulation and procedural river generator. It neither loads a ROM nor emulates the console.

The supplied cartridge, annotated disassembly, REA/Ghidra evidence, and offscreen runtime observations guide the implementation. Tests compare 289 consecutive river generations across ten difficulty levels with the reference. See [mechanics and fidelity limits](docs/mechanics.md) and [the reconstruction record](docs/reconstruction.md).

The [validation record](docs/validation.md) covers flight comparisons, replay, packaging, window behavior, and host ALSA checks.

## Play

Unpack the Linux release ZIP and run `app/bin/river-raid`. On Linux it uses X11/XWayland and the host's ALSA library (`libasound.so.2`). No terminal interaction or network connection is required to start.

| Control | Action |
|---|---|
| Enter | Start or resume |
| Left/right arrows or A/D | Steer |
| Up/down arrows or W/S | Accelerate / slow down |
| Space or left mouse button | Fire; hold to keep firing |
| P or Escape | Pause / resume |
| F1 | Controls |
| M | Mute |
| Q while paused | Quit |

Fly through fuel depots to refuel, shoot enemies and bridges, and avoid riverbanks and islands. The default missile follows your horizontal movement. Destruction of a bridge establishes the next checkpoint. Extra lives arrive every 10,000 points. Controller support is deferred.

The window is resizable, keeps a 4:3 display with nearest-neighbor sampling, and submits opaque pixels and proper application identity metadata. Losing focus pauses play. The game does not change desktop settings. Select an ALSA device with `--audio-device NAME`, or disable playback with `--no-audio`.

## Build

Requires Rust 1.85 or newer, a C compiler, and Linux packages `pkg-config libasound2-dev libx11-dev`. Release CI builds on Ubuntu 22.04; Linux x64 is the verified target.

```sh
cargo run --release
```

The code separates deterministic game rules, river generation, shared sprite masks, software rendering, sound synthesis, ALSA playback, window identity, input, and persistence. Original cartridge instructions are never executed by the game.

## Verify

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
python3 -m unittest discover -s tools -p 'test_*.py'
```

Window checks require Xvfb, xdotool, and xprop. The harness always starts its own private display and never connects to the caller's desktop:

```sh
cargo build --release
python3 tools/window_smoke.py
```

A fixed simulation step, fixed original river seed, and recorded input make runs reproducible within the same implementation version:

```sh
cargo run --release -- --headless --ticks 12000 --autoplay --record /tmp/river-flight.json --snapshot /tmp/river-flight.ppm
cargo run --release -- --headless --replay /tmp/river-flight.json --snapshot /tmp/river-replay.ppm
cmp /tmp/river-flight.ppm /tmp/river-replay.ppm
```

`--autoplay` is an imperfect test driver. It is off during normal play. `--record FILE` also records real play. Snapshots use native 160×210 PPM. `--smoke-frames N` bounds a native window run; use the isolated harness for automation.

## Release and Basalt

```sh
cargo build --release --locked
python3 tools/package.py
python3 tools/validate_package.py dist/RiverRaid-0.1.0-linux-x64.zip
```

The archive contains only `app/`, including `app/bin/river-raid` and the original 900×1200 launcher cover. A `v<version>` tag runs the release workflow and publishes the ZIP. The [catalog entry](packaging/RiverRaid.json) and [integration notes](docs/basalt.md) target Basalt standard v1 without changes to Basalt's repository.

High scores and best level live in `data/save/records.json`; mute preferences live in `data/config/shared/preferences.json`. Writes are atomic. The game honors Basalt directory variables and otherwise finds the install root from its executable path, including when launched from another working directory. Runtime never writes under `app/`. Corrupt and unsupported saves are preserved.

No original ROM is distributed or required. Original implementation and cover artwork are MIT licensed; historical game artwork and trademarks remain their owners' property.
