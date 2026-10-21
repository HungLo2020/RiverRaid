//! Source-derived NTSC sprite masks shared by simulation and rendering.
//! Each 32-scanline object block starts with eight blank positioning lines,
//! followed by twelve interleaved A/B pairs from the cartridge bitmap tables.
use crate::game::EntityKind;

const JET: [u8; 18] = [
    0, 0, 0, 0, 0, 0x2a, 0x3e, 0x1c, 8, 0x49, 0x6b, 0x7f, 0x7f, 0x3e, 0x1c, 8, 8, 8,
];
const JET_BANK: [u8; 18] = [
    0, 0, 0, 0, 2, 0x2e, 0x3c, 0x18, 8, 0x0a, 0x2e, 0x3e, 0x3e, 0x3c, 0x18, 8, 8, 8,
];
pub(crate) const JET_EXPLOSION: [u8; 18] = [
    0, 0, 0, 0, 0, 2, 8, 0x10, 0, 0x40, 8, 0x21, 0x44, 0x10, 4, 8, 0, 0,
];
// Include shared bytes after the source labels: this is how the cartridge reads
// the tables, and matters particularly for the fuel depot and bridge.
const FUEL_A: [u8; 12] = [
    0xfe, 0xde, 0xde, 0xfe, 0xde, 0xde, 0xfe, 0xd6, 0xd6, 0xde, 0xce, 0xc6,
];
const FUEL_B: [u8; 12] = [
    0xc6, 0xde, 0xde, 0xc6, 0xce, 0xc6, 0xc6, 0xd6, 0xfe, 0xde, 0xde, 0x7c,
];
const BRIDGE_A: [u8; 12] = [
    0x42, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x42,
];
const BRIDGE_B: [u8; 12] = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x42, 0,
];
const SHIP_A: [u8; 12] = [0, 0, 0, 0x7c, 0xfe, 0x78, 0x10, 0, 0, 0, 0, 0];
const SHIP_B: [u8; 12] = [0, 0, 0, 0xfc, 0xff, 0x30, 0x10, 0, 0, 0, 0, 0];
const PLANE_A: [u8; 12] = [0, 0, 0, 0, 0, 0x30, 0x4f, 0xc6, 0, 0, 0, 0];
const PLANE_B: [u8; 12] = [0, 0, 0, 0, 0, 0x38, 0xff, 0x80, 0, 0, 0, 0];
const HELI_A0: [u8; 12] = [0, 0, 0, 0, 4, 0xff, 0x9f, 4, 7, 0, 0, 0];
const HELI_B0: [u8; 12] = [0, 0, 0, 0x0e, 0x8e, 0xff, 0x0e, 0x1c, 0, 0, 0, 0];
const HELI_A1: [u8; 12] = [0, 0, 0, 0, 4, 0xff, 0x9f, 4, 0x1c, 0, 0, 0];
const HELI_B1: [u8; 12] = [0, 0, 0, 0x0e, 0x8e, 0xff, 0x0e, 7, 0, 0, 0, 0];
const HOUSE_A: [u8; 12] = [0, 4, 0x0e, 0x1f, 0x0e, 4, 0, 0xfe, 0xaa, 0xfe, 0x38, 0];
const HOUSE_B: [u8; 12] = [0, 4, 0x1f, 0x0e, 4, 4, 0, 0xaa, 0xfe, 0x7c, 0, 0];
pub(crate) const EXPLOSION_A: [u8; 12] = [0, 0, 0, 4, 0x88, 0x10, 4, 0x80, 0x10, 0, 0, 0];
pub(crate) const EXPLOSION_B: [u8; 12] = [0, 0, 0, 0x20, 2, 0x41, 0x20, 2, 4, 0, 0, 0];
pub(crate) fn tables(
    kind: EntityKind,
    frame: u64,
) -> (&'static [u8; 12], &'static [u8; 12], usize) {
    match kind {
        EntityKind::Ship => (&SHIP_A, &SHIP_B, 2),
        EntityKind::Helicopter if (frame / 2) % 2 == 0 => (&HELI_A0, &HELI_B0, 1),
        EntityKind::Helicopter => (&HELI_A1, &HELI_B1, 1),
        EntityKind::Plane => (&PLANE_A, &PLANE_B, 1),
        EntityKind::Fuel => (&FUEL_A, &FUEL_B, 1),
        EntityKind::House => (&HOUSE_A, &HOUSE_B, 2),
        EntityKind::Bridge => (&BRIDGE_A, &BRIDGE_B, 4),
    }
}

pub(crate) fn bitmap_pixel(
    a: &[u8; 12],
    b: &[u8; 12],
    x: usize,
    y: usize,
    scale: usize,
    direction: i8,
) -> bool {
    if x >= 8 * scale || !(8..32).contains(&y) {
        return false;
    }
    let row = 11 - (y - 8) / 2;
    let column = if direction < 0 {
        7 - x / scale
    } else {
        x / scale
    };
    // GRP0 writes latch the delayed GRP1 value: B is displayed before A.
    let byte = if y % 2 == 0 { b[row] } else { a[row] };
    byte & (0x80 >> column) != 0
}

/// Object-local pixel occupancy, shared with the collision simulation.
pub fn sprite_pixel(kind: EntityKind, x: usize, y: usize, frame: u64, direction: i8) -> bool {
    let (a, b, scale) = tables(kind, frame);
    bitmap_pixel(a, b, x, y, scale, direction)
}

/// Jet-local pixel occupancy. Banking reflects the source's alternate bitmap.
pub fn player_pixel(x: usize, y: usize, bank: i8) -> bool {
    if x >= 8 || y == 0 || y >= 19 || (bank < 0 && x == 0) {
        return false;
    }
    let bits = if bank == 0 {
        JET[18 - y]
    } else {
        JET_BANK[18 - y]
    };
    // The cartridge increments playerX before positioning a reflected jet,
    // keeping its nose fixed while the banking bitmap reverses.
    let column = if bank < 0 { 8 - x } else { x };
    bits & (0x80 >> column) != 0
}
