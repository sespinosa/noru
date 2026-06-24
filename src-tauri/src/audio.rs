use anyhow::{Context, Result};
use audiopus::{coder::Encoder as OpusEncoder, Application, Bitrate, Channels, SampleRate};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use ogg::writing::{PacketWriteEndInfo, PacketWriter};
use rubato::{FftFixedIn, Resampler};
use std::io::Write;
use std::sync::mpsc;

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

// 20ms frames: 320 samples at the 16k input rate; Ogg Opus granule positions
// always run at 48kHz, so 20ms == 960 granule units regardless of input rate.
const OPUS_FRAME: usize = 320;
const OPUS_GRANULE_STEP: u64 = 960;
const OPUS_SERIAL: u32 = 0x6e6f_7275; // "noru"

/// Streams 16kHz mono f32 audio into an Ogg Opus file. ~20x smaller than the
/// previous 32-bit float WAV, speech-optimized, and natively playable in
/// browsers / WebView2. Samples are buffered into fixed Opus frames and encoded
/// incrementally as they arrive.
pub struct OpusWriter {
    encoder: OpusEncoder,
    writer: PacketWriter<'static, std::fs::File>,
    pending: Vec<f32>,
    granule: u64,
    scratch: Vec<u8>,
}

impl OpusWriter {
    /// `_sample_rate` is accepted for call-site parity with the old WAV writer;
    /// Opus here is always fed 16kHz mono (the Whisper-aligned capture rate).
    pub fn new(path: &std::path::Path, _sample_rate: u32) -> Result<Self> {
        let mut encoder = OpusEncoder::new(SampleRate::Hz16000, Channels::Mono, Application::Audio)
            .map_err(|e| anyhow::anyhow!("creating opus encoder: {e}"))?;
        // ~24kbps mono is ample for meeting speech and keeps files tiny
        // (~3KB/s) for archival/hosting.
        let _ = encoder.set_bitrate(Bitrate::BitsPerSecond(24_000));
        // OPUS_GET_LOOKAHEAD is in input-rate (16k) samples; pre-skip is at 48k.
        let pre_skip = (encoder.lookahead().unwrap_or(0).saturating_mul(3)).min(u16::MAX as u32) as u16;

        let file = std::fs::File::create(path)
            .with_context(|| format!("creating opus file: {}", path.display()))?;
        let mut writer = PacketWriter::new(file);

        // OpusHead (RFC 7845 §5.1) on its own BOS page.
        let mut head = Vec::with_capacity(19);
        head.extend_from_slice(b"OpusHead");
        head.push(1); // version
        head.push(1); // channel count
        head.extend_from_slice(&pre_skip.to_le_bytes());
        head.extend_from_slice(&16_000u32.to_le_bytes()); // original input rate (informational)
        head.extend_from_slice(&0i16.to_le_bytes()); // output gain
        head.push(0); // channel mapping family
        writer
            .write_packet(head, OPUS_SERIAL, PacketWriteEndInfo::EndPage, 0)
            .context("writing OpusHead")?;

        // OpusTags (RFC 7845 §5.2) on its own page.
        let vendor = b"noru";
        let mut tags = Vec::new();
        tags.extend_from_slice(b"OpusTags");
        tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
        tags.extend_from_slice(vendor);
        tags.extend_from_slice(&0u32.to_le_bytes()); // user comment count
        writer
            .write_packet(tags, OPUS_SERIAL, PacketWriteEndInfo::EndPage, 0)
            .context("writing OpusTags")?;

        Ok(Self {
            encoder,
            writer,
            pending: Vec::with_capacity(OPUS_FRAME * 2),
            granule: 0,
            scratch: vec![0u8; 4000],
        })
    }

    pub fn write_samples(&mut self, samples: &[f32]) -> Result<()> {
        self.pending.extend_from_slice(samples);
        while self.pending.len() >= OPUS_FRAME {
            let frame: Vec<f32> = self.pending.drain(..OPUS_FRAME).collect();
            self.encode_frame(&frame, false)?;
        }
        Ok(())
    }

    fn encode_frame(&mut self, frame: &[f32], end: bool) -> Result<()> {
        let n = self
            .encoder
            .encode_float(frame, &mut self.scratch)
            .map_err(|e| anyhow::anyhow!("opus encode: {e}"))?;
        self.granule += OPUS_GRANULE_STEP;
        let info = if end {
            PacketWriteEndInfo::EndStream
        } else {
            PacketWriteEndInfo::NormalPacket
        };
        let packet = self.scratch[..n].to_vec();
        self.writer
            .write_packet(packet, OPUS_SERIAL, info, self.granule)
            .context("writing opus packet")?;
        Ok(())
    }

    pub fn finalize(mut self) -> Result<()> {
        // Encode any remainder (zero-padded) as the final, end-of-stream frame.
        let mut frame = std::mem::take(&mut self.pending);
        frame.resize(OPUS_FRAME, 0.0);
        self.encode_frame(&frame, true)?;
        let mut file = self.writer.into_inner();
        file.flush().context("flushing opus file")?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_writer_produces_valid_ogg_opus() {
        // 3s of a 440Hz tone at 16k mono.
        let n = 16_000 * 3;
        let samples: Vec<f32> = (0..n)
            .map(|i| (i as f32 * 440.0 * 2.0 * std::f32::consts::PI / 16_000.0).sin() * 0.5)
            .collect();

        let path = std::env::temp_dir().join("noru-opus-test.opus");
        let mut w = OpusWriter::new(&path, WHISPER_SAMPLE_RATE).expect("create");
        // Feed in irregular chunks to exercise frame buffering.
        for chunk in samples.chunks(777) {
            w.write_samples(chunk).expect("write");
        }
        w.finalize().expect("finalize");

        let bytes = std::fs::read(&path).expect("read back");
        // Ogg page capture pattern + Opus identification header.
        assert_eq!(&bytes[0..4], b"OggS", "missing Ogg capture pattern");
        assert!(
            bytes.windows(8).any(|win| win == b"OpusHead"),
            "missing OpusHead"
        );
        assert!(
            bytes.windows(8).any(|win| win == b"OpusTags"),
            "missing OpusTags"
        );
        // 3s of raw 16k mono f32 = 192_000 bytes; at 24kbps Opus is ~9KB.
        assert!(
            bytes.len() < 30_000,
            "opus not compressing as expected: {} bytes",
            bytes.len()
        );
        let _ = std::fs::remove_file(&path);
    }
}
