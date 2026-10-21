//! Fixed-tick, platform-independent River Raid rules.
use crate::river::River;

pub const MAX_FUEL: u16 = u16::MAX;
pub const PLAYER_Y: f32 = 141.0;
pub const TICK_RATE: f64 = 60.0;
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub fire: bool,
    pub start: bool,
    pub pause: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Title,
    Playing,
    Paused,
    Exploding(u16),
    GameOver,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Shot,
    Explosion,
    Refuel,
    FullTank,
    Bridge,
    ExtraLife,
    LowFuel,
    GameOver,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityKind {
    Ship,
    Helicopter,
    Plane,
    Fuel,
    House,
    Bridge,
}
impl EntityKind {
    pub const fn score(self) -> u32 {
        match self {
            Self::Ship => 30,
            Self::Helicopter => 60,
            Self::Plane => 100,
            Self::Fuel => 80,
            Self::Bridge => 500,
            Self::House => 0,
        }
    }
    pub const fn width(self) -> u8 {
        match self {
            Self::Ship | Self::House => 16,
            Self::Bridge => 32,
            _ => 8,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Entity {
    pub id: u64,
    pub kind: EntityKind,
    pub x: f32,
    pub world_y: f32,
    pub direction: i8,
    pub alive: bool,
    pub explosion_ticks: u8,
    pub moving: bool,
}
impl Entity {
    pub fn width(&self) -> f32 {
        self.kind.width() as f32
    }
    pub fn height(&self) -> f32 {
        32.0
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Bullet {
    pub x: f32,
    pub y: f32,
}
#[derive(Clone, Debug)]
pub struct Game {
    pub player_x: f32,
    pub scroll: f32,
    pub entities: Vec<Entity>,
    pub bullet: Option<Bullet>,
    pub score: u32,
    pub lives: u8,
    pub fuel: u16,
    pub speed: u8,
    pub level: u8,
    pub frame: u64,
    pub mode: Mode,
    pub river: River,
    pub bank: i8,
    pub bridge_flash: u8,
    horizontal_accel: u8,
    previous_right: bool,
    horizontal_fraction: u8,
    vertical_fraction: u8,
    spawned: usize,
    random: u8,
    checkpoint_level: u8,
    checkpoint_seed: u16,
    previous_pause: bool,
    previous_start: bool,
}
impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}
impl Game {
    pub fn new() -> Self {
        let mut game = Self {
            player_x: 76.0,
            scroll: 158.0,
            entities: Vec::new(),
            bullet: None,
            score: 0,
            lives: 3,
            fuel: MAX_FUEL,
            speed: 64,
            level: 1,
            frame: 0,
            mode: Mode::Title,
            river: River::new(),
            bank: 0,
            bridge_flash: 0,
            horizontal_accel: 0,
            previous_right: false,
            horizontal_fraction: 0,
            vertical_fraction: 0,
            spawned: 6,
            random: 1,
            checkpoint_level: 1,
            checkpoint_seed: crate::river::Lfsr::default().0,
            previous_pause: false,
            previous_start: false,
        };
        game.populate();
        game
    }
    pub fn start(&mut self) {
        *self = Self::new();
        self.mode = Mode::Playing;
    }
    pub const fn player_y(&self) -> f32 {
        PLAYER_Y
    }
    pub fn screen_y(&self, world_y: f32) -> f32 {
        160.0 - (world_y - self.scroll)
    }
    pub fn fuel_fraction(&self) -> f32 {
        self.fuel as f32 / MAX_FUEL as f32
    }
    pub fn tick(&mut self, input: Input) -> Vec<Event> {
        let mut events = Vec::new();
        let start = input.start && !self.previous_start;
        let pause = input.pause && !self.previous_pause;
        self.previous_start = input.start;
        self.previous_pause = input.pause;
        if matches!(self.mode, Mode::Title | Mode::GameOver) {
            if start || input.fire {
                self.start();
            }
            return events;
        }
        if pause {
            self.mode = match self.mode {
                Mode::Playing => Mode::Paused,
                Mode::Paused => Mode::Playing,
                other => other,
            };
        }
        if self.mode == Mode::Paused {
            return events;
        }
        self.frame += 1;
        self.bridge_flash = self.bridge_flash.saturating_sub(1);
        if let Mode::Exploding(remaining) = self.mode {
            if remaining > 1 {
                self.mode = Mode::Exploding(remaining - 1);
            } else if self.lives == 0 {
                self.mode = Mode::GameOver;
                events.push(Event::GameOver);
            } else {
                self.respawn();
            }
            return events;
        }
        self.animate_enemies();
        // The original reads hardware collisions before advancing its movement.
        if self.collisions(&mut events) {
            return events;
        }
        let Some(fuel) = self.fuel.checked_sub(32) else {
            self.crash(&mut events);
            return events;
        };
        self.fuel = fuel;
        if self.fuel < 0x2000 && self.frame % 32 == 0 {
            events.push(Event::LowFuel);
        }
        self.move_player(input);
        self.speed = if input.up {
            self.speed.saturating_add(2).min(254)
        } else if input.down {
            self.speed.saturating_sub(2).max(64)
        } else if self.speed < 128 {
            self.speed.saturating_add(2).min(128)
        } else if self.speed > 128 {
            self.speed.saturating_sub(2).max(128)
        } else {
            128
        };
        self.move_bullet(input.fire, &mut events);
        for _ in 0..2 {
            if self.speed >= 254 {
                self.scroll += 1.0;
            } else {
                let (next, carry) = self.vertical_fraction.overflowing_add(self.speed);
                self.vertical_fraction = next;
                if carry {
                    self.scroll += 1.0;
                }
            }
        }
        self.populate();
        self.level = self.river.level;
        self.entities.retain(|e| e.world_y >= self.scroll - 40.0);
        events
    }
    fn populate(&mut self) {
        self.river.ensure(self.scroll + 160.0);
        while self.spawned < self.river.blocks.len() {
            if let Some(spawn) = self.river.blocks[self.spawned].spawn {
                self.entities.push(Entity {
                    id: self.spawned as u64,
                    kind: spawn.kind,
                    x: spawn.x,
                    // The repositioning kernel places object scanlines two lines
                    // ahead of the nominal32line part boundary (VDELP1).
                    world_y: (self.spawned * 32 + 34) as f32,
                    direction: spawn.direction,
                    alive: true,
                    explosion_ticks: 0,
                    moving: false,
                });
            }
            self.spawned += 1;
        }
    }
    fn move_player(&mut self, input: Input) {
        let direction = match (input.left, input.right) {
            (true, false) => -1,
            (false, true) => 1,
            _ => 0,
        };
        self.bank = direction;
        if direction == 0 {
            self.horizontal_accel = 0;
            self.previous_right = false;
            self.horizontal_fraction = 0;
            return;
        }
        let Some(acceleration) = self.horizontal_accel.checked_add(8) else {
            // The cartridge leaves acceleration at248 on overflow and moves one
            // whole pixel on every subsequent held frame.
            self.player_x = (self.player_x + direction as f32).clamp(-8.0, 160.0);
            self.horizontal_fraction = if direction < 0 { 8 } else { 0 };
            self.previous_right = direction > 0;
            return;
        };
        self.horizontal_accel = acceleration;
        let (next, carry) = if direction < 0 {
            let (n, b) = self
                .horizontal_fraction
                .overflowing_sub(self.horizontal_accel);
            (n, b)
        } else {
            if self.previous_right {
                self.horizontal_fraction
            } else {
                255
            }
            .overflowing_add(self.horizontal_accel)
        };
        self.horizontal_fraction = next;
        self.previous_right = direction > 0;
        if carry {
            self.player_x += direction as f32;
        }
        // Do not wrap the jet through the opposite shore.
        self.player_x = self.player_x.clamp(-8.0, 160.0);
    }
    fn animate_enemies(&mut self) {
        self.random = (self.random << 1) | (((self.random >> 7) ^ (self.random >> 4)) & 1);
        if self.frame % 16 == 0 {
            let mut index = (self.random & 7) as usize;
            if index >= 5 {
                index -= 5;
            }
            if let Some(entity) = self
                .entities
                .iter_mut()
                .filter(|e| e.world_y >= self.scroll)
                .nth(index)
            {
                entity.moving = true;
            }
        }
        for entity in &mut self.entities {
            entity.explosion_ticks = entity.explosion_ticks.saturating_sub(1);
            if !entity.alive || self.level == 1 {
                continue;
            }
            let moving = entity.kind == EntityKind::Plane
                || (entity.moving
                    && self.frame % 2 == 0
                    && matches!(entity.kind, EntityKind::Ship | EntityKind::Helicopter));
            if !moving {
                continue;
            }
            if entity.kind != EntityKind::Plane {
                let y = entity.world_y - 20.0;
                let edge = if entity.direction > 0 {
                    entity.x + entity.width()
                } else {
                    entity.x - 1.0
                };
                if self.river.is_land(edge as i32, y) {
                    entity.direction = -entity.direction;
                }
            }
            entity.x += entity.direction as f32;
            if entity.x >= 160.0 {
                entity.x = 0.0;
            } else if entity.x < 0.0 {
                entity.x = 159.0;
            }
        }
    }
    fn collisions(&mut self, events: &mut Vec<Event>) -> bool {
        for y in 0..19 {
            for x in 0..8 {
                if crate::sprites::player_pixel(x, y, self.bank)
                    && self.river.is_land(
                        self.player_x as i32 + x as i32,
                        self.scroll + 19.0 - y as f32,
                    )
                {
                    self.crash(events);
                    return true;
                }
            }
        }
        let mut hit = None;
        let mut refueling = false;
        for (index, entity) in self.entities.iter().enumerate() {
            if !entity.alive {
                continue;
            }
            let top = self.screen_y(entity.world_y);
            if top > PLAYER_Y + 19.0 || top + 32.0 < PLAYER_Y {
                continue;
            }
            for y in 0..19 {
                for x in 0..8 {
                    if !crate::sprites::player_pixel(x, y, self.bank) {
                        continue;
                    }
                    let ex = self.player_x as i32 + x as i32 - entity.x as i32;
                    let ey = (PLAYER_Y - top) as i32 + y as i32;
                    if ex >= 0
                        && ey >= 0
                        && crate::sprites::sprite_pixel(
                            entity.kind,
                            ex as usize,
                            ey as usize,
                            self.frame,
                            entity.direction,
                        )
                    {
                        if entity.kind == EntityKind::Fuel {
                            refueling = true;
                        } else if entity.kind != EntityKind::House {
                            hit = Some(index);
                        }
                    }
                }
            }
        }
        if let Some(index) = hit {
            self.destroy(index, events);
            self.crash(events);
            return true;
        }
        if refueling {
            let previous = self.fuel;
            self.fuel = self.fuel.saturating_add(512);
            if self.fuel == MAX_FUEL && previous != MAX_FUEL {
                events.push(Event::FullTank);
            } else if self.frame % 8 == 0 {
                events.push(Event::Refuel);
            }
        }
        false
    }
    fn move_bullet(&mut self, fire: bool, events: &mut Vec<Event>) {
        if let Some(mut bullet) = self.bullet {
            // Default Atari difficulty B guides the missile horizontally with the jet.
            bullet.x = self.player_x + 5.0;
            let new_y = bullet.y - 6.0;
            let mut hit = None;
            'entities: for (index, entity) in self.entities.iter().enumerate() {
                if !entity.alive {
                    continue;
                }
                let ex = (bullet.x - entity.x) as i32;
                if ex < 0 {
                    continue;
                }
                let top = self.screen_y(entity.world_y);
                for y in (new_y as i32)..=(bullet.y as i32 + 7) {
                    let ey = y - top as i32;
                    if ey >= 0
                        && crate::sprites::sprite_pixel(
                            entity.kind,
                            ex as usize,
                            ey as usize,
                            self.frame,
                            entity.direction,
                        )
                    {
                        hit = Some(index);
                        break 'entities;
                    }
                }
            }
            if let Some(index) = hit {
                self.destroy(index, events);
                self.bullet = None;
            } else if self
                .river
                .is_land(bullet.x as i32, self.scroll + 160.0 - new_y)
            {
                self.bullet = None;
            } else {
                bullet.y = new_y;
                self.bullet = if new_y + 8.0 >= 0.0 {
                    Some(bullet)
                } else {
                    None
                };
            }
        } else if fire {
            self.bullet = Some(Bullet {
                x: self.player_x + 5.0,
                y: 147.0,
            });
            events.push(Event::Shot);
        }
    }
    fn destroy(&mut self, index: usize, events: &mut Vec<Event>) {
        let entity = &mut self.entities[index];
        entity.alive = false;
        entity.explosion_ticks = 48;
        let bridge = entity.kind == EntityKind::Bridge;
        let points = entity.kind.score();
        if bridge {
            let block = &self.river.blocks[entity.id as usize];
            self.checkpoint_level = if block.level >= 48 {
                47
            } else {
                block.level + 1
            };
            self.checkpoint_seed = block.seed;
            self.bridge_flash = 23;
            events.push(Event::Bridge);
        }
        events.push(Event::Explosion);
        self.add_score(points, events);
    }
    pub fn add_score(&mut self, points: u32, events: &mut Vec<Event>) {
        let old = self.score;
        self.score = self.score.saturating_add(points);
        if self.score / 10000 > old / 10000 && self.lives < 9 {
            self.lives += 1;
            events.push(Event::ExtraLife);
        }
        if self.score > 999990 {
            self.lives = 0;
            self.mode = Mode::GameOver;
            events.push(Event::GameOver);
        }
    }
    fn crash(&mut self, events: &mut Vec<Event>) {
        self.lives = self.lives.saturating_sub(1);
        self.mode = Mode::Exploding(90);
        self.bullet = None;
        events.push(Event::Explosion);
    }
    fn respawn(&mut self) {
        self.scroll = 158.0;
        self.player_x = 76.0;
        self.fuel = MAX_FUEL;
        self.speed = 64;
        self.horizontal_accel = 0;
        self.previous_right = false;
        self.horizontal_fraction = 0;
        self.vertical_fraction = 0;
        self.bank = 0;
        self.bullet = None;
        // Replay the deterministic section from its checkpoint. Earlier sections do not influence its geometry.
        self.entities.clear();
        self.spawned = 6;
        self.river = River::new_at(self.checkpoint_level, self.checkpoint_seed);
        self.populate();
        self.level = self.river.level;
        self.mode = Mode::Playing;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fuel_has_original_capacity_and_drain() {
        let mut g = Game::new();
        g.start();
        g.entities.clear();
        let before = g.fuel;
        g.tick(Input::default());
        assert_eq!(g.fuel, before - 32);
    }
    #[test]
    fn bonus_lives_every_ten_thousand_capped_at_nine() {
        let mut g = Game::new();
        let mut e = Vec::new();
        g.add_score(9990, &mut e);
        assert_eq!(g.lives, 3);
        g.add_score(30, &mut e);
        assert_eq!(g.lives, 4);
        assert!(e.contains(&Event::ExtraLife));
        g.lives = 9;
        g.add_score(10000, &mut e);
        assert_eq!(g.lives, 9);
    }
    #[test]
    fn pause_freezes_all_simulation() {
        let mut g = Game::new();
        g.start();
        g.mode = Mode::Paused;
        let snapshot = (g.frame, g.scroll, g.fuel);
        g.tick(Input {
            fire: true,
            up: true,
            ..Input::default()
        });
        assert_eq!((g.frame, g.scroll, g.fuel), snapshot);
    }
    #[test]
    fn score_values_match_cartridge() {
        assert_eq!(
            [
                EntityKind::Ship,
                EntityKind::Helicopter,
                EntityKind::Plane,
                EntityKind::Fuel,
                EntityKind::Bridge
            ]
            .map(EntityKind::score),
            [30, 60, 100, 80, 500]
        );
    }
    #[test]
    fn deterministic_inputs_produce_identical_state() {
        let (mut a, mut b) = (Game::new(), Game::new());
        a.start();
        b.start();
        for frame in 0..1500 {
            let input = Input {
                fire: true,
                right: frame % 200 < 30,
                left: frame % 200 > 170,
                up: frame % 300 < 100,
                ..Input::default()
            };
            a.tick(input);
            b.tick(input);
        }
        assert_eq!(
            (a.scroll, a.score, a.lives, a.fuel, a.mode),
            (b.scroll, b.score, b.lives, b.fuel, b.mode)
        );
    }
    #[test]
    fn movement_and_fuel_match_124_original_rom_frames() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../docs/evidence/motion-vectors.json")).unwrap();
        let mut game = Game::new();
        game.start();
        for row in fixture["rows"].as_array().unwrap() {
            let flag = |name: &str| row["input"][name].as_bool().unwrap();
            game.tick(Input {
                left: flag("left"),
                right: flag("right"),
                up: flag("up"),
                down: flag("down"),
                fire: flag("fire"),
                ..Input::default()
            });
            let number = |name: &str| row[name].as_u64().unwrap();
            let frame = number("frame");

            assert_eq!(
                game.player_x,
                number("x") as f32,
                "x at original frame {frame}"
            );
            assert_eq!(
                game.horizontal_fraction as u64,
                number("fraction_x"),
                "horizontal fraction at frame {frame}"
            );
            assert_eq!(
                game.horizontal_accel as u64,
                number("acceleration"),
                "acceleration at frame {frame}"
            );
            assert_eq!(game.speed as u64, number("speed"), "speed at frame {frame}");
            assert_eq!(game.fuel as u64, number("fuel"), "fuel at frame {frame}");
            assert_eq!(
                game.scroll as u64 % 32,
                number("offset"),
                "scroll at frame {frame}"
            );
            assert_eq!(
                game.vertical_fraction as u64,
                number("fraction_y"),
                "scroll fraction at frame {frame}"
            );
        }
        // The unmodified-ROM probe crashes into the helicopter on frame285.
        game.tick(Input::default());
        assert!(matches!(game.mode, Mode::Exploding(_)));
        assert_eq!(game.lives, 2);
    }
    #[test]
    fn original_maximum_horizontal_acceleration_moves_every_frame() {
        let mut game = Game::new();
        game.player_x = -1000.0;
        game.horizontal_accel = 248;
        for _ in 0..10 {
            game.player_x = 70.0;
            game.move_player(Input {
                right: true,
                ..Input::default()
            });
            assert_eq!(game.player_x, 71.0);
            assert_eq!(game.horizontal_accel, 248);
        }
    }
    #[test]
    fn depot_refuels_without_being_destroyed() {
        let mut game = Game::new();
        game.start();
        game.entities.clear();
        game.fuel = 10000;
        game.entities.push(Entity {
            id: 0,
            kind: EntityKind::Fuel,
            x: 76.0,
            world_y: game.scroll + 30.0,
            direction: 1,
            alive: true,
            explosion_ticks: 0,
            moving: false,
        });
        game.tick(Input::default());
        assert_eq!(game.fuel, 10480);
        assert!(game.entities[0].alive);
        assert_eq!(game.mode, Mode::Playing);
    }
    #[test]
    fn crash_consumes_one_life_and_restores_full_tank_at_checkpoint() {
        let mut game = Game::new();
        game.start();
        game.player_x = 0.0;
        game.tick(Input::default());
        assert_eq!(game.lives, 2);
        assert!(matches!(game.mode, Mode::Exploding(_)));
        for _ in 0..90 {
            game.tick(Input::default());
        }
        assert_eq!(game.mode, Mode::Playing);
        assert_eq!(game.fuel, MAX_FUEL);
        assert_eq!(game.player_x, 76.0);
    }
    #[test]
    fn bridge_checkpoint_preserves_its_next_section_seed() {
        let mut game = Game::new();
        game.start();
        game.river.ensure(1216.0);
        game.populate();
        let bridge_index = game
            .entities
            .iter()
            .position(|e| e.kind == EntityKind::Bridge)
            .unwrap();
        let seed = game.river.blocks[game.entities[bridge_index].id as usize].seed;
        game.destroy(bridge_index, &mut Vec::new());
        game.respawn();
        assert_eq!(game.checkpoint_seed, seed);
        assert_eq!(game.checkpoint_level, 2);
        assert_eq!(game.level, 2);
        let mut expected = crate::river::Lfsr(seed);
        for _ in 0..4 {
            expected.advance();
        }
        assert_eq!(game.river.random, expected);
    }
    #[test]
    fn horizontal_movement_stops_immediately_without_input() {
        let mut g = Game::new();
        g.start();
        for _ in 0..20 {
            g.move_player(Input {
                right: true,
                ..Input::default()
            });
        }
        let x = g.player_x;
        g.move_player(Input::default());
        assert_eq!(g.player_x, x);
        assert_eq!(g.bank, 0);
    }
}
