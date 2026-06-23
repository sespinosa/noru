use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use rubato::{FftFixedIn, Resampler};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

pub const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// Resample audio from `from_rate` to 16kHz mono.
fn resample(samples: &[f32], from_rate: u32) -> Result<Vec<f32>> {
    if from_rate == WHISPER_SAMPLE_RATE {
        return Ok(samples.to_vec());
    }

    let mut resampler = FftFixedIn::<f32>::new(
        from_rate as usize,
        WHISPER_SAMPLE_RATE as usize,
        samples.len(),
        1, // sub-chunks
        1, // mono
    )?;

    let input = vec![samples.to_vec()];
    let output = resampler.process(&input, None)?;
    Ok(output.into_iter().next().unwrap_or_default())
}

/// Convert interleaved multi-channel audio to mono by averaging channels.
fn to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    samples
        .chunks(channels as usize)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Mix any number of equal-rate mono tracks by summing with a hard clamp.
/// Shorter tracks are treated as zero past their end (so a silent/absent source
/// contributes nothing). Returns an empty vec if every track is empty.
pub fn mix_tracks(tracks: &[&[f32]]) -> Vec<f32> {
    let n = tracks.iter().map(|t| t.len()).max().unwrap_or(0);
    let mut out = vec![0.0f32; n];
    for t in tracks {
        for (o, &s) in out.iter_mut().zip(t.iter()) {
            *o += s;
        }
    }
    for o in out.iter_mut() {
        *o = o.clamp(-1.0, 1.0);
    }
    out
}

/// Which sources noru captures. Mirrors the Settings → Recording UI: a chosen
/// input device (or system default) and a toggle for WASAPI loopback.
#[derive(Clone, Debug)]
pub struct CaptureOptions {
    /// Microphone device name; `None`/empty => system default input device.
    pub mic_device: Option<String>,
    /// Capture the microphone (your own voice).
    pub capture_mic: bool,
    /// Capture system audio via WASAPI loopback (other meeting participants).
    pub capture_system: bool,
}

impl Default for CaptureOptions {
    fn default() -> Self {
        Self {
            mic_device: None,
            capture_mic: true,
            capture_system: true,
        }
    }
}

/// One independently-captured source (mic OR system loopback). The cpal
/// callback pushes native-rate mono f32 chunks through `rx`; the owner drains
/// and resamples to 16k. Kept separate so the two sources never pre-mix —
/// downstream features (audio-scene classification) need them apart.
struct Source {
    rx: mpsc::Receiver<Vec<f32>>,
    sample_rate: u32,
    _stream: Stream,
}

/// 16k-mono audio drained for one capture tick — one field per source, empty
/// when that source produced nothing (or isn't being captured).
pub struct CaptureTick {
    pub mic: Vec<f32>,
    pub system: Vec<f32>,
}

/// Holds the live cpal streams for the requested sources. `Stream` is `!Send`,
/// so this must be created and drained on the same (capture) thread.
pub struct AudioCapture {
    mic: Option<Source>,
    system: Option<Source>,
}

impl AudioCapture {
    /// Open the requested sources. The microphone is a normal capture endpoint;
    /// system audio uses the default *output* (render) device — cpal's WASAPI
    /// backend transparently turns an input stream on a render endpoint into
    /// loopback capture (AUDCLNT_STREAMFLAGS_LOOPBACK).
    pub fn start(opts: &CaptureOptions) -> Result<Self> {
        let host = cpal::default_host();

        let mic = if opts.capture_mic {
            let device = match opts.mic_device.as_deref() {
                Some(name) if !name.is_empty() => find_input_device(&host, name)?,
                _ => host
                    .default_input_device()
                    .context("no default input device found")?,
            };
            let config = device
                .default_input_config()
                .context("failed to get default input config")?;
            Some(open_source(&device, config, "mic")?)
        } else {
            None
        };

        let system = if opts.capture_system {
            match host.default_output_device() {
                Some(device) => {
                    let config = device
                        .default_output_config()
                        .context("failed to get default output config for loopback")?;
                    Some(open_source(&device, config, "system(loopback)")?)
                }
                None => {
                    eprintln!("noru audio: no default output device; system audio unavailable");
                    None
                }
            }
        } else {
            None
        };

        if mic.is_none() && system.is_none() {
            anyhow::bail!("no audio source available (mic and system audio both off/unavailable)");
        }

        Ok(Self { mic, system })
    }

    /// Non-blocking: drain everything buffered since the last call, resampled to
    /// 16k mono, per source. Sources stay independent; the caller mixes.
    pub fn drain(&self) -> CaptureTick {
        CaptureTick {
            mic: self.mic.as_ref().map(drain_source).unwrap_or_default(),
            system: self.system.as_ref().map(drain_source).unwrap_or_default(),
        }
    }

    pub fn has_mic(&self) -> bool {
        self.mic.is_some()
    }

    pub fn has_system(&self) -> bool {
        self.system.is_some()
    }
}

/// Find an input device by exact name.
fn find_input_device(host: &cpal::Host, name: &str) -> Result<cpal::Device> {
    for d in host.input_devices().context("enumerating input devices")? {
        if d.name().map(|n| n == name).unwrap_or(false) {
            return Ok(d);
        }
    }
    anyhow::bail!("audio input device '{name}' not found")
}

/// Build + start an input stream (mic or loopback) feeding native-rate mono f32
/// chunks into a channel. Handles f32/i16/u16 sample formats.
fn open_source(
    device: &cpal::Device,
    config: cpal::SupportedStreamConfig,
    label: &str,
) -> Result<Source> {
    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    let sample_format = config.sample_format();
    let stream_config: cpal::StreamConfig = config.into();
    let (tx, rx) = mpsc::channel::<Vec<f32>>();

    eprintln!(
        "noru audio [{label}]: {} ({}Hz, {}ch, {:?})",
        device.name().unwrap_or_default(),
        sample_rate,
        channels,
        sample_format,
    );

    let err_fn = |err| eprintln!("noru audio stream error: {err}");

    let stream = match sample_format {
        SampleFormat::F32 => {
            let tx = tx.clone();
            device.build_input_stream(
                &stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let _ = tx.send(to_mono(data, channels));
                },
                err_fn,
                None,
            )?
        }
        SampleFormat::I16 => {
            let tx = tx.clone();
            device.build_input_stream(
                &stream_config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let f: Vec<f32> = data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                    let _ = tx.send(to_mono(&f, channels));
                },
                err_fn,
                None,
            )?
        }
        SampleFormat::U16 => {
            let tx = tx.clone();
            device.build_input_stream(
                &stream_config,
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    let f: Vec<f32> = data
                        .iter()
                        .map(|&s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0)
                        .collect();
                    let _ = tx.send(to_mono(&f, channels));
                },
                err_fn,
                None,
            )?
        }
        other => anyhow::bail!("unsupported sample format: {other:?}"),
    };

    stream.play().context("failed to start audio stream")?;

    Ok(Source {
        rx,
        sample_rate,
        _stream: stream,
    })
}

/// Drain a source's buffered native samples and resample to 16k mono. Tiny tail
/// blocks (<~16ms) are dropped — too short for a clean FFT resample and
/// inaudible.
fn drain_source(src: &Source) -> Vec<f32> {
    let mut native = Vec::new();
    while let Ok(chunk) = src.rx.try_recv() {
        native.extend(chunk);
    }
    if native.len() < 256 {
        return Vec::new();
    }
    resample(&native, src.sample_rate).unwrap_or_default()
}

/// Save f32 mono samples to a WAV file, appending if the writer is reused.
pub struct WavWriter {
    writer: Arc<Mutex<hound::WavWriter<std::io::BufWriter<std::fs::File>>>>,
}

impl WavWriter {
    pub fn new(path: &std::path::Path, sample_rate: u32) -> Result<Self> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let writer = hound::WavWriter::create(path, spec)
            .with_context(|| format!("Failed to create WAV file: {}", path.display()))?;

        Ok(Self {
            writer: Arc::new(Mutex::new(writer)),
        })
    }

    pub fn write_samples(&self, samples: &[f32]) -> Result<()> {
        let mut writer = self.writer.lock().unwrap();
        for &sample in samples {
            writer.write_sample(sample)?;
        }
        Ok(())
    }

    pub fn finalize(self) -> Result<()> {
        let writer = Arc::try_unwrap(self.writer)
            .map_err(|_| anyhow::anyhow!("WAV writer still has references"))?
            .into_inner()
            .unwrap();
        writer.finalize()?;
        Ok(())
    }
}

/// Load a WAV file and return 16kHz mono f32 samples ready for Whisper.
pub fn load_wav(path: &std::path::Path) -> Result<Vec<f32>> {
    let mut reader =
        hound::WavReader::open(path).with_context(|| format!("Failed to open: {}", path.display()))?;

    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => {
            let max = (1 << (spec.bits_per_sample - 1)) as f32;
            reader.samples::<i32>().map(|s| s.unwrap() as f32 / max).collect()
        }
    };

    let mono = to_mono(&samples, spec.channels);
    resample(&mono, spec.sample_rate)
}
