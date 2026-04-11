use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

const BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

pub const AVAILABLE_MODELS: &[&str] = &[
    "tiny",
    "base",
    "small",
    "medium",
    "large-v3",
    "large-v3-turbo",
];

/// Progress update during model download.
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub percent: u8,
}

fn models_dir() -> Result<PathBuf> {
    let dir = dirs::home_dir()
        .context("Could not determine home directory")?
        .join(".noru")
        .join("models");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn model_filename(name: &str) -> String {
    format!("ggml-{name}.bin")
}

/// Resolve a model name to a local path, downloading if needed.
/// `on_progress` is called with download progress updates.
pub fn resolve(
    name: &str,
    on_progress: impl Fn(DownloadProgress),
) -> Result<PathBuf> {
    // If it's already a file path, use it directly
    let as_path = PathBuf::from(name);
    if as_path.exists() {
        return Ok(as_path);
    }

    if !AVAILABLE_MODELS.contains(&name) {
        anyhow::bail!(
            "Unknown model '{name}'. Available: {}",
            AVAILABLE_MODELS.join(", ")
        );
    }

    let filename = model_filename(name);
    let path = models_dir()?.join(&filename);

    if path.exists() {
        return Ok(path);
    }

    let url = format!("{BASE_URL}/{filename}");
    eprintln!("Downloading model '{name}' from {url}");

    download(&url, &path, on_progress)?;

    eprintln!("Model saved to: {}", path.display());
    Ok(path)
}

fn download(
    url: &str,
    dest: &PathBuf,
    on_progress: impl Fn(DownloadProgress),
) -> Result<()> {
    let resp = ureq::get(url)
        .call()
        .context("Failed to download model")?;

    let total = resp
        .header("content-length")
        .and_then(|v| v.parse::<u64>().ok());

    let mut reader = resp.into_reader();
    let tmp = dest.with_extension("bin.tmp");
    let mut file = fs::File::create(&tmp)?;

    let mut downloaded: u64 = 0;
    let mut buf = vec![0u8; 1024 * 1024];
    let mut last_pct: u8 = 0;

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        downloaded += n as u64;

        let pct = total.map_or(0, |t| (downloaded * 100 / t) as u8);
        if pct != last_pct {
            on_progress(DownloadProgress {
                downloaded,
                total,
                percent: pct,
            });
            last_pct = pct;
        }
    }

    file.flush()?;
    fs::rename(&tmp, dest)?;
    Ok(())
}
