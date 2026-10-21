//! Deterministic river construction recovered from the NTSC cartridge's generator.
//! This is a typed reconstruction of its rules, not a 6502 interpreter.
use crate::game::EntityKind;

pub const BLOCK_HEIGHT: usize = 32;
pub const SECTION_HEIGHT: usize = 16 * 2 * BLOCK_HEIGHT;
const BANK_POINTERS: [usize; 15] = [
    0, 161, 145, 127, 107, 85, 61, 35, 7, 169, 144, 117, 88, 57, 24,
];
const HOUSE_POSITIONS: [u8; 15] = [
    143, 141, 7, 10, 132, 13, 128, 18, 124, 22, 120, 26, 116, 30, 112,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lfsr(pub u16);
impl Default for Lfsr {
    fn default() -> Self {
        Self(0xa814)
    }
}
impl Lfsr {
    pub fn advance(&mut self) -> u16 {
        let hi = (self.0 >> 8) as u8;
        self.0 = (self.0 << 1) | (((hi >> 7) ^ (hi >> 4)) & 1) as u16;
        self.0
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Spawn {
    pub kind: EntityKind,
    pub x: f32,
    pub direction: i8,
}
#[derive(Clone, Debug)]
pub struct Block {
    pub pattern: u8,
    pub previous_pattern: u8,
    pub seed: u16,
    pub island: bool,
    pub road: bool,
    pub bright: bool,
    pub level: u8,
    pub spawn: Option<Spawn>,
    pub pf1: [u8; 16],
    pub pf2: [u8; 16],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RiverRow {
    pub left: i32,
    pub right: i32,
    pub island: Option<(i32, i32)>,
    pub road: bool,
    pub bright: bool,
}
#[derive(Clone, Debug)]
pub struct River {
    pub blocks: Vec<Block>,
    pub random: Lfsr,
    section_block: u8,
    part: u8,
    pattern: u8,
    state: u8,
    pub level: u8,
    bright: bool,
}
impl Default for River {
    fn default() -> Self {
        Self::new()
    }
}
impl River {
    pub fn new() -> Self {
        Self::new_at(1, Lfsr::default().0)
    }
    pub fn new_at(level: u8, seed: u16) -> Self {
        let mut river = Self {
            blocks: Vec::new(),
            random: Lfsr(seed),
            section_block: 17,
            part: 1,
            pattern: 12,
            state: 0,
            level,
            bright: level % 2 == 1,
        };
        // Six work buffers are initialized to the runway by GameInit.
        for _ in 0..6 {
            river.blocks.push(Block {
                pattern: 12,
                previous_pattern: 12,
                seed,
                island: false,
                road: false,
                bright: level % 2 == 1,
                level,
                spawn: None,
                pf1: [255; 16],
                pf2: [15; 16],
            });
        }
        river.blocks[5].road = true;
        river
    }
    pub fn ensure(&mut self, world_y: f32) {
        let count = (world_y.max(0.0) as usize / BLOCK_HEIGHT) + 1;
        while self.blocks.len() < count {
            self.generate();
        }
    }
    pub fn level_at(&self, world_y: f32) -> u8 {
        self.blocks
            .get(world_y.max(0.0) as usize / BLOCK_HEIGHT)
            .map_or(1, |b| b.level)
    }
    pub fn row(&self, world_y: f32) -> RiverRow {
        let Some(block) = self.blocks.get(world_y.max(0.0) as usize / BLOCK_HEIGHT) else {
            return RiverRow {
                left: 44,
                right: 116,
                island: None,
                road: false,
                bright: self.level % 2 == 1,
            };
        };
        let row = ((world_y.max(0.0) as usize % BLOCK_HEIGHT) / 2).min(15);
        let (p1, p2) = (block.pf1[row], block.pf2[row]);
        let mut left = 16;
        while left < 80 && land_bits(p1, p2, left) {
            left += 4;
        }
        let mut island = None;
        let mut x = left;
        while x < 80 {
            if land_bits(p1, p2, x) {
                island = Some((x, 160 - x));
                break;
            }
            x += 4;
        }
        RiverRow {
            left,
            right: 160 - left,
            island,
            road: block.road && row < 13,
            bright: block.bright,
        }
    }
    pub fn is_land(&self, x: i32, world_y: f32) -> bool {
        if !(0..160).contains(&x) {
            return true;
        }
        let row = self.row(world_y);
        x < row.left || x >= row.right || row.island.is_some_and(|(l, r)| x >= l && x < r)
    }
    fn generate(&mut self) {
        let previous = self.pattern;
        self.part -= 1;
        let road;
        if self.part == 0 {
            self.section_block -= 1;
            if self.section_block == 0 {
                self.level = if self.level >= 48 { 47 } else { self.level + 1 };
                self.section_block = 16;
            }
            self.random.advance();
            self.part = 2;
            road = false;
            if self.section_block == 1 {
                self.state = 0;
                self.pattern = 12;
            } else if self.level % 2 == 1 {
                self.pattern = 7;
            } else {
                self.pattern = self.next_pattern();
            }
        } else {
            road = self.section_block == 1;
            if road {
                self.bright = self.level % 2 == 0;
            }
            self.random.advance();
        }
        let pf = make_patterns(previous, self.pattern, self.state & 128 != 0);
        let spawn = if road {
            Some(Spawn {
                kind: EntityKind::Bridge,
                x: 63.0,
                direction: 1,
            })
        } else {
            Some(self.spawn(previous))
        };
        self.blocks.push(Block {
            pattern: self.pattern,
            previous_pattern: previous,
            seed: self.random.0,
            island: self.state & 128 != 0,
            road,
            bright: self.bright,
            level: self.level,
            spawn,
            pf1: pf.0,
            pf2: pf.1,
        });
        // Keep the individual bank rows; the renderer and collision model consume identical geometry.
    }
    fn next_pattern(&mut self) -> u8 {
        let lo = self.random.0 as u8;
        if self.section_block == 2 {
            if self.state == 192 {
                self.state = 128;
                return 0;
            }
            self.state = 0;
        } else if ((self.state << 1) ^ self.state) & 128 != 0 {
            self.state = if self.state & 64 != 0 { 192 } else { 0 };
        } else if lo & 48 == 0 {
            self.state = if self.state & 128 != 0 { 128 } else { 64 };
            return 0;
        }
        let mut id = lo & 15;
        if id < 2 {
            id += 2;
        }
        id.min(if self.level < 5 {
            8
        } else if self.state & 128 != 0 {
            13
        } else {
            14
        })
    }
    fn spawn(&self, previous: u8) -> Spawn {
        let lo = self.random.0 as u8;
        let hi = (self.random.0 >> 8) as u8;
        let margin = if self.level < 5 { 6 } else { 0 };
        let mut kind = if self.section_block + self.part >= 18
            || (hi <= (63 - self.level) * 2 && lo & 64 != 0)
        {
            EntityKind::House
        } else if hi <= (63 - self.level) * 2 {
            EntityKind::Fuel
        } else {
            match hi & if self.level < 3 { 1 } else { 7 } {
                0 | 2 | 5 => EntityKind::Ship,
                4 => EntityKind::Plane,
                _ => EntityKind::Helicopter,
            }
        };
        let mut right = lo & 128 == 0;
        let x: u8;
        if kind == EntityKind::House {
            if self.level % 2 == 1 {
                let pos = (lo & 31) + 9;
                x = if pos < 25 { pos } else { pos + 93 };
            } else {
                let id = self.pattern.min(previous);
                x = if self.state & 128 != 0 && id != 0 {
                    71
                } else {
                    HOUSE_POSITIONS[id as usize]
                };
            }
        } else if self.pattern == previous && self.level % 2 == 1 {
            x = ((lo & 63) + 46 + margin).min(if kind == EntityKind::Ship {
                97 - margin
            } else {
                106 - margin
            });
        } else {
            let max_id = self.pattern.max(previous);
            if kind == EntityKind::Ship && max_id > if self.state & 128 != 0 { 10 } else { 13 } {
                kind = EntityKind::Helicopter;
            }
            let base = max_id * 4;
            if self.state & 128 != 0 && base != 0 {
                if lo & 128 == 0 {
                    x = (80 - base)
                        .wrapping_sub(if kind == EntityKind::Fuel { 9 } else { 10 })
                        .wrapping_sub(margin)
                        .wrapping_sub(if kind == EntityKind::Ship { 10 } else { 0 });
                } else {
                    x = base + 82 + margin;
                }
            } else if lo & 128 == 0 {
                x = base + 18 + margin;
            } else {
                x = (144 - base)
                    .wrapping_sub(if kind == EntityKind::Fuel { 9 } else { 10 })
                    .wrapping_sub(margin)
                    .wrapping_sub(if kind == EntityKind::Ship { 10 } else { 0 });
            }
            right = if self.state & 128 != 0 && base != 0 {
                lo & 128 != 0
            } else {
                lo & 128 == 0
            };
        }
        Spawn {
            kind,
            x: x as f32,
            direction: if kind == EntityKind::Fuel || right {
                1
            } else {
                -1
            },
        }
    }
}
fn land_bits(p1: u8, p2: u8, x: i32) -> bool {
    if x < 16 {
        true
    } else if x < 48 {
        p1 & (128 >> ((x - 16) / 4)) != 0
    } else {
        p2 & (1 << ((x - 48) / 4)) != 0
    }
}
fn get_page(id: u8) -> usize {
    usize::from(id != 0 && id < 9)
}
fn make_patterns(previous: u8, new: u8, island: bool) -> ([u8; 16], [u8; 16]) {
    let mut difference = previous as i16 - new as i16;
    if difference < 0 {
        difference += 1;
    } else if difference > 0 {
        difference -= 1;
    }
    let (mut a, mut b);
    if previous < new && previous < 8 && new >= 9 {
        a = read_pattern(1, previous as isize); // PF1 is a constant low-byte pointer in this crossing case.
        b = read_pattern(
            get_page(new),
            BANK_POINTERS[new as usize] as isize + difference as isize,
        );
    } else if previous >= new && previous < 9 || previous < new && previous < 8 && new < 9 {
        let id = if previous < new { new } else { previous };
        a = read_pattern(
            get_page(id),
            BANK_POINTERS[id as usize] as isize + difference as isize,
        );
        b = [0; 16];
    } else {
        let first = if previous >= 9 && new < 8 {
            14 - new
        } else {
            7
        };
        a = read_pattern(1, first as isize);
        let id = if previous < new { new } else { previous };
        b = read_pattern(
            get_page(id),
            BANK_POINTERS[id as usize] as isize + difference as isize,
        );
    }
    if island {
        std::mem::swap(&mut a, &mut b);
    }
    (a, b)
}
fn read_pattern(page: usize, offset: isize) -> [u8; 16] {
    let mut result = [0; 16];
    for (i, value) in result.iter_mut().enumerate() {
        *value = pattern_byte(page, (offset + i as isize).rem_euclid(256) as usize);
    }
    result
}
fn pattern_byte(page: usize, offset: usize) -> u8 {
    if page == 1 {
        if offset < 7 {
            return 255u8 << (7 - offset);
        }
        for id in (1..=8usize).rev() {
            let start = BANK_POINTERS[id];
            let relative = offset as isize - start as isize;
            if (0..16).contains(&relative) {
                return 255u8 << (8 - id);
            }
            if relative >= 16 {
                let down = id.saturating_sub(1);
                let step = relative as usize - 16;
                let width = if step < down {
                    down - step
                } else {
                    step - down + 2
                };
                if width < id {
                    return 255u8 << (8 - width);
                }
            }
        }
    } else {
        if offset < 19 {
            return 0;
        }
        if offset < 24 {
            return ((1u16 << (offset - 18)) - 1) as u8;
        }
        for id in (9..=14usize).rev() {
            let start = BANK_POINTERS[id];
            let relative = offset as isize - start as isize;
            if (0..16).contains(&relative) {
                return ((1u16 << (id - 8)) - 1) as u8;
            }
            let next = if id > 9 { BANK_POINTERS[id - 1] } else { 256 };
            if offset >= start + 16 && offset < next {
                let step = offset - start - 16;
                let width = id - 8;
                let value = if step < width - 1 {
                    width - 1 - step
                } else if step < width + 7 {
                    0
                } else {
                    step - width - 6
                };
                return ((1u16 << value.min(8)) - 1) as u8;
            }
        }
    }
    0
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generator_matches_original_mame_ram_snapshots() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/ntsc-generator.json")).unwrap();
        let mut river = River::new();
        for (index, expected) in fixtures.as_array().unwrap().iter().enumerate() {
            river.ensure(((index + 6) * 32) as f32);
            let block = &river.blocks[index + 6];
            let number = |name: &str| expected[name].as_u64().unwrap();
            assert_eq!(block.seed as u64, number("seed"), "seed at block {index}");
            assert_eq!(block.pattern as u64, number("pattern"));
            let flags = number("flags") as usize;
            assert_eq!(
                block.pf1,
                read_pattern(flags & 1, number("pf1") as isize),
                "PF1 at block {index}"
            );
            assert_eq!(
                block.pf2,
                read_pattern((flags >> 1) & 1, number("pf2") as isize),
                "PF2 at block {index}"
            );
            let spawn = block.spawn.unwrap();
            let shape = match spawn.kind {
                EntityKind::Ship => 7,
                EntityKind::Helicopter => 5,
                EntityKind::Plane => 4,
                EntityKind::Fuel => 10,
                EntityKind::House => 9,
                EntityKind::Bridge => 8,
            };
            assert_eq!(
                shape,
                if number("shape") == 6 {
                    5
                } else {
                    number("shape")
                },
                "object at block {index}"
            );
            assert_eq!(spawn.x, number("x") as f32, "x at block {index}");
            assert_eq!(
                spawn.direction as i64,
                expected["direction"].as_i64().unwrap(),
                "direction at block {index}"
            );
        }
    }
    #[test]
    fn ten_sections_match_original_generator_runtime() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../docs/evidence/generator-vectors.json")).unwrap();
        let mut river = River::new();
        for (index, expected) in fixture["rows"].as_array().unwrap().iter().enumerate() {
            river.generate();
            let block = &river.blocks[index + 6];
            let number = |name: &str| expected[name].as_u64().unwrap();
            let context = format!("block {index} level {} rng {:04x}", block.level, block.seed);
            assert_eq!(block.seed as u64, number("rng"), "seed {context}");
            assert_eq!(block.level as u64, number("level"), "level {context}");
            assert_eq!(
                river.section_block as u64,
                number("section_block"),
                "section block {context}"
            );
            assert_eq!(river.part as u64, number("part"), "part {context}");
            assert_eq!(river.state as u64, number("state"), "state {context}");
            assert_eq!(block.pattern as u64, number("pattern"), "pattern {context}");
            assert_eq!(
                block.previous_pattern as u64,
                number("previous"),
                "previous {context}"
            );
            assert_eq!(block.road, number("flags") & 128 != 0, "road {context}");
            assert_eq!(block.bright, number("flags") & 4 != 0, "color {context}");
            let bytes = |name: &str| {
                expected[name]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u8)
                    .collect::<Vec<_>>()
            };
            assert_eq!(block.pf1.as_slice(), bytes("pf1"), "PF1 {context}");
            assert_eq!(block.pf2.as_slice(), bytes("pf2"), "PF2 {context}");
            let spawn = block.spawn.unwrap();
            let shape = match spawn.kind {
                EntityKind::Ship => 7,
                EntityKind::Helicopter => 5,
                EntityKind::Plane => 4,
                EntityKind::Fuel => 10,
                EntityKind::House => 9,
                EntityKind::Bridge => 8,
            };
            assert_eq!(
                shape,
                if number("shape") == 6 {
                    5
                } else {
                    number("shape")
                },
                "object {context}"
            );
            // MAME's frame callback observes the next update, after the new
            // plane has moved. Unlike ships/helicopters, planes move immediately
            // on every frame (source .xMoveEnemy). All14 plane samples have
            // precisely this one-pixel phase difference; normalize only those.
            assert_eq!(
                if spawn.kind == EntityKind::Plane {
                    (spawn.x + spawn.direction as f32).rem_euclid(160.0)
                } else {
                    spawn.x
                },
                number("x") as f32,
                "x {context}"
            );
            assert_eq!(
                spawn.direction < 0,
                expected["reflected"].as_bool().unwrap(),
                "direction {context}"
            );
        }
    }
    #[test]
    fn original_seed_vectors() {
        let mut r = Lfsr::default();
        assert_eq!(
            (0..8).map(|_| r.advance()).collect::<Vec<_>>(),
            [
                0x5029, 0xa053, 0x40a7, 0x814e, 0x029d, 0x053a, 0x0a74, 0x14e8
            ]
        );
    }
    #[test]
    fn section_has_two_parts_for_every_block() {
        let mut r = River::new();
        r.ensure(3000.);
        assert!(r.blocks[6..36].iter().all(|b| b.level == 1));
        assert!(r.blocks[37].road);
        assert_eq!(r.blocks[38].level, 2);
    }
    #[test]
    fn source_bank_table_geometry() {
        assert_eq!(pattern_byte(1, 7), 255);
        assert_eq!(pattern_byte(1, 35), 254);
        assert_eq!(pattern_byte(1, 161), 128);
        assert_eq!(pattern_byte(0, 24), 63);
        assert_eq!(pattern_byte(0, 88), 15);
    }
    #[test]
    fn opening_straight_has_original_banks() {
        let mut r = River::new();
        r.ensure(1000.);
        for y in 240..800 {
            let row = r.row(y as f32);
            assert_eq!((row.left, row.right), (44, 116));
        }
    }
    #[test]
    fn generated_river_is_mirror_symmetric() {
        let mut r = River::new();
        r.ensure(10000.);
        for y in 0..10000 {
            for x in 0..80 {
                assert_eq!(r.is_land(x, y as f32), r.is_land(159 - x, y as f32));
            }
        }
    }
}
