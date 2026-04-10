use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use rubato::{FftFixedIn, Resampler};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

const WHISPER_SAMPLE_RATE: u32 = 16_000;

pub struct AudioCapture {
    _stream: Stream,
    receiver: mpsc::Receiver<Vec<f32>>,
    device_sample_rate: u32,
}

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
    if channels == 1 {
        return samples.to_vec();
    }
    samples
        .chunks(channels as usize)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

impl AudioCapture {
    /// Start capturing from the default input device.
    /// Sends chunks of raw f32 samples (at device sample rate) through a channel.
    pub fn start() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("No input audio device found")?;

        let config = device
            .default_input_config()
            .context("Failed to get default input config")?;

        let device_sample_rate = config.sample_rate().0;
        let channels = config.channels();
        let sample_format = config.sample_format();

        eprintln!(
            "Audio device: {} ({}Hz, {}ch, {:?})",
            device.name().unwrap_or_default(),
            device_sample_rate,
            channels,
            sample_format,
        );

        let (tx, rx) = mpsc::channel::<Vec<f32>>();

        let stream = match sample_format {
            SampleFormat::F32 => {
                let tx = tx.clone();
                device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        let mono = to_mono(data, channels);
                        let _ = tx.send(mono);
                    },
                    |err| eprintln!("Audio stream error: {err}"),
                    None,
                )?
            }
            SampleFormat::I16 => {
                let tx = tx.clone();
                device.build_input_stream(
                    &config.into(),
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let f32_data: Vec<f32> =
                            data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                        let mono = to_mono(&f32_data, channels);
                        let _ = tx.send(mono);
                    },
                    |err| eprintln!("Audio stream error: {err}"),
                    None,
                )?
            }
            SampleFormat::U16 => {
                let tx = tx.clone();
                device.build_input_stream(
                    &config.into(),
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        let f32_data: Vec<f32> = data
                            .iter()
                            .map(|&s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0)
                            .collect();
                        let mono = to_mono(&f32_data, channels);
                        let _ = tx.send(mono);
                    },
                    |err| eprintln!("Audio stream error: {err}"),
                    None,
                )?
            }
            _ => anyhow::bail!("Unsupported sample format: {:?}", sample_format),
        };

        stream.play().context("Failed to start audio stream")?;

        Ok(Self {
            _stream: stream,
            receiver: rx,
            device_sample_rate,
        })
    }

    /// Collect audio for `duration_secs` seconds, returning 16kHz mono f32 samples
    /// ready for Whisper. Also returns the raw samples at device sample rate for WAV saving.
    pub fn collect_chunk(&self, duration_secs: f32) -> Result<AudioChunk> {
        let target_samples = (self.device_sample_rate as f32 * duration_secs) as usize;
        let mut raw_samples = Vec::with_capacity(target_samples);

        while raw_samples.len() < target_samples {
            match self.receiver.recv() {
                Ok(chunk) => raw_samples.extend(chunk),
                Err(_) => break,
            }
        }

        let whisper_samples = resample(&raw_samples, self.device_sample_rate)?;

        Ok(AudioChunk {
            raw_samples,
            raw_sample_rate: self.device_sample_rate,
            whisper_samples,
        })
    }

    pub fn device_sample_rate(&self) -> u32 {
        self.device_sample_rate
    }
}

#[allow(dead_code)]
pub struct AudioChunk {
    /// Raw mono samples at device sample rate (for WAV saving)
    pub raw_samples: Vec<f32>,
    pub raw_sample_rate: u32,
    /// Resampled 16kHz mono samples (for Whisper)
    pub whisper_samples: Vec<f32>,
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
