//! Opaque, deterministic Atari-resolution software rendering.
//!
//! Bitmap rows and palette indices follow the NTSC cartridge tables documented in
//! Thomas Jentzsch's River Raid disassembly. The game kernel spends eight blank
//! scanlines positioning each object, then interleaves twelve A/B bitmap pairs.
use crate::game::{EntityKind, Game, Mode};
use crate::sprites::{EXPLOSION_A, EXPLOSION_B, JET_EXPLOSION, bitmap_pixel, tables};
pub use crate::sprites::{player_pixel, sprite_pixel};

pub const WIDTH: usize = 160;
pub const HEIGHT: usize = 210;
pub const SCENE_HEIGHT: usize = 160;

const BLACK: u32 = 0xff00_0000;
const WATER: u32 = 0xff33_4dbb;
const BANK: u32 = 0xff35_5f18;
const BRIGHT_BANK: u32 = 0xff5d_9518;
const GOLD: u32 = 0xfff6_e577;
const HUD: u32 = 0xff77_7777;
const WHITE: u32 = 0xffde_dede;

const FUEL_GAUGE: [[u8; 15]; 5] = [
    [
        0x7f, 0x40, 0x4f, 0x48, 0x48, 0x4e, 0x48, 0x48, 0x4f, 0x40, 0x40, 0x4c, 0x4c, 0x4c, 0x7f,
    ],
    [0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [
        0xff, 0, 3, 0xc2, 0x63, 0x30, 0x1b, 0xec, 0x46, 0x43, 0xc1, 0x48, 8, 8, 0xff,
    ],
    [
        0xff, 0, 0x80, 0, 0x80, 0x80, 0x80, 0, 0, 0, 0x80, 0, 0, 0, 0xff,
    ],
    [
        0xff, 1, 0x81, 0x81, 0x81, 0xe1, 0x81, 0x81, 0xf1, 1, 1, 0x19, 0x19, 0x19, 0xff,
    ],
];
const COPYRIGHT: [[u8; 8]; 5] = [
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0xad, 0xa9, 0xe9, 0xa9, 0xed, 0x41, 0x0f, 0],
    [0x50, 0x58, 0x5c, 0x56, 0x53, 0x11, 0xf0, 0],
    [0xba, 0x8a, 0xba, 0xa2, 0x3a, 0x80, 0xfe, 0],
    [0xe9, 0xab, 0xaf, 0xad, 0xe9, 0, 0, 0],
];

fn color(index: u8) -> u32 {
    match index {
        0x00 => BLACK,
        0x04 => 0xff53_5353,
        0x06 => HUD,
        0x08 => 0xff9a_9a9a,
        0x0c => WHITE,
        0x10 => 0xff21_0b00,
        0x12 => 0xff46_2700,
        0x14 => 0xff66_4300,
        0x18 => 0xffbb_a34a,
        0x1c => GOLD,
        0x1e => 0xffff_ff9a,
        0x20 => 0xff3f_0000,
        0x2a => 0xffeb_b467,
        0x32 => 0xff6f_1200,
        0x48 => 0xffd7_7a99,
        0x80 => 0xff00_0075,
        0x84 => WATER,
        0x8c => 0xff9a_a9ff,
        0x9c => 0xff5a_b2c4,
        0xa8 => 0xffa4_d965,
        0xac => 0xffc4_f58a,
        0xb0 => 0xff00_2900,
        0xda => 0xffb2_d766,
        _ => GOLD,
    }
}

fn object_color(kind: EntityKind, y: usize) -> u32 {
    let i = 11 - (y - 8) / 2;
    let palette = match kind {
        EntityKind::Fuel => [
            0x0c, 0x0c, 0x0c, 0x48, 0x48, 0x48, 0x0c, 0x0c, 0x0c, 0x48, 0x48, 0x48,
        ][i],
        EntityKind::Bridge => [
            0x20, 0x14, 0x12, 0x14, 0x12, 0x18, 0x12, 0x14, 0x12, 0x14, 0x20, 0x84,
        ][i],
        EntityKind::Ship => [0, 0, 0, 0xa8, 0x32, 0, 0, 0, 0, 0, 0, 0][i],
        EntityKind::Plane => [0, 0, 0, 0, 0, 0xac, 0x9c, 0x8c, 0, 0, 0, 0][i],
        EntityKind::Helicopter => [0, 0, 0, 0xb0, 0xb0, 0x80, 0xb0, 0x2a, 0x2a, 0, 0, 0][i],
        EntityKind::House => [0, 0x10, 0xda, 0xda, 0xda, 0xda, 0, 0x0c, 0x0c, 0, 0, 0][i],
    };
    color(palette)
}

pub struct Renderer {
    pub pixels: Vec<u32>,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            pixels: vec![BLACK; WIDTH * HEIGHT],
        }
    }

    pub fn render(&mut self, game: &Game, highscore: u64) -> &[u32] {
        self.pixels.fill(BLACK);
        let water = if game.bridge_flash & 1 != 0 {
            0xff7a_264d
        } else {
            WATER
        };
        for y in 0..SCENE_HEIGHT {
            let world_y = game.scroll + (SCENE_HEIGHT - y) as f32;
            let row = game.river.row(world_y);
            let bank = if row.bright { BRIGHT_BANK } else { BANK };
            let road_color = if row.road {
                let phase = world_y.floor() as usize % 32 / 2;
                color([4, 4, 8, 8, 8, 8, 0x1c, 8, 8, 8, 8, 4, 4, 4, 4, 4][phase])
            } else {
                bank
            };
            for x in 0..WIDTH {
                self.pixels[y * WIDTH + x] = if game.river.is_land(x as i32, world_y) {
                    road_color
                } else {
                    water
                };
            }
        }
        for entity in &game.entities {
            if !entity.alive && entity.explosion_ticks == 0 {
                continue;
            }
            let x0 = entity.x.round() as i32;
            let y0 = game.screen_y(entity.world_y).round() as i32;
            let (_, _, scale) = tables(entity.kind, game.frame);
            for y in 8..32 {
                for x in 0..8 * scale {
                    let lit = if entity.explosion_ticks > 0 {
                        bitmap_pixel(&EXPLOSION_A, &EXPLOSION_B, x, y, scale, entity.direction)
                    } else {
                        sprite_pixel(entity.kind, x, y, game.frame, entity.direction)
                    };
                    if lit {
                        let tint = if entity.explosion_ticks > 0 {
                            GOLD
                        } else {
                            object_color(entity.kind, y)
                        };
                        self.put_scene(x0 + x as i32, y0 + y as i32, tint);
                    }
                }
            }
        }
        if let Some(bullet) = &game.bullet {
            for y in 0..8 {
                self.put_scene(bullet.x.round() as i32, bullet.y.round() as i32 + y, GOLD);
            }
        }
        if !matches!(game.mode, Mode::Title | Mode::GameOver) {
            for y in 0..19 {
                for x in 0..8 {
                    let lit = if matches!(game.mode, Mode::Exploding(_)) {
                        y != 0 && JET_EXPLOSION[18 - y] & (0x80 >> x) != 0
                    } else {
                        player_pixel(x, y, game.bank)
                    };
                    if lit {
                        self.put_scene(
                            game.player_x.round() as i32 + x as i32,
                            game.player_y().round() as i32 + y as i32,
                            GOLD,
                        );
                    }
                }
            }
        }
        self.hud(game);
        match game.mode {
            Mode::Title => self.overlay(
                "RIVER RAID",
                &[
                    "ENTER / SPACE TO FLY",
                    "ARROWS / WASD MOVE",
                    "SPACE FIRES",
                    "ESC PAUSES F1 HELP",
                    &format!("BEST {highscore:06}"),
                ],
            ),
            Mode::Paused => self.overlay(
                "PAUSED",
                &["ESC / P TO RESUME", "F1 FOR CONTROLS", "Q TO QUIT"],
            ),
            Mode::GameOver => self.overlay(
                "GAME OVER",
                &[
                    &format!("SCORE {:06}", game.score),
                    &format!("BEST {highscore:06}"),
                    "ENTER TO FLY AGAIN",
                    "Q TO QUIT",
                ],
            ),
            _ => {}
        }
        &self.pixels
    }

    fn hud(&mut self, game: &Game) {
        self.rect(0, 161, 160, 38, HUD);
        let score = format!("{:>6}", game.score.min(999_999));
        for (i, digit) in score.chars().enumerate() {
            if let Some(n) = digit.to_digit(10) {
                for y in 0..8 {
                    let bits = DIGITS[n as usize][7 - y];
                    for x in 0..8 {
                        if bits & (0x80 >> x) != 0 {
                            self.put(56 + i as i32 * 8 + x, 162 + y as i32, GOLD);
                        }
                    }
                }
            }
        }
        // Five interleaved hardware-player copies form the original gauge.
        for (column, rows) in FUEL_GAUGE.iter().enumerate() {
            for y in 0..15 {
                for x in 0..8 {
                    if rows[14 - y] & (0x80 >> x) != 0 {
                        self.put(64 + column as i32 * 8 + x, 172 + y as i32, BLACK);
                    }
                }
            }
        }
        let needle = 69 + i32::from(game.fuel >> 11);
        self.rect(needle, 176, 2, 10, color(0x1e));
        for y in 0..8 {
            for x in 0..8 {
                if DIGITS[usize::from(game.lives.min(9))][7 - y] & (0x80 >> x) != 0 {
                    self.put(56 + x, 189 + y as i32, GOLD);
                }
            }
            for (column, rows) in COPYRIGHT.iter().enumerate() {
                for x in 0..8 {
                    if rows[7 - y] & (0x80 >> x) != 0 {
                        self.put(64 + column as i32 * 8 + x, 189 + y as i32, GOLD);
                    }
                }
            }
        }
    }

    pub fn overlay(&mut self, title: &str, lines: &[&str]) {
        let height = 26 + lines.len() as i32 * 12;
        let top = (SCENE_HEIGHT as i32 - height) / 2;
        self.rect(6, top, 148, height, 0xff10_192c);
        self.rect(6, top, 148, 2, GOLD);
        self.text_center(title, top + 9, GOLD);
        for (i, line) in lines.iter().enumerate() {
            self.text_center(line, top + 24 + i as i32 * 12, WHITE);
        }
    }

    fn put_scene(&mut self, x: i32, y: i32, tint: u32) {
        if (0..SCENE_HEIGHT as i32).contains(&y) {
            self.put(x, y, tint);
        }
    }
    fn put(&mut self, x: i32, y: i32, tint: u32) {
        if (0..WIDTH as i32).contains(&x) && (0..HEIGHT as i32).contains(&y) {
            self.pixels[y as usize * WIDTH + x as usize] = tint;
        }
    }
    fn rect(&mut self, x: i32, y: i32, width: i32, height: i32, tint: u32) {
        for yy in y..y + height {
            for xx in x..x + width {
                self.put(xx, yy, tint);
            }
        }
    }
    fn text_center(&mut self, value: &str, y: i32, tint: u32) {
        self.text(
            value,
            (WIDTH as i32 - value.len() as i32 * 6 + 1) / 2,
            y,
            tint,
        );
    }
    fn text(&mut self, value: &str, x: i32, y: i32, tint: u32) {
        for (i, letter) in value.to_uppercase().chars().enumerate() {
            let glyph = glyph(letter);
            for (yy, row) in glyph.iter().enumerate() {
                for xx in 0..5 {
                    if row & (1 << (4 - xx)) != 0 {
                        self.put(x + i as i32 * 6 + xx, y + yy as i32, tint);
                    }
                }
            }
        }
    }
}

const DIGITS: [[u8; 8]; 10] = [
    [0x3c, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3c],
    [0x3c, 0x18, 0x18, 0x18, 0x18, 0x18, 0x38, 0x18],
    [0x7e, 0x60, 0x60, 0x3c, 0x06, 0x06, 0x46, 0x3c],
    [0x3c, 0x46, 0x06, 0x0c, 0x0c, 0x06, 0x46, 0x3c],
    [0x0c, 0x0c, 0x0c, 0x7e, 0x4c, 0x2c, 0x1c, 0x0c],
    [0x7c, 0x46, 0x06, 0x06, 0x7c, 0x60, 0x60, 0x7e],
    [0x3c, 0x66, 0x66, 0x66, 0x7c, 0x60, 0x62, 0x3c],
    [0x18, 0x18, 0x18, 0x18, 0x0c, 0x06, 0x42, 0x7e],
    [0x3c, 0x66, 0x66, 0x3c, 0x3c, 0x66, 0x66, 0x3c],
    [0x3c, 0x46, 0x06, 0x3e, 0x66, 0x66, 0x66, 0x3c],
];

fn glyph(letter: char) -> [u8; 7] {
    match letter {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 2, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '/' => [1, 1, 2, 4, 8, 16, 16],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        ':' => [0, 4, 4, 0, 4, 4, 0],
        '.' => [0, 0, 0, 0, 0, 4, 4],
        _ => [0; 7],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprite_padding_and_hardware_widths_are_preserved() {
        for kind in [
            EntityKind::Ship,
            EntityKind::Helicopter,
            EntityKind::Plane,
            EntityKind::Fuel,
            EntityKind::House,
            EntityKind::Bridge,
        ] {
            for y in 0..8 {
                for x in 0..32 {
                    assert!(!sprite_pixel(kind, x, y, 0, 1));
                }
            }
            for y in 0..32 {
                assert!(!sprite_pixel(kind, 32, y, 0, 1));
            }
        }
        assert!(sprite_pixel(EntityKind::Fuel, 1, 8, 0, 1));
        assert!(sprite_pixel(EntityKind::Bridge, 4, 9, 0, 1));
        assert!(player_pixel(4, 1, 0));
        assert!(!player_pixel(0, 0, 0));
        assert!(player_pixel(4, 1, -1));
        assert!(player_pixel(4, 1, 1));
        assert!(!player_pixel(3, 1, -1));
    }

    #[test]
    fn reflection_preserves_bitmap_occupancy() {
        for y in 0..32 {
            for x in 0..16 {
                assert_eq!(
                    sprite_pixel(EntityKind::Ship, x, y, 0, 1),
                    sprite_pixel(EntityKind::Ship, 15 - x, y, 0, -1)
                );
            }
        }
    }

    #[test]
    fn frames_and_overlays_always_have_opaque_alpha() {
        let mut renderer = Renderer::new();
        let game = Game::new();
        assert_eq!(renderer.render(&game, 0).len(), WIDTH * HEIGHT);
        renderer.overlay("HELP", &["ARROWS MOVE", "SPACE FIRES"]);
        assert!(renderer.pixels.iter().all(|pixel| pixel >> 24 == 255));
    }

    #[test]
    fn fuel_rows_match_the_captured_cartridge_frame() {
        // Independently sampled from the isolated Stella reference capture.
        let captured = [
            0x7c, 0xc6, 0xde, 0xce, 0xde, 0xde, 0xfe, 0xd6, 0xd6, 0xd6, 0xc6, 0xfe, 0xc6, 0xde,
            0xce, 0xde, 0xc6, 0xfe, 0xde, 0xde, 0xde, 0xde, 0xc6, 0xfe,
        ];
        for (y, expected) in captured.iter().enumerate() {
            let actual = (0..8).fold(0_u8, |row, x| {
                row | if sprite_pixel(EntityKind::Fuel, x, y + 8, 0, 1) {
                    0x80 >> x
                } else {
                    0
                }
            });
            assert_eq!(actual, *expected, "fuel row {y}");
        }
    }

    #[test]
    fn jet_rows_match_the_captured_cartridge_frame() {
        // Frame 0002: first visible jet pixel is main-kernel scanline 142,
        // one line below its nominal 141 origin (JET_Y is an exclusive limit).
        let captured = [
            0, 8, 8, 8, 0x1c, 0x3e, 0x7f, 0x7f, 0x6b, 0x49, 8, 0x1c, 0x3e, 0x2a, 0, 0, 0, 0, 0,
        ];
        for (y, expected) in captured.iter().enumerate() {
            let actual = (0..8).fold(0_u8, |row, x| {
                row | if player_pixel(x, y, 0) { 0x80 >> x } else { 0 }
            });
            assert_eq!(actual, *expected, "jet row {y}");
        }
    }
}
