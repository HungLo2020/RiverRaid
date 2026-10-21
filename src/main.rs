mod desktop;
mod input;
use minifb::{Key, MouseButton, ScaleMode, Window, WindowOptions};
use river_raid::{
    audio::{Audio, Sound},
    game::{EntityKind, Event, Game, Input, Mode, TICK_RATE},
    render::{HEIGHT, Renderer, WIDTH},
    storage::{self, Paths, Preferences, Records},
};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::Instant,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Default)]
struct Options {
    headless: bool,
    autoplay: bool,
    ticks: Option<u64>,
    smoke_frames: Option<u64>,
    snapshot: Option<PathBuf>,
    record: Option<PathBuf>,
    replay: Option<PathBuf>,
    no_audio: bool,
    audio_device: String,
}

impl Options {
    fn parse() -> Result<Option<Self>> {
        let mut args = std::env::args().skip(1);
        let mut options = Self {
            audio_device: "default".into(),
            ..Self::default()
        };
        while let Some(arg) = args.next() {
            let mut value = || args.next().ok_or_else(|| format!("{arg} requires a value"));
            match arg.as_str() {
                "--headless" => options.headless = true,
                "--autoplay" => options.autoplay = true,
                "--no-audio" => options.no_audio = true,
                "--ticks" => options.ticks = Some(value()?.parse()?),
                "--smoke-frames" => options.smoke_frames = Some(value()?.parse()?),
                "--snapshot" => options.snapshot = Some(value()?.into()),
                "--record" => options.record = Some(value()?.into()),
                "--replay" => options.replay = Some(value()?.into()),
                "--audio-device" => options.audio_device = value()?,
                "--version" => {
                    println!("River Raid {}", env!("CARGO_PKG_VERSION"));
                    return Ok(None);
                }
                "--help" | "-h" => {
                    println!(
                        "River Raid\n\nArrows / WASD: steer and change speed    Space / left mouse: fire\nEnter: start    P / Escape: pause    F1: help    M: mute    Q: quit while paused\n\n--headless --ticks N --autoplay\n--record FILE / --replay FILE   deterministic JSON input replay\n--snapshot FILE                final framebuffer as PPM\n--smoke-frames N               run native window for N rendered frames\n--no-audio / --audio-device NAME  host ALSA playback (default: default)"
                    );
                    return Ok(None);
                }
                _ => return Err(format!("unknown option {arg}; see --help").into()),
            }
        }
        if options.record.is_some() && options.replay.is_some() {
            return Err("choose either --record or --replay".into());
        }
        Ok(Some(options))
    }
}

#[derive(Serialize, Deserialize)]
struct Replay {
    version: u32,
    frames: Vec<ReplayInput>,
}
#[derive(Serialize, Deserialize)]
struct ReplayInput {
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    fire: bool,
    start: bool,
    pause: bool,
}
impl From<Input> for ReplayInput {
    fn from(input: Input) -> Self {
        Self {
            left: input.left,
            right: input.right,
            up: input.up,
            down: input.down,
            fire: input.fire,
            start: input.start,
            pause: input.pause,
        }
    }
}
impl From<&ReplayInput> for Input {
    fn from(input: &ReplayInput) -> Self {
        Self {
            left: input.left,
            right: input.right,
            up: input.up,
            down: input.down,
            fire: input.fire,
            start: input.start,
            pause: input.pause,
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("River Raid: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let Some(options) = Options::parse()? else {
        return Ok(());
    };
    let replay: Option<Replay> = options
        .replay
        .as_ref()
        .map(|path| -> Result<_> {
            let replay: Replay = serde_json::from_slice(&fs::read(path)?)?;
            if replay.version != 1 {
                return Err("unsupported replay version".into());
            }
            Ok(replay)
        })
        .transpose()?;
    let mut recorded = Replay {
        version: 1,
        frames: Vec::new(),
    };
    let mut game = Game::new();
    let mut renderer = Renderer::new();
    if options.headless {
        let ticks = options
            .ticks
            .unwrap_or_else(|| replay.as_ref().map_or(3600, |r| r.frames.len() as u64));
        for tick in 0..ticks {
            let input = replay
                .as_ref()
                .and_then(|r| r.frames.get(tick as usize))
                .map(Input::from)
                .unwrap_or_else(|| {
                    if replay.is_some() {
                        Input::default()
                    } else {
                        automatic_input(&game, options.autoplay, tick == 0)
                    }
                });
            if options.record.is_some() {
                recorded.frames.push(input.into());
            }
            game.tick(input);
        }
        renderer.render(&game, u64::from(game.score));
        finish_outputs(&options, &recorded, &renderer, &game)?;
        return Ok(());
    }
    let paths = Paths::discover()?;
    paths.create()?;
    let records_path = paths.save.join("records.json");
    let preferences_path = paths.shared.join("preferences.json");
    let (mut records, records_writable) = read_or_default::<Records>(&records_path);
    let (mut preferences, preferences_writable) = read_or_default::<Preferences>(&preferences_path);
    let records_writable = records_writable && records.version <= 1;
    let preferences_writable = preferences_writable && preferences.version <= 1;
    let audio = if options.no_audio {
        None
    } else {
        match Audio::open(&options.audio_device, preferences.muted) {
            Ok(audio) => Some(audio),
            Err(error) => {
                eprintln!("Audio unavailable: {error}");
                None
            }
        }
    };
    let mut window = Window::new(
        "River Raid — Enter to start · F1 help",
        1024,
        768,
        WindowOptions {
            transparency: false,
            borderless: false,
            resize: true,
            scale_mode: ScaleMode::UpperLeft,
            ..WindowOptions::default()
        },
    )?;
    let keyboard = input::Keyboard::attach(&mut window);
    let desktop = desktop::Desktop::attach(&window)?;
    let mut previous_size = (0, 0);
    let mut previous_title = String::new();
    window.set_target_fps(120);
    let mut frame = Vec::new();
    let mut paused = false;
    let mut help = false;
    let mut pending_movement = [false; 4];
    let mut pending_fire = false;
    let mut had_focus = false;
    let mut pending_start = false;
    let mut elapsed = 0.0_f64;
    let mut last = Instant::now();
    let mut rendered = 0_u64;
    let mut simulation_ticks = 0_usize;
    let step = 1.0 / TICK_RATE;
    let mut last_score_saved = records.high_score;
    while window.is_open() {
        let pressed = keyboard.take_pressed();
        let now = Instant::now();
        let dt = (now - last).as_secs_f64().min(0.1);
        last = now;
        if pressed.contains(&Key::F1) {
            help = !help;
        }
        if pressed.contains(&Key::P) || pressed.contains(&Key::Escape) {
            if help {
                help = false;
            } else {
                paused = !paused;
            }
        }
        let focused = window.is_active();
        if had_focus && !focused {
            keyboard.release_all();
            paused = true;
        }
        had_focus = focused;
        if paused && pressed.contains(&Key::Q) {
            break;
        }
        if pressed.contains(&Key::M) {
            preferences.muted = !preferences.muted;
            if preferences_writable {
                storage::atomic_save(&preferences_path, &preferences)?;
            }
        }
        let (width, height) = window.get_size();
        if width == 0 || height == 0 {
            window.update();
            continue;
        }
        if previous_size != (width, height) {
            desktop.resize(width, height)?;
            previous_size = (width, height);
        }
        let viewport = Viewport::new(width, height);
        if pressed.contains(&Key::Enter) {
            if paused || help {
                paused = false;
                help = false;
            } else {
                pending_start = true;
            }
        }
        let stopped = paused || help;
        if let Some(audio) = &audio {
            audio.set_muted(preferences.muted || stopped);
            audio.set_engine(game.speed, !stopped && matches!(game.mode, Mode::Playing));
        }
        for (pending, keys) in pending_movement.iter_mut().zip([
            [Key::Left, Key::A],
            [Key::Right, Key::D],
            [Key::Up, Key::W],
            [Key::Down, Key::S],
        ]) {
            *pending |= keys.iter().any(|key| pressed.contains(key));
        }
        pending_fire |= pressed.contains(&Key::Space);
        if stopped {
            elapsed = 0.0;
            pending_fire = false;
            pending_movement = [false; 4];
            pending_start = false;
        } else {
            elapsed += dt;
            while elapsed >= step {
                let input = if let Some(replay) = &replay {
                    replay
                        .frames
                        .get(simulation_ticks)
                        .map(Input::from)
                        .unwrap_or_default()
                } else if options.autoplay {
                    automatic_input(&game, true, simulation_ticks == 0)
                } else {
                    Input {
                        left: pending_movement[0]
                            || keyboard.held(Key::Left)
                            || keyboard.held(Key::A),
                        right: pending_movement[1]
                            || keyboard.held(Key::Right)
                            || keyboard.held(Key::D),
                        up: pending_movement[2] || keyboard.held(Key::Up) || keyboard.held(Key::W),
                        down: pending_movement[3]
                            || keyboard.held(Key::Down)
                            || keyboard.held(Key::S),
                        fire: pending_fire
                            || keyboard.held(Key::Space)
                            || window.get_mouse_down(MouseButton::Left),
                        start: pending_start,
                        pause: false,
                    }
                };
                if options.record.is_some() {
                    recorded.frames.push(input.into());
                }
                let events = game.tick(input);
                if let Some(audio) = &audio {
                    for event in &events {
                        if let Some(sound) = event_sound(event) {
                            audio.play(sound);
                        }
                    }
                }
                records.high_score = records.high_score.max(u64::from(game.score));
                records.best_level = records.best_level.max(u32::from(game.level));
                if records_writable
                    && last_score_saved != records.high_score
                    && matches!(game.mode, Mode::GameOver)
                {
                    storage::atomic_save(&records_path, &records)?;
                    last_score_saved = records.high_score;
                }
                pending_fire = false;
                pending_movement = [false; 4];
                pending_start = false;
                simulation_ticks += 1;
                elapsed -= step;
            }
        }
        renderer.render(&game, records.high_score);
        if help {
            renderer.overlay(
                "HOW TO PLAY",
                &[
                    "ARROWS / WASD : STEER",
                    "UP / DOWN : SPEED",
                    "SPACE / LEFT MOUSE : FIRE",
                    "FLY OVER FUEL TO REFILL",
                    "DESTROY BRIDGES TO ADVANCE",
                    "10000 POINTS : EXTRA JET",
                    "M : MUTE   ENTER : RETURN",
                ],
            );
        } else if paused {
            renderer.overlay(
                "PAUSED",
                &[
                    "ENTER / P : RESUME",
                    "F1 : HOW TO PLAY",
                    "M : MUTE",
                    "Q : QUIT",
                ],
            );
        }
        let title = if help {
            "River Raid — Controls"
        } else if paused {
            "River Raid — Paused"
        } else {
            "River Raid — F1 help"
        };
        if title != previous_title {
            window.set_title(title);
            previous_title = title.into();
        }
        viewport.present(&renderer.pixels, &mut frame, width, height);
        window.update_with_buffer(&frame, width, height)?;
        rendered += 1;
        if options.smoke_frames.is_some_and(|limit| rendered >= limit) {
            break;
        }
    }
    if records_writable {
        storage::atomic_save(&records_path, &records)?;
    }
    finish_outputs(&options, &recorded, &renderer, &game)?;
    Ok(())
}

fn read_or_default<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> (T, bool) {
    match storage::load(path) {
        Ok(value) => (value, true),
        Err(error) => {
            eprintln!(
                "Cannot read {}: {error}; preserving original file",
                path.display()
            );
            (T::default(), false)
        }
    }
}

/// A test driver, deliberately outside the game rules and unavailable during normal play.
fn automatic_input(game: &Game, autoplay: bool, start: bool) -> Input {
    let mut input = Input {
        start,
        ..Input::default()
    };
    if !autoplay {
        return input;
    }
    // This driver observes the public scene, like a player, without changing rules.
    let center = game.player_x + 4.0;
    let mut safe_left = 0.0_f32;
    let mut safe_right = 160.0_f32;
    for ahead in (0..=64).step_by(8) {
        let row = game.river.row(game.scroll + ahead as f32);
        let (mut left, mut right) = (row.left as f32, row.right as f32);
        if let Some((island_left, island_right)) = row.island {
            if center < 80.0 {
                right = island_left as f32;
            } else {
                left = island_right as f32;
            }
        }
        safe_left = safe_left.max(left + 6.0);
        safe_right = safe_right.min(right - 6.0);
    }
    if safe_left > safe_right {
        let row = game.river.row(game.scroll + 19.0);
        safe_left = row.left as f32 + 6.0;
        safe_right = (row.right as f32 - 6.0).max(safe_left);
    }
    let mut target = (safe_left + safe_right) * 0.5;
    input.fire = true;
    if let Some(entity) = game
        .entities
        .iter()
        .filter(|entity| {
            let y = game.screen_y(entity.world_y);
            entity.alive && entity.kind != EntityKind::House && (10.0..135.0).contains(&y)
        })
        .max_by(|a, b| {
            game.screen_y(a.world_y)
                .total_cmp(&game.screen_y(b.world_y))
        })
    {
        let entity_center = entity.x + entity.width() * 0.5;
        if entity.kind == EntityKind::Fuel {
            if game.fuel_fraction() < 0.8 {
                target = entity_center;
                input.fire = false;
            }
        } else {
            target = entity_center;
        }
    }
    target = target.clamp(safe_left, safe_right);
    input.left = center > target + 1.5;
    input.right = center < target - 1.5;
    input
}

fn event_sound(event: &Event) -> Option<Sound> {
    Some(match event {
        Event::Shot => Sound::Shot,
        Event::Explosion => Sound::Explosion,
        Event::Refuel => Sound::Refuel,
        Event::FullTank => Sound::FullTank,
        Event::ExtraLife => Sound::ExtraLife,
        Event::LowFuel => Sound::LowFuel,
        Event::GameOver => Sound::GameOver,
        Event::Bridge => return None,
    })
}

fn finish_outputs(
    options: &Options,
    recorded: &Replay,
    renderer: &Renderer,
    game: &Game,
) -> Result<()> {
    if let Some(path) = &options.record {
        storage::atomic_save(path, recorded)?;
    }
    if let Some(path) = &options.snapshot {
        write_ppm(path, &renderer.pixels)?;
    }
    println!(
        "ticks={} level={} score={} lives={} fuel={} mode={:?}",
        game.frame, game.level, game.score, game.lives, game.fuel, game.mode
    );
    Ok(())
}

fn write_ppm(path: &Path, pixels: &[u32]) -> io::Result<()> {
    let mut file = io::BufWriter::new(fs::File::create(path)?);
    write!(file, "P6\n{WIDTH} {HEIGHT}\n255\n")?;
    for pixel in pixels {
        file.write_all(&[(pixel >> 16) as u8, (pixel >> 8) as u8, *pixel as u8])?;
    }
    file.flush()
}

struct Viewport {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}
impl Viewport {
    fn new(width: usize, height: usize) -> Self {
        let view_width = width.min(height.saturating_mul(4) / 3).max(1);
        let view_height = (view_width * 3 / 4).max(1).min(height);
        Self {
            x: (width - view_width) / 2,
            y: (height - view_height) / 2,
            width: view_width,
            height: view_height,
        }
    }
    fn present(&self, pixels: &[u32], output: &mut Vec<u32>, width: usize, height: usize) {
        output.resize(width * height, 0xff000000);
        output.fill(0xff000000);
        for y in 0..self.height {
            let source_y = y * HEIGHT / self.height;
            for x in 0..self.width {
                output[(self.y + y) * width + self.x + x] =
                    0xff000000 | pixels[source_y * WIDTH + x * WIDTH / self.width];
            }
        }
    }
}

#[cfg(test)]
mod window_tests {
    use super::*;
    #[test]
    fn composed_frame_and_letterbox_are_always_opaque() {
        let viewport = Viewport::new(1000, 500);
        let mut output = Vec::new();
        let input = vec![0x00010203; WIDTH * HEIGHT];
        viewport.present(&input, &mut output, 1000, 500);
        assert!(output.iter().all(|pixel| pixel >> 24 == 0xff));
        assert_eq!(output[0], 0xff000000);
        assert_eq!(output[viewport.y * 1000 + viewport.x], 0xff010203);
    }
    #[test]
    fn resizing_keeps_four_three_viewport_inside_window() {
        for (w, h) in [(1, 1), (640, 480), (1920, 1080), (400, 1000)] {
            let viewport = Viewport::new(w, h);
            assert!(viewport.width + viewport.x <= w);
            assert!(viewport.height + viewport.y <= h);
            let mut output = Vec::new();
            viewport.present(&vec![0; WIDTH * HEIGHT], &mut output, w, h);
            assert_eq!(output.len(), w * h);
        }
    }
}
