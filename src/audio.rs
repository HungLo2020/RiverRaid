//! River Raid sound synthesis with direct playback through the host's ALSA.
//! Tone/noise modes and timings follow the TIA sound routines; this is not a
//! cycle-exact TIA emulator. The original program never executes at runtime.
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, Ordering},
    mpsc::{self, SyncSender},
};
use std::thread::JoinHandle;

#[derive(Clone, Copy, Debug)]
pub enum Sound {
    Shot,
    Explosion,
    Refuel,
    FullTank,
    ExtraLife,
    LowFuel,
    GameOver,
}

struct Voice {
    sound: Sound,
    age: f32,
    duration: f32,
    phase: f32,
}

#[derive(Default)]
pub struct Synth {
    voices: Vec<Voice>,
    noise: u16,
    noise_phase: f32,
    noise_value: f32,
    engine: Option<u8>,
}

impl Synth {
    pub fn trigger(&mut self, sound: Sound) {
        if self.voices.len() == 16 {
            self.voices.remove(0);
        }
        let duration = match sound {
            Sound::Shot => 15.0 / 60.0,
            Sound::Explosion => 31.0 / 60.0,
            Sound::Refuel | Sound::FullTank => 8.0 / 60.0,
            Sound::ExtraLife => 0.18,
            Sound::LowFuel => 0.12,
            Sound::GameOver => 35.0 / 60.0,
        };
        self.voices.push(Voice {
            sound,
            age: 0.0,
            duration,
            phase: 0.0,
        });
    }
    pub fn clear(&mut self) {
        self.voices.clear();
    }
    pub fn set_engine(&mut self, speed: Option<u8>) {
        self.engine = speed;
    }
    pub fn sample(&mut self, sample_rate: f32) -> f32 {
        let dt = 1.0 / sample_rate;
        let divider = self.engine.map_or(9, |speed| 31 - (speed >> 4)).max(1);
        self.noise_phase += 31_400.0 / f32::from(divider) * dt;
        if self.noise_phase >= 1.0 {
            self.noise_phase -= 1.0;
            if self.noise == 0 {
                self.noise = 0x1ff;
            }
            let feedback = (self.noise ^ (self.noise >> 4)) & 1;
            self.noise = (self.noise >> 1) | (feedback << 8);
            self.noise_value = if self.noise & 1 == 0 { -1.0 } else { 1.0 };
        }
        let mut mixed = if self.engine.is_some() {
            self.noise_value * 0.055
        } else {
            0.0
        };
        for voice in &mut self.voices {
            let fraction = voice.age / voice.duration;
            let envelope = (1.0 - fraction).max(0.0);
            let (frequency, noise_mix, gain) = match voice.sound {
                Sound::Shot => (31_400.0 / (6.0 * (14.0 + 15.0 * fraction)), 0.0, 0.13),
                Sound::Explosion => (31_400.0 / 29.0, 1.0, 0.25),
                Sound::Refuel => (31_400.0 / 64.0, 0.0, 0.13),
                Sound::FullTank => (31_400.0 / 32.0, 0.0, 0.13),
                Sound::ExtraLife => (1100.0 + 500.0 * fraction, 0.0, 0.10),
                Sound::LowFuel => (31_400.0 / (6.0 * (15.0 + 15.0 * fraction)), 0.0, 0.14),
                Sound::GameOver => (31_400.0 / 9.0, 1.0, 0.20),
            };
            voice.phase = (voice.phase + frequency * dt).fract();
            let square = if voice.phase < 0.5 { -1.0 } else { 1.0 };
            mixed += (square * (1.0 - noise_mix) + self.noise_value * noise_mix) * envelope * gain;
            voice.age += dt;
        }
        self.voices.retain(|v| v.age < v.duration);
        mixed.clamp(-0.85, 0.85)
    }
}

pub struct Audio {
    sender: SyncSender<Sound>,
    muted: Arc<AtomicBool>,
    engine: Arc<AtomicU32>,
    running: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Audio {
    #[cfg(target_os = "linux")]
    pub fn open(device: &str, muted: bool) -> Result<Self, Box<dyn std::error::Error>> {
        use alsa::{
            Direction, ValueOr,
            pcm::{Access, Format, HwParams, PCM},
        };
        let pcm = PCM::new(device, Direction::Playback, false)?;
        let rate;
        {
            let params = HwParams::any(&pcm)?;
            params.set_channels(2)?;
            params.set_rate(48_000, ValueOr::Nearest)?;
            params.set_access(Access::RWInterleaved)?;
            params.set_format(Format::s16())?;
            params.set_buffer_time_near(40_000, ValueOr::Nearest)?;
            params.set_period_time_near(5_000, ValueOr::Nearest)?;
            pcm.hw_params(&params)?;
            rate = params.get_rate()?;
        }
        let (sender, receiver) = mpsc::sync_channel(64);
        let muted = Arc::new(AtomicBool::new(muted));
        let running = Arc::new(AtomicBool::new(true));
        let engine = Arc::new(AtomicU32::new(0));
        let engine_flag = engine.clone();
        let mute_flag = muted.clone();
        let run_flag = running.clone();
        let worker = std::thread::Builder::new()
            .name("alsa-playback".into())
            .spawn(move || {
                let Ok(output) = pcm.io_i16() else {
                    return;
                };
                let mut synth = Synth::default();
                let mut samples = [0_i16; 512];
                while run_flag.load(Ordering::Relaxed) {
                    for sound in receiver.try_iter().take(64) {
                        synth.trigger(sound);
                    }
                    let muted = mute_flag.load(Ordering::Relaxed);
                    let engine = engine_flag.load(Ordering::Relaxed);
                    synth.set_engine(if muted || engine == 0 {
                        None
                    } else {
                        Some((engine - 1) as u8)
                    });
                    if muted {
                        synth.clear();
                    }
                    for frame in samples.chunks_exact_mut(2) {
                        let sample = (synth.sample(rate as f32) * i16::MAX as f32) as i16;
                        frame.fill(sample);
                    }
                    let mut offset = 0;
                    while offset < samples.len() && run_flag.load(Ordering::Relaxed) {
                        match output.writei(&samples[offset..]) {
                            Ok(0) => {
                                std::thread::sleep(std::time::Duration::from_millis(1));
                            }
                            Ok(frames) => offset += frames * 2,
                            Err(error) => {
                                if pcm.try_recover(error, true).is_err() {
                                    return;
                                }
                            }
                        }
                    }
                }
                let _ = pcm.drop();
            })?;
        Ok(Self {
            sender,
            muted,
            engine,
            running,
            worker: Some(worker),
        })
    }

    #[cfg(not(target_os = "linux"))]
    pub fn open(_device: &str, _muted: bool) -> Result<Self, Box<dyn std::error::Error>> {
        Err("ALSA playback is available on Linux".into())
    }

    pub fn set_engine(&self, speed: u8, active: bool) {
        self.engine.store(
            if active { u32::from(speed) + 1 } else { 0 },
            Ordering::Relaxed,
        );
    }

    pub fn play(&self, sound: Sound) {
        let _ = self.sender.try_send(sound);
    }
    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn engine_can_be_disabled_without_a_sound_device() {
        let mut synth = Synth::default();
        synth.set_engine(Some(128));
        assert!((0..100).any(|_| synth.sample(48000.0).abs() > 0.01));
        synth.set_engine(None);
        assert_eq!(synth.sample(48000.0), 0.0);
    }
    #[test]
    fn sound_is_bounded_and_decays_to_silence() {
        let mut synth = Synth::default();
        synth.trigger(Sound::Explosion);
        let mut peak = 0.0_f32;
        for _ in 0..48_000 {
            let sample = synth.sample(48_000.0);
            assert!(sample.is_finite() && sample.abs() <= 0.85);
            peak = peak.max(sample.abs());
        }
        assert!(peak > 0.05);
        assert_eq!(synth.sample(48_000.0), 0.0);
    }
}
