use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::mpsc::{self, Receiver, Sender};
use std::{
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone)]
pub struct Sample {
    pub data: Vec<f32>,
    pub rate: u32,
    pub channels: usize,
}
impl Sample {
    pub fn wav(bytes: &[u8], trim_ms: Option<u32>) -> Result<Self, String> {
        let mut reader = hound::WavReader::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
        let spec = reader.spec();
        let raw: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => reader
                .samples::<f32>()
                .map(|s| s.map_err(|e| e.to_string()))
                .collect::<Result<_, _>>()?,
            hound::SampleFormat::Int => {
                let scale = (1u64 << (spec.bits_per_sample - 1)) as f32;
                reader
                    .samples::<i32>()
                    .map(|s| s.map(|v| v as f32 / scale).map_err(|e| e.to_string()))
                    .collect::<Result<_, _>>()?
            }
        };
        let count = trim_ms
            .map(|ms| spec.sample_rate as usize * ms as usize / 1000)
            .unwrap_or(usize::MAX);
        let data = raw
            .into_iter()
            .take(count.saturating_mul(spec.channels as usize))
            .collect();
        Ok(Self {
            data,
            rate: spec.sample_rate,
            channels: spec.channels as usize,
        })
    }
    pub fn resample(&self, rate: u32) -> Vec<f32> {
        if rate == self.rate {
            return self.data.clone();
        }
        let count = (self.data.len() / self.channels) as u64 * rate as u64 / self.rate as u64;
        (0..count * self.channels as u64)
            .map(|sample| {
                let i = sample / self.channels as u64;
                let channel = sample as usize % self.channels;
                let position = i as f64 * self.rate as f64 / rate as f64;
                let index = position as usize * self.channels + channel;
                let a = self.data[index];
                let b = *self.data.get(index + self.channels).unwrap_or(&a);
                a + (b - a) * position.fract() as f32
            })
            .collect()
    }
    fn tone(hz: f32) -> Self {
        let rate = 48000;
        let len = rate / 20;
        let data = (0..len)
            .map(|i| {
                let edge = (i.min(len - 1 - i) as f32 / 240.0).min(1.0);
                (std::f32::consts::TAU * hz * i as f32 / rate as f32).sin() * 0.2 * edge
            })
            .collect();
        Self {
            data,
            rate,
            channels: 1,
        }
    }
}

pub enum Command {
    Device(Option<String>),
    Quit,
}
pub struct Audio {
    pub flags: Arc<AtomicU32>,
    pub preview: Arc<AtomicU32>,
    pub tx: Sender<Command>,
    pub status: Receiver<String>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Audio {
    pub fn start(device: Option<String>, flags: u32) -> Result<Self, String> {
        let samples = vec![
            Sample::wav(include_bytes!("../assets/tick.wav"), Some(120))?,
            Sample::tone(250.0),
            Sample::tone(1000.0),
            Sample::wav(include_bytes!("../assets/minute.wav"), None)?,
        ];
        let flags = Arc::new(AtomicU32::new(flags));
        let preview = Arc::new(AtomicU32::new(0));
        let (tx, rx) = mpsc::channel();
        let (status_tx, status) = mpsc::channel();
        let f = flags.clone();
        let p = preview.clone();
        let worker = std::thread::spawn(move || {
            let host = cpal::default_host();
            let mut selected = device;
            let mut stream = None;
            let failed = Arc::new(AtomicU32::new(0));
            let mut last_default = String::new();
            let mut last_status = String::new();
            let mut last_check = std::time::Instant::now() - Duration::from_secs(5);
            loop {
                match rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(Command::Quit) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Ok(Command::Device(d)) => {
                        selected = d;
                        stream = None;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => (),
                }
                if failed.swap(0, Ordering::Relaxed) != 0 {
                    stream = None;
                }
                if last_check.elapsed() >= Duration::from_secs(2) {
                    last_check = std::time::Instant::now();
                    if selected.is_none() {
                        let name = host
                            .default_output_device()
                            .and_then(|d| d.name().ok())
                            .unwrap_or_default();
                        if name != last_default {
                            last_default = name;
                            stream = None;
                        }
                    }
                }
                if stream.is_none() {
                    let result = open(
                        &host,
                        selected.as_deref(),
                        &samples,
                        f.clone(),
                        p.clone(),
                        failed.clone(),
                    );
                    let message = match result {
                        Ok((s, name)) => {
                            stream = Some(s);
                            format!("Sound output: {name}")
                        }
                        Err(e) => format!("Audio unavailable: {e}. Retrying automatically."),
                    };
                    if message != last_status {
                        let _ = status_tx.send(message.clone());
                        last_status = message;
                    }
                }
            }
        });
        Ok(Self {
            flags,
            preview,
            tx,
            status,
            worker: Some(worker),
        })
    }
}
impl Drop for Audio {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Quit);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}
pub fn devices() -> Result<Vec<String>, String> {
    let mut names: Vec<_> = cpal::default_host()
        .output_devices()
        .map_err(|e| e.to_string())?
        .filter_map(|d| d.name().ok())
        .collect();
    names.sort();
    names.dedup();
    Ok(names)
}
fn open(
    host: &cpal::Host,
    selected: Option<&str>,
    samples: &[Sample],
    flags: Arc<AtomicU32>,
    preview: Arc<AtomicU32>,
    failed: Arc<AtomicU32>,
) -> Result<(cpal::Stream, String), String> {
    let device = if let Some(name) = selected {
        host.output_devices()
            .map_err(|e| e.to_string())?
            .find(|d| d.name().ok().as_deref() == Some(name))
            .ok_or_else(|| format!("The selected device '{name}' is disconnected"))?
    } else {
        host.default_output_device()
            .ok_or("No output device found")?
    };
    let name = device.name().map_err(|e| e.to_string())?;
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let config: cpal::StreamConfig = supported.clone().into();
    let rate = config.sample_rate.0;
    let prepared: Vec<_> = samples
        .iter()
        .map(|s| {
            s.resample(rate)
                .chunks_exact(s.channels)
                .map(|f| {
                    if f.len() == 1 {
                        [f[0], f[0]]
                    } else {
                        [f[0], f[1]]
                    }
                })
                .collect()
        })
        .collect();
    let mixer = Mixer::new(prepared, rate, flags, preview);
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => build::<f32>(&device, &config, mixer, failed),
        cpal::SampleFormat::I16 => build::<i16>(&device, &config, mixer, failed),
        cpal::SampleFormat::U16 => build::<u16>(&device, &config, mixer, failed),
        cpal::SampleFormat::I32 => build::<i32>(&device, &config, mixer, failed),
        cpal::SampleFormat::F64 => build::<f64>(&device, &config, mixer, failed),
        other => return Err(format!("Unsupported output format {other:?}")),
    }
    .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((stream, name))
}
fn build<T: cpal::SizedSample + cpal::FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut mixer: Mixer,
    failed: Arc<AtomicU32>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let channels = config.channels as usize;
    device.build_output_stream(
        config,
        move |output: &mut [T], _| {
            mixer.sync_clock();
            for frame in output.chunks_mut(channels) {
                let value = mixer.next();
                for (index, channel) in frame.iter_mut().enumerate() {
                    *channel = T::from_sample(if channels == 1 {
                        (value[0] + value[1]) * 0.5
                    } else {
                        value[index % 2]
                    });
                }
            }
        },
        move |_| {
            failed.store(1, Ordering::Relaxed);
        },
        None,
    )
}
/// All PCM is prepared before entering the audio callback. No locks, allocation, UI, or file access.
struct Mixer {
    samples: Vec<Vec<[f32; 2]>>,
    positions: [usize; 4],
    preview_active: bool,
    rate: u32,
    anchor: f64,
    frames: u64,
    last_second: u64,
    flags: Arc<AtomicU32>,
    preview: Arc<AtomicU32>,
}
fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
pub fn cue_mask(second: u32, flags: u32) -> u32 {
    (flags & 3)
        | if second >= 55 { flags & 4 } else { 0 }
        | (flags & 8)
        | if second == 0 { flags & 16 } else { 0 }
}
impl Mixer {
    fn new(
        samples: Vec<Vec<[f32; 2]>>,
        rate: u32,
        flags: Arc<AtomicU32>,
        preview: Arc<AtomicU32>,
    ) -> Self {
        let anchor = now();
        Self {
            samples,
            positions: [usize::MAX; 4],
            preview_active: false,
            rate,
            anchor,
            frames: 0,
            last_second: anchor as u64,
            flags,
            preview,
        }
    }
    fn sync_clock(&mut self) {
        let wall = now();
        let expected = self.anchor + self.frames as f64 / self.rate as f64;
        if (wall - expected).abs() > 0.1 {
            self.anchor = wall;
            self.frames = 0;
            self.positions = [usize::MAX; 4]; // No backlog after suspend or clock changes.
        }
        if self.preview.swap(0, Ordering::Relaxed) != 0 {
            self.positions[3] = 0;
            self.preview_active = true;
        }
    }
    fn next(&mut self) -> [f32; 2] {
        let epoch = (self.anchor + self.frames as f64 / self.rate as f64) as u64;
        let flags = self.flags.load(Ordering::Relaxed);
        if epoch != self.last_second {
            self.last_second = epoch;
            let sec = (epoch % 60) as u32;
            let mask = cue_mask(sec, flags);
            if mask & 1 != 0 {
                self.positions[0] = 0;
            }
            if mask & 2 != 0 {
                self.positions[1] = 0;
            }
            if mask & 4 != 0 {
                self.positions[2] = 0;
            }
            if mask & 16 != 0 {
                self.positions[3] = 0;
                self.preview_active = false;
            }
        }
        self.frames += 1;
        let mut value = [0.0; 2];
        for voice in 0..4 {
            let bit = [1, 2, 4, 16][voice];
            // Test sound is allowed even with the minute cue disabled.
            if flags & bit == 0 && !(voice == 3 && self.preview_active) {
                self.positions[voice] = usize::MAX;
            }
            if let Some(sample) = self
                .samples
                .get(voice)
                .and_then(|s| s.get(self.positions[voice]))
            {
                value[0] += sample[0] * 0.85;
                value[1] += sample[1] * 0.85;
                self.positions[voice] += 1;
            }
        }
        [value[0].clamp(-1.0, 1.0), value[1].clamp(-1.0, 1.0)]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_assets_decode_and_tick_is_trimmed() {
        let tick = Sample::wav(include_bytes!("../assets/tick.wav"), Some(120)).unwrap();
        assert_eq!(
            tick.data.len(),
            tick.rate as usize * 120 / 1000 * tick.channels
        );
        let minute = Sample::wav(include_bytes!("../assets/minute.wav"), None).unwrap();
        assert!(minute.data.len() > minute.rate as usize / 2);
        assert!(
            tick.data
                .iter()
                .chain(&minute.data)
                .all(|s| s.is_finite() && s.abs() <= 1.0)
        );
    }
    #[test]
    fn correct_minute_and_warning_boundaries() {
        assert_eq!(cue_mask(0, 31), 27);
        assert_eq!(cue_mask(54, 31), 11);
        for s in 55..60 {
            assert_eq!(cue_mask(s, 31), 15);
        }
        assert_eq!(cue_mask(59, 0), 0);
    }
    #[test]
    fn resampling_preserves_duration() {
        let sample = Sample {
            data: vec![0.5; 48000],
            rate: 48000,
            channels: 1,
        };
        let out = sample.resample(44100);
        assert_eq!(out.len(), 44100);
        assert!(out.iter().all(|s| *s == 0.5));
    }
    #[test]
    fn overlapping_cues_clip_safely_and_do_not_replay_after_gap() {
        let f = Arc::new(AtomicU32::new(31));
        let mut m = Mixer::new(
            vec![vec![[1.0; 2]; 100]; 4],
            48000,
            f,
            Arc::new(AtomicU32::new(0)),
        );
        m.anchor = 60.0;
        m.last_second = 59;
        assert_eq!(m.next(), [1.0; 2]);
        m.anchor = 119.0;
        m.frames = 0;
        m.positions = [usize::MAX; 4];
        m.next();
        assert_eq!(m.last_second, 119);
        assert_eq!(m.positions[3], usize::MAX);
    }
}
