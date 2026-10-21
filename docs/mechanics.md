# Mechanics and fidelity

The simulation is native Rust. It neither executes the ROM nor embeds an emulator. The reference is the supplied NTSC Atari 2600 River Raid image, canonical SHA-256 `4c6842b8af64fc75e599b773821b3e6bee8e5028f5d63414821b0d63f0b2dfe9`, and Thomas Jentzsch's annotated disassembly. The reference assembly was assembled independently and matched all 4096 ROM bytes. The user's 16 KiB file consists of four identical copies of that cartridge image. No ROM is distributed.

## Verified reconstruction

`river.rs` reconstructs the 16-bit shift register seeded `A814`, including the feedback from high-byte bits 7 and 4. Each section has 16 logical blocks, each with two 32-scanline parts. The playable 160-line field therefore shows about five parts at once; a complete section spans 1024 scanlines. Odd difficulty levels are straight, and even levels generate the source's changing banks and islands. Early valley limits end at level 5. Level 48 cycles back to 47. Terrain is mirrored in 4-pixel playfield cells, with the source's two-scanline vertical updates. Roads occupy 13 pairs of scanlines, and bridges span 32 pixels.

The river masks are synthesized from the recovered geometric rules and bank-pointer table. An offline comparison against the reference matched every terrain-table byte used by generation. More substantially, the test `ten_sections_match_original_generator_runtime` checks 289 consecutive generated parts across levels 1 through 10 against actual MAME RAM observations: RNG, section and part counters, island state, difficulty, current and previous patterns, both 16-byte playfield masks, road and color flags, object kind, position, and reflection. Fourteen plane positions are observed one movement update after generation; that explicitly documented one-pixel capture phase is accounted for in the test. The independently captured first-level fixture checks 17 generations without trainer instrumentation.

The long reference capture used a separately hashed source-derived trainer: seven bytes disabled player collision deaths and fuel loss while leaving generator instructions unchanged. Scroll speed was forced to its normal maximum. This allows inspection of later rivers without mistaking death and restart behavior for generation. Its provenance and observations are in `docs/evidence/generator-vectors.json`.

A separate unmodified-ROM capture supplies 124 consecutive input frames. `movement_and_fuel_match_124_original_rom_frames` runs the full native simulation with collisions enabled and matches jet position, horizontal fraction and acceleration, vertical speed, fuel, scroll offset, and scroll fraction on every frame. It also reproduces the subsequent helicopter collision on frame 285. This catches the original immediate first rightward movement, reversal carry behavior, maximum acceleration 248, and the two-line object positioning offset.

`game.rs` implements single-player keyboard play, horizontal fractional acceleration, immediate stopping when steering is released, vertical speed control, one guided missile at a time, sprite-mask collisions, refueling, destructive collisions, scoring, extra lives, bridge checkpoints, and deterministic respawns. Rendering and collision tests use the same recovered sprite masks. The initial playable view starts after the cartridge's 158-pixel runway scroll.

Fuel starts at 65535 units, drains 32 units per active tick, and gains 512 units during a depot overlap before normal drain. This gives just over 34 seconds from a full tank at 60 ticks/second. Cruise speed is 128, with 64 as the minimum and 254 as the maximum; the original scroll loop performs two fractional updates each tick, producing about 0.5, 1, or 2 scanlines per tick. A missile advances 6 pixels per tick and is 8 pixels tall. Holding fire launches another once the current missile disappears. The default corresponds to the cartridge's guided-missile difficulty setting.

| Target | Points |
| --- | ---: |
| Ship | 30 |
| Helicopter | 60 |
| Jet | 100 |
| Fuel depot | 80 |
| Bridge | 500 |
| House | 0 |

A bonus life is awarded when the score crosses each 10,000-point boundary, up to nine lives. A score beyond 999,990 ends the game, matching the cartridge's score limit. Fuel depots are safe to overlap and can also be destroyed. Ships and helicopters remain stationary on level 1, then begin patrols on deterministic 16-frame decisions; planes enter from level 3 and move every frame. Bridges establish the next section's saved generator seed for a fresh runway and full tank after a lost life.

## Intentional presentation changes and remaining limits

The app starts directly at a playable ready state when requested, and uses a short death pause followed by automatic checkpoint restart. It does not reproduce the original console reset/select sequence, two-player state swapping, inactive-screen color cycling, difficulty switches, or exact death/intro countdown. Keyboard controls replace the joystick. Menus, pause, persistent high scores, save placement, and modern window behavior are native additions.

Terrain and fresh object generation have runtime conformance evidence. Complete gameplay is not claimed to be cycle-identical to TIA hardware. Pixel collisions use shared software masks rather than the TIA's scanline collision latches, and immediate ordering at a scrolling edge can differ. Patrol starts are deterministic but their initial random phase differs from a cartridge left waiting at the ready screen; shore turns use sprite extents. Checkpoint selection is based on bridge destruction and its recorded section seed rather than every original color/sectionEnd branch during a simultaneous death. Sound is synthesized in native Rust and played through host ALSA; it is not a cycle-exact TIA audio emulation. The fixed simulation rate is 60 Hz rather than the exact NTSC television frequency. These differences are scoped separately from the tested generator equivalence.
