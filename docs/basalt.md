# Basalt release integration

The release targets [Basalt Game Standard version 1](https://github.com/HungLo2020/Basalt/blob/516209fc8489779cb322cc9b641a1487beff4ec3/STANDARDS.md),
reviewed at revision `516209fc8489779cb322cc9b641a1487beff4ec3`, using catalog schema 2.
The Basalt repository is not modified.

The installed Basalt CLI successfully downloaded [release v0.1.0](https://github.com/HungLo2020/RiverRaid/releases/tag/v0.1.0)
into a temporary library with spaces in its path. Manual backup and restore recovered the exact
save bytes; reinstall preserved them, and clearing cache preserved saves. The installed release
also completed a 12,000-frame headless run. All launcher configuration, data, and cache for this
test used isolated temporary XDG directories. See [installation evidence](evidence/basalt-install.json).

Build and package on Linux x64 with Rust and Python 3.11 or newer:

```sh
cargo build --locked --release
python3 tools/package.py
python3 tools/validate_package.py dist/RiverRaid-0.1.0-linux-x64.zip
```

The filename uses `package.version` from `Cargo.toml`; a release tag must match that version.
The ZIP contains only these files underneath `app/`:

```text
app/bin/river-raid
app/README.md
app/LICENSE
app/basalt/cover.png
```

Optional original `assets/icon.png` or `assets/icon.svg` is also included when present.
Executable permissions are recorded explicitly, ZIP timestamps are fixed unless
`SOURCE_DATE_EPOCH` is set, and the packager validates the temporary archive before replacing
the final output. Validation rejects paths outside `app/`, traversal, duplicate entries,
symlinks, file/directory collisions, missing execute permission, missing README/license/cover,
invalid cover dimensions or transparency, and corrupt ZIP members. No ROM, user data, updater,
or development tools ship in the archive.

`packaging/RiverRaid.json` is the proposed Linux catalog entry. The installed Basalt CLI
accepted it from a temporary isolated XDG catalog and exposed all seven standard actions;
see [catalog validation](evidence/basalt-catalog.json). It retains Basalt's standard
layout and maintenance actions. To use it, copy it into a Basalt user's catalog after a release
is available:

```sh
mkdir -p ~/.config/basalt/games
cp packaging/RiverRaid.json ~/.config/basalt/games/
basalt catalog
basalt install RiverRaid --library default
basalt actions RiverRaid
```

Basalt launches `app/bin/river-raid` with no arguments from the install root. Runtime paths
honor `BASALT_GAME_DIR`, `BASALT_SAVE_DIR`, `BASALT_CONFIG_DIR`, `BASALT_SHARED_CONFIG_DIR`, and
`BASALT_CACHE_DIR`. Without launcher variables, the executable's `app/bin/` location determines
the install root, independently of the current working directory. Missing data directories
are created. Save records belong in `data/save/records.json`; portable preferences belong in
`data/config/shared/preferences.json`. Writes use temporary files in the same directory and
atomic replacement. The application does not write into `app/`.

The executable uses the host's X11 libraries and ALSA `libasound.so.2`. Development headers
are needed only to build; audio-device failure leaves the game playable without sound. The
release starts without terminal input, a ROM, or network access. Controller support is outside
this release's scope. Linux x64 is the only release platform currently built and supported.
Ubuntu 22.04 CI provides a conservative glibc baseline; other desktop distributions need
individual compatibility verification.

CI checks formatting, Clippy, Rust tests, Python packaging tests, a release build, and native
window behavior before producing the ZIP. The native smoke test always starts its own private
Xvfb server with `-displayfd`, clears inherited launcher variables, and gives every X client
an explicit private `DISPLAY`. It identifies the game window by PID before addressing input.
It never uses the host desktop or sends global keyboard input. The binary is copied into a
read-only `app/bin/` under a temporary install root and launched from an unrelated working
directory to exercise standalone data placement. The test verifies the application class,
normal window type, full opacity, opaque region updates after resize, help/pause/resume, and
normal keyboard quit with save creation. Run it with:

```sh
python3 tools/window_smoke.py --binary target/release/river-raid
```

Pushing a `v<version>` tag repeats validation and publishes the predictable ZIP as a GitHub
Release asset. A manually triggered release workflow builds an artifact without publishing.
The catalog's cover URL references the same original, opaque, 3:4 cover shipped in the release.
