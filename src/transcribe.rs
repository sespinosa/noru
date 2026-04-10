use anyhow::{Context, Result};
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct Segment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

impl std::fmt::Display for Segment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let start = format_timestamp(self.start_ms);
        let end = format_timestamp(self.end_ms);
        write!(f, "[{start} -> {end}]{}", self.text)
    }
}

fn format_timestamp(ms: i64) -> String {
    let secs = ms / 1000;
    let millis = ms % 1000;
    let mins = secs / 60;
    let secs = secs % 60;
    format!("{mins:02}:{secs:02}.{millis:03}")
}

pub struct WhisperEngine {
    ctx: WhisperContext,
    language: Option<String>,
}

impl WhisperEngine {
    /// Load a Whisper model from a ggml .bin file.
    pub fn new(model_path: &Path, language: Option<String>) -> Result<Self> {
        eprintln!("Loading Whisper model: {}", model_path.display());

        let ctx = WhisperContext::new_with_params(
            model_path.to_str().context("Invalid model path")?,
            WhisperContextParameters::default(),
        )
        .map_err(|e| anyhow::anyhow!("Failed to load Whisper model: {e}"))?;

        eprintln!("Model loaded.");

        Ok(Self { ctx, language })
    }

    /// Transcribe 16kHz mono f32 audio samples.
    /// `time_offset_ms` shifts all timestamps by this amount (for chunked transcription).
    pub fn transcribe(&self, samples: &[f32], time_offset_ms: i64) -> Result<Vec<Segment>> {
        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| anyhow::anyhow!("Failed to create Whisper state: {e}"))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        if let Some(ref lang) = self.language {
            if lang != "auto" {
                params.set_language(Some(lang));
            }
        }

        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_no_speech_thold(0.6);
        params.set_suppress_blank(true);

        state
            .full(params, samples)
            .map_err(|e| anyhow::anyhow!("Whisper transcription failed: {e}"))?;

        let n = state
            .full_n_segments()
            .map_err(|e| anyhow::anyhow!("Failed to get segment count: {e}"))?;

        let mut segments = Vec::with_capacity(n as usize);

        for i in 0..n {
            let text = state
                .full_get_segment_text(i)
                .map_err(|e| anyhow::anyhow!("Failed to get segment text: {e}"))?;

            let start = state
                .full_get_segment_t0(i)
                .map_err(|e| anyhow::anyhow!("Failed to get segment t0: {e}"))?;

            let end = state
                .full_get_segment_t1(i)
                .map_err(|e| anyhow::anyhow!("Failed to get segment t1: {e}"))?;

            // whisper timestamps are in centiseconds (10ms units)
            segments.push(Segment {
                start_ms: (start as i64) * 10 + time_offset_ms,
                end_ms: (end as i64) * 10 + time_offset_ms,
                text,
            });
        }

        Ok(segments)
    }
}
