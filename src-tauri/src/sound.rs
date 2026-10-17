//! Ten chimes, synthesised in Rust at runtime — no audio assets ship with Ember.
//!
//! A single long-lived audio worker thread owns the output stream (cpal streams
//! are not `Send`) and plays whatever the rest of the app asks for. That means
//! chimes keep working while the window is minimised.

use std::f64::consts::PI;
use std::num::{NonZeroU16, NonZeroU32};
use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};

use rodio::buffer::SamplesBuffer;
use rodio::{DeviceSinkBuilder, MixerDeviceSink};

use crate::settings::Chime;

const SR: u32 = 44_100;

enum AudioCmd {
    Play { chime: Chime, volume: f32 },
}

static AUDIO_TX: OnceLock<Sender<AudioCmd>> = OnceLock::new();

fn sender() -> &'static Sender<AudioCmd> {
    AUDIO_TX.get_or_init(|| {
        let (tx, rx) = channel::<AudioCmd>();
        let _ = std::thread::Builder::new()
            .name("ember-audio".into())
            .spawn(move || audio_loop(rx));
        tx
    })
}

/// Kick the worker awake so the first real chime is not late.
pub fn warm_up() {
    let _ = sender();
}

/// Queue a chime. Never blocks and never fails loudly.
pub fn play(chime: Chime, volume: f32) {
    let _ = sender().send(AudioCmd::Play {
        chime,
        volume: volume.clamp(0.0, 1.0),
    });
}

fn audio_loop(rx: std::sync::mpsc::Receiver<AudioCmd>) {
    let sink: Option<MixerDeviceSink> = match DeviceSinkBuilder::open_default_sink() {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("ember: audio output unavailable ({e}); chimes disabled");
            None
        }
    };

    while let Ok(cmd) = rx.recv() {
        match cmd {
            AudioCmd::Play { chime, volume } => {
                let Some(sink) = sink.as_ref() else { continue };
                let data = synth(chime);
                let buffer = SamplesBuffer::new(
                    NonZeroU16::new(1).unwrap(),
                    NonZeroU32::new(SR).unwrap(),
                    data,
                );
                let player = rodio::Player::connect_new(sink.mixer());
                player.set_volume(volume);
                player.append(buffer);
                // Let it ring out even if this handle is dropped.
                player.detach();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Synthesis
// ---------------------------------------------------------------------------

fn render(ms: u64, f: impl Fn(f64) -> f64) -> Vec<f32> {
    let n = (SR as u64 * ms / 1000) as usize;
    (0..n).map(|i| f(i as f64 / SR as f64) as f32).collect()
}

fn expdec(t: f64, tau: f64) -> f64 {
    (-t / tau).exp()
}

fn sine(f: f64, t: f64) -> f64 {
    (2.0 * PI * f * t).sin()
}

fn tri(f: f64, t: f64) -> f64 {
    (2.0 / PI) * sine(f, t).asin()
}

/// Phase-continuous linear frequency sweep.
fn sweep(f0: f64, f1: f64, dur: f64, t: f64) -> f64 {
    (2.0 * PI * (f0 * t + (f1 - f0) * t * t / (2.0 * dur))).sin()
}

/// Attack ramp + tail fade so nothing clicks.
fn apply_edges(buf: &mut [f32]) {
    let n = buf.len();
    if n == 0 {
        return;
    }
    let head = ((SR as f64 * 0.004) as usize).min(n);
    let tail = ((SR as f64 * 0.012) as usize).min(n);
    for (i, sample) in buf[..head].iter_mut().enumerate() {
        *sample *= i as f32 / head as f32;
    }
    for i in 0..tail {
        let idx = n - 1 - i;
        buf[idx] *= i as f32 / tail as f32;
    }
}

fn normalize(buf: &mut [f32], peak: f32) {
    let max = buf.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if max > 0.0 {
        let k = peak / max;
        for s in buf.iter_mut() {
            *s *= k;
        }
    }
}

fn mix_into(dst: &mut Vec<f32>, src: &[f32], offset_ms: u64, gain: f32) {
    let offset = (SR as u64 * offset_ms / 1000) as usize;
    if dst.len() < offset + src.len() {
        dst.resize(offset + src.len(), 0.0);
    }
    for (i, s) in src.iter().enumerate() {
        dst[offset + i] += s * gain;
    }
}

/// Render one chime to a mono f32 buffer.
pub fn synth(chime: Chime) -> Vec<f32> {
    let (mut buf, peak): (Vec<f32>, f32) = match chime {
        Chime::Glass => {
            let b = render(760, |t| {
                (sine(1180.0, t) + 0.45 * sine(1770.0, t) + 0.22 * sine(2640.0, t))
                    * expdec(t, 0.17)
            });
            (b, 0.85)
        }
        Chime::Droplet => {
            let b = render(340, |t| {
                sweep(1500.0, 680.0, 0.34, t) * expdec(t, 0.075)
                    + 0.25 * sine(400.0, t) * expdec(t, 0.05)
            });
            (b, 0.85)
        }
        Chime::Bell => {
            let b = render(1600, |t| {
                (sine(523.25, t)
                    + 0.5 * sine(1046.5, t)
                    + 0.28 * sine(1569.8, t)
                    + 0.14 * sine(2093.0, t))
                    * expdec(t, 0.55)
            });
            (b, 0.8)
        }
        Chime::Marimba => {
            let mut b = render(560, |t| tri(659.3, t) * expdec(t, 0.2));
            let second = render(420, |t| tri(987.8, t) * expdec(t, 0.16));
            mix_into(&mut b, &second, 130, 0.7);
            (b, 0.85)
        }
        Chime::Pulse => {
            let b = render(760, |t| {
                let am = 0.55 + 0.45 * ((2.0 * PI * 7.0 * t).sin() * 0.5 + 0.5);
                (sine(300.0, t) + 0.3 * sine(600.0, t)) * expdec(t, 0.45) * am
            });
            (b, 0.8)
        }
        Chime::Sparkle => {
            let mut b = Vec::new();
            for (i, f) in [1760.0, 2349.3, 3136.0].iter().enumerate() {
                let blip = render(110, |t| sine(*f, t) * expdec(t, 0.045));
                mix_into(&mut b, &blip, i as u64 * 85, 1.0);
            }
            (b, 0.8)
        }
        Chime::LowTone => {
            let b = render(950, |t| {
                (0.75 * sine(131.0, t) + 0.25 * tri(131.0, t)) * expdec(t, 0.34)
            });
            (b, 0.85)
        }
        Chime::DoubleTap => {
            let mut b = Vec::new();
            let tap = render(60, |t| sine(190.0, t) * expdec(t, 0.022));
            mix_into(&mut b, &tap, 0, 1.0);
            mix_into(&mut b, &tap, 105, 0.85);
            (b, 0.8)
        }
        Chime::Rise => {
            let b = render(620, |t| sweep(320.0, 1280.0, 0.4, t) * expdec(t, 0.32));
            (b, 0.85)
        }
        Chime::Fall => {
            let b = render(620, |t| sweep(1280.0, 320.0, 0.4, t) * expdec(t, 0.32));
            (b, 0.85)
        }
    };

    normalize(&mut buf, peak);
    apply_edges(&mut buf);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_chimes_render_audible_finite_samples() {
        for chime in Chime::ALL {
            let buf = synth(chime);
            assert!(!buf.is_empty(), "{} rendered nothing", chime.id());
            assert!(
                buf.iter().all(|s| s.is_finite()),
                "{} produced non-finite samples",
                chime.id()
            );
            let peak = buf.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(peak > 0.4, "{} too quiet ({peak})", chime.id());
            assert!(peak <= 1.0, "{} clips ({peak})", chime.id());
        }
    }

    #[test]
    fn durations_are_reasonable() {
        for chime in Chime::ALL {
            let ms = synth(chime).len() as f64 / SR as f64 * 1000.0;
            assert!((50.0..=2000.0).contains(&ms), "{} is {ms}ms", chime.id());
        }
    }
}
