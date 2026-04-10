mod audio;
mod models;
mod transcribe;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "noru")]
#[command(about = "Capture and transcribe audio using local Whisper")]
struct Cli {
    /// Whisper model name (tiny, base, small, medium, large-v3, large-v3-turbo) or path to .bin file
    #[arg(short, long, default_value = "base")]
    model: String,

    /// Transcribe an existing WAV file instead of live capture
    #[arg(short, long)]
    file: Option<PathBuf>,

    /// Output WAV file path (live capture mode)
    #[arg(short, long, default_value = "recording.wav")]
    output: PathBuf,

    /// Chunk duration in seconds for live transcription
    #[arg(long, default_value = "5")]
    chunk_secs: f32,

    /// Language code (e.g. "en", "es") or "auto" for detection
    #[arg(short, long, default_value = "auto")]
    language: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let lang = if cli.language == "auto" {
        None
    } else {
        Some(cli.language.clone())
    };

    let model_path = models::resolve(&cli.model, |p| {
        eprint!("\r  {}% ({}/{})", p.percent, p.downloaded, p.total.unwrap_or(0));
    })?;
    eprintln!();
    let engine = transcribe::WhisperEngine::new(&model_path, lang)?;

    if let Some(ref wav_path) = cli.file {
        transcribe_file(&engine, wav_path)?;
    } else {
        live_capture(&engine, &cli.output, cli.chunk_secs)?;
    }

    Ok(())
}

/// Transcribe an existing WAV file.
fn transcribe_file(engine: &transcribe::WhisperEngine, path: &PathBuf) -> Result<()> {
    eprintln!("Transcribing: {}", path.display());

    let samples = audio::load_wav(path)?;
    eprintln!("Loaded {} samples ({:.1}s at 16kHz)", samples.len(), samples.len() as f32 / 16000.0);

    let segments = engine.transcribe(&samples, 0)?;

    for seg in &segments {
        println!("{seg}");
    }

    eprintln!("Done. {} segments.", segments.len());
    Ok(())
}

/// Live capture: record audio, save to WAV, transcribe in chunks.
fn live_capture(
    engine: &transcribe::WhisperEngine,
    output_path: &PathBuf,
    chunk_secs: f32,
) -> Result<()> {
    let capture = audio::AudioCapture::start()?;
    let wav = audio::WavWriter::new(output_path, capture.device_sample_rate())?;

    eprintln!(
        "Recording. Chunk size: {chunk_secs}s. Saving to: {}",
        output_path.display()
    );
    eprintln!("Press Ctrl+C to stop.\n");

    // Handle Ctrl+C gracefully
    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let r = running.clone();
    ctrlc_handler(r);

    let mut elapsed_ms: i64 = 0;
    let chunk_ms = (chunk_secs * 1000.0) as i64;

    while running.load(std::sync::atomic::Ordering::Relaxed) {
        let chunk = capture.collect_chunk(chunk_secs)?;

        // Save raw audio to WAV
        wav.write_samples(&chunk.raw_samples)?;

        // Transcribe
        let segments = engine.transcribe(&chunk.whisper_samples, elapsed_ms)?;

        for seg in &segments {
            println!("{seg}");
        }

        elapsed_ms += chunk_ms;
    }

    eprintln!("\nStopping...");
    wav.finalize()?;
    eprintln!("Saved recording to: {}", output_path.display());

    Ok(())
}

fn ctrlc_handler(running: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    let _ = ctrlc::set_handler(move || {
        running.store(false, std::sync::atomic::Ordering::Relaxed);
    });
}
