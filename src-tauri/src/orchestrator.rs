//! Phase 3 — the glue between `detect` events, `audio` capture, `transcribe`
//! (Whisper), and `storage`. Owns an explicit FSM; no race hand-waving.
//!
//! ```text
//!   Idle ─manual start / auto Started──▶ Recording ─manual stop / auto Ended──▶ Transcribing ─persist ok──▶ Idle
//!    ▲                                                                                 │
//!    └──────────────────────────── error / done ───────────────────────────────────────┘
//! ```
//!
//! Thread topology:
//! - `detect::start` spawns its own polling thread and invokes our callback on
//!   debounced state transitions.
//! - Audio capture runs on a dedicated thread (cpal `Stream` is `!Send`).
//! - Whisper + persist runs on a blocking thread (CPU-heavy, must not touch
//!   the Tauri async runtime).
//! - All three talk to the orchestrator through `Arc<Mutex<Inner>>`.
//!
//! Unit tests live at the bottom and cover the pure `decide` function — the
//! transition table without any real threads, audio, or sqlite.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use tauri::{AppHandle, Emitter};

use crate::audio::{AudioCapture, WavWriter};
use crate::detect::{self, DetectHandle};
use crate::models;
use crate::storage;
use crate::transcribe::WhisperEngine;
use crate::types::{
    MeetingId, MeetingStateChange, NewMeeting, RecordingState, TranscriptSegment,
};

const DEFAULT_WHISPER_MODEL: &str = "base";
const CAPTURE_CHUNK_SECS: f32 = 0.5;
const RECORDING_STATE_EVENT: &str = "recording://state";
const RECORDING_ERROR_EVENT: &str = "recording://error";

/// Process-wide cached Whisper engine. Loading the model is expensive
/// (hundreds of MB on disk + CPU init), so we memoize the first successful
/// load and reuse it for every subsequent transcription.
static WHISPER_ENGINE: OnceLock<Mutex<Option<WhisperEngine>>> = OnceLock::new();

/// The thing Tauri manages via `State<Orchestrator>`. Cheap to clone — holds
/// an `AppHandle` and an `Arc<Mutex<Inner>>`.
pub struct Orchestrator {
    inner: Arc<Mutex<Inner>>,
    app: AppHandle,
}

impl Clone for Orchestrator {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            app: self.app.clone(),
        }
    }
}

struct Inner {
    state: SessionState,
    detect_handle: Option<DetectHandle>,
}

enum SessionState {
    Idle,
    Recording(Recording),
    Transcribing(Transcribing),
}

struct Recording {
    /// Orchestrator-local unique tag used as the audio-file basename and the
    /// `RecordingState::Recording { meeting_id }` payload. NOT the storage
    /// primary key — storage allocates its own id in `save_meeting`.
    tag: MeetingId,
    started_at: String,
    audio_path: PathBuf,
    /// `true` if this recording was started by a detect `Started` event and
    /// should be auto-stopped on the matching `Ended`. Manual recordings are
    /// only stopped by an explicit `stop_recording` command.
    auto_started: bool,
    stop_signal: Arc<AtomicBool>,
    samples_rx: Receiver<Result<Vec<f32>>>,
    _capture_thread: JoinHandle<()>,
}

struct Transcribing {
    tag: MeetingId,
}

impl Orchestrator {
    pub fn new(app: AppHandle) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                state: SessionState::Idle,
                detect_handle: None,
            })),
            app,
        }
    }

    pub fn recording_state(&self) -> RecordingState {
        recording_state_from(&self.inner.lock().unwrap().state)
    }

    pub fn start_recording(&self, manual: bool) -> Result<RecordingState> {
        start_recording_impl(&self.inner, &self.app, manual)
    }

    pub fn stop_recording(&self) -> Result<RecordingState> {
        stop_recording_impl(&self.inner, &self.app)
    }

    /// Wire `detect::start` into the FSM so auto-recording can fire on
    /// meeting-detected events. Idempotent — calling twice is a no-op.
    ///
    /// Not called automatically at boot in Phase 3 MVP; gated on a prefs flag
    /// that Step 2 will add (`prefs::auto_detect`).
    pub fn start_auto_detect(&self) -> Result<()> {
        let mut guard = self.inner.lock().unwrap();
        if guard.detect_handle.is_some() {
            return Ok(());
        }
        let inner_clone = Arc::clone(&self.inner);
        let app_clone = self.app.clone();
        let handle = detect::start(move |change| {
            on_detect_change(&inner_clone, &app_clone, change);
        })
        .context("starting detect poller")?;
        guard.detect_handle = Some(handle);
        Ok(())
    }

    pub fn stop_auto_detect(&self) {
        let mut guard = self.inner.lock().unwrap();
        if let Some(handle) = guard.detect_handle.take() {
            handle.stop();
        }
    }
}

// ---------------------------------------------------------------------------
// Core transition implementations — all take `&Arc<Mutex<Inner>>` so both the
// Tauri command handlers AND the detect callback can reach them.
// ---------------------------------------------------------------------------

fn start_recording_impl(
    inner: &Arc<Mutex<Inner>>,
    app: &AppHandle,
    manual: bool,
) -> Result<RecordingState> {
    // Cheap early check without holding the lock across the capture spawn.
    {
        let guard = inner.lock().unwrap();
        if !matches!(guard.state, SessionState::Idle) {
            return Err(anyhow!(
                "cannot start recording — already {}",
                state_label(&guard.state)
            ));
        }
    }

    let tag = orchestrator_tag();
    let started_at = now_iso8601();
    let audio_path = audio_path_for(&tag)?;
    if let Some(parent) = audio_path.parent() {
        std::fs::create_dir_all(parent).context("creating ~/.noru/audio directory")?;
    }

    let stop_signal = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop_signal);
    let (ready_tx, ready_rx) = mpsc::channel::<Result<()>>();
    let (samples_tx, samples_rx) = mpsc::channel::<Result<Vec<f32>>>();
    let audio_path_thread = audio_path.clone();

    // Spawn the capture thread. It signals readiness via `ready_tx` once
    // cpal's stream is live; errors during Stream::build travel through the
    // same channel so start_recording_impl can fail cleanly instead of
    // transitioning into a broken Recording state.
    let capture_thread = thread::Builder::new()
        .name(format!("noru-capture-{tag}"))
        .spawn(move || {
            let result = capture_loop(audio_path_thread, stop_clone, ready_tx);
            let _ = samples_tx.send(result);
        })
        .context("spawning audio capture thread")?;

    match ready_rx.recv() {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            return Err(e).context("audio capture failed to start");
        }
        Err(_) => {
            return Err(anyhow!(
                "audio capture thread exited before signalling ready"
            ));
        }
    }

    // Commit state. Defensive re-check in case someone else raced us.
    {
        let mut guard = inner.lock().unwrap();
        if !matches!(guard.state, SessionState::Idle) {
            stop_signal.store(true, Ordering::Relaxed);
            return Err(anyhow!(
                "state raced — became {} while starting recording",
                state_label(&guard.state)
            ));
        }
        guard.state = SessionState::Recording(Recording {
            tag: tag.clone(),
            started_at,
            audio_path,
            auto_started: !manual,
            stop_signal,
            samples_rx,
            _capture_thread: capture_thread,
        });
    }

    let state = RecordingState::Recording {
        meeting_id: tag,
    };
    let _ = app.emit(RECORDING_STATE_EVENT, &state);
    Ok(state)
}

fn stop_recording_impl(
    inner: &Arc<Mutex<Inner>>,
    app: &AppHandle,
) -> Result<RecordingState> {
    // Pull the Recording out under the lock so the slow samples_rx.recv
    // happens without holding it.
    let recording = {
        let mut guard = inner.lock().unwrap();
        match std::mem::replace(&mut guard.state, SessionState::Idle) {
            SessionState::Recording(r) => r,
            other => {
                let label = state_label(&other);
                guard.state = other;
                return Err(anyhow!("cannot stop recording — {}", label));
            }
        }
    };

    // Tell the capture loop to finalize the WAV and exit.
    recording.stop_signal.store(true, Ordering::Relaxed);

    let samples = recording
        .samples_rx
        .recv()
        .map_err(|_| anyhow!("capture thread disconnected before delivering samples"))??;

    // Transition into Transcribing and emit immediately so the UI can show the
    // spinner while whisper grinds.
    {
        let mut guard = inner.lock().unwrap();
        guard.state = SessionState::Transcribing(Transcribing {
            tag: recording.tag.clone(),
        });
    }
    let state = RecordingState::Transcribing {
        meeting_id: recording.tag.clone(),
    };
    let _ = app.emit(RECORDING_STATE_EVENT, &state);

    // Whisper + persist on a dedicated thread. Whisper is CPU-heavy and must
    // not block the Tauri async runtime.
    let inner_clone = Arc::clone(inner);
    let app_clone = app.clone();
    let started_at = recording.started_at.clone();
    let audio_path = recording.audio_path.clone();
    let tag = recording.tag.clone();
    thread::Builder::new()
        .name(format!("noru-transcribe-{tag}"))
        .spawn(move || {
            let result = transcribe_and_persist(&started_at, &audio_path, &samples);

            // Always return to Idle after Transcribing, success or failure.
            {
                let mut guard = inner_clone.lock().unwrap();
                guard.state = SessionState::Idle;
            }

            match result {
                Ok(_) => {
                    let _ = app_clone.emit(RECORDING_STATE_EVENT, &RecordingState::Idle);
                }
                Err(e) => {
                    let msg = format!("{e:#}");
                    let _ = app_clone.emit(RECORDING_ERROR_EVENT, &msg);
                    let _ = app_clone.emit(RECORDING_STATE_EVENT, &RecordingState::Idle);
                }
            }
        })
        .context("spawning transcribe thread")?;

    Ok(state)
}

// ---------------------------------------------------------------------------
// Audio capture loop — owns cpal's !Send Stream and writes a WAV file.
// ---------------------------------------------------------------------------

fn capture_loop(
    audio_path: PathBuf,
    stop: Arc<AtomicBool>,
    ready: mpsc::Sender<Result<()>>,
) -> Result<Vec<f32>> {
    let capture = match AudioCapture::start() {
        Ok(c) => c,
        Err(e) => {
            let _ = ready.send(Err(e));
            return Err(anyhow!("audio capture start failed (see ready channel)"));
        }
    };
    let writer = match WavWriter::new(&audio_path, capture.device_sample_rate()) {
        Ok(w) => w,
        Err(e) => {
            let _ = ready.send(Err(e));
            return Err(anyhow!("wav writer create failed (see ready channel)"));
        }
    };

    let _ = ready.send(Ok(()));

    let mut whisper_samples = Vec::new();
    while !stop.load(Ordering::Relaxed) {
        match capture.collect_chunk(CAPTURE_CHUNK_SECS) {
            Ok(chunk) => {
                if let Err(e) = writer.write_samples(&chunk.raw_samples) {
                    let _ = writer.finalize();
                    return Err(e).context("writing audio samples to WAV");
                }
                whisper_samples.extend(chunk.whisper_samples);
            }
            Err(e) => {
                let _ = writer.finalize();
                return Err(e).context("collecting audio chunk");
            }
        }
    }
    writer.finalize().context("finalizing WAV")?;
    Ok(whisper_samples)
}

// ---------------------------------------------------------------------------
// Transcribe + persist — runs on a blocking thread.
// ---------------------------------------------------------------------------

fn transcribe_and_persist(
    started_at: &str,
    audio_path: &PathBuf,
    samples: &[f32],
) -> Result<()> {
    let segments = if samples.is_empty() {
        // WSL / headless — no real audio was captured. Persist an empty
        // transcript so the UI still shows the row; don't invoke Whisper.
        Vec::new()
    } else {
        transcribe(samples).context("running whisper on captured samples")?
    };

    let ended_at = now_iso8601();

    let new_meeting = NewMeeting {
        started_at: started_at.to_string(),
        ended_at: Some(ended_at),
        // Phase 3 Step 1 doesn't yet plumb the detected platform from the
        // `Started` event down to the orchestrator — Step 2 or a follow-up
        // widens `Recording` to carry it.
        platform: None,
        audio_path: Some(audio_path.to_string_lossy().into_owned()),
        segments,
    };

    storage::save_meeting(new_meeting).context("persisting meeting to storage")?;
    Ok(())
}

fn transcribe(samples: &[f32]) -> Result<Vec<TranscriptSegment>> {
    let cell = WHISPER_ENGINE.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().map_err(|_| anyhow!("whisper mutex poisoned"))?;
    if guard.is_none() {
        let model_path =
            models::resolve(DEFAULT_WHISPER_MODEL, |_| {}).context("resolving whisper model")?;
        let engine = WhisperEngine::new(&model_path, None).context("loading whisper engine")?;
        *guard = Some(engine);
    }
    let engine = guard.as_ref().expect("whisper engine just populated");
    let raw = engine.transcribe(samples, 0).context("whisper transcribe")?;
    Ok(raw
        .into_iter()
        .map(|s| TranscriptSegment {
            start_ms: s.start_ms,
            end_ms: s.end_ms,
            text: s.text,
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Detect callback — thin wrapper over `decide` + the two impl functions.
// ---------------------------------------------------------------------------

fn on_detect_change(
    inner: &Arc<Mutex<Inner>>,
    app: &AppHandle,
    change: MeetingStateChange,
) {
    let action = {
        let guard = inner.lock().unwrap();
        decide(&guard.state, &change)
    };

    match action {
        FsmAction::StartAutoRecording => {
            if let Err(e) = start_recording_impl(inner, app, false) {
                let _ = app.emit(RECORDING_ERROR_EVENT, format!("{e:#}"));
            }
        }
        FsmAction::StopAutoRecording => {
            if let Err(e) = stop_recording_impl(inner, app) {
                let _ = app.emit(RECORDING_ERROR_EVENT, format!("{e:#}"));
            }
        }
        FsmAction::Ignore => {}
    }
}

#[derive(Debug, PartialEq, Eq)]
enum FsmAction {
    StartAutoRecording,
    StopAutoRecording,
    Ignore,
}

/// Pure transition function. Given the current session state and an incoming
/// detect event, decide what action (if any) the orchestrator should take.
///
/// Transition rules:
/// 1. `Idle + Started` → start an auto-recording
/// 2. `Recording(auto) + Ended` → stop the recording and begin transcribing
/// 3. `Recording(manual) + Ended` → ignore; manual recordings only stop on
///    an explicit `stop_recording` command
/// 4. `Recording + Started` → ignore; multi-meeting support is a v1.1 feature
/// 5. `Transcribing + *` → ignore; late detect events are stale by the time
///    transcription is running
/// 6. `Idle + Ended` → ignore; we weren't recording, there's nothing to stop
fn decide(state: &SessionState, change: &MeetingStateChange) -> FsmAction {
    match (state, change) {
        (SessionState::Idle, MeetingStateChange::Started { .. }) => FsmAction::StartAutoRecording,
        (SessionState::Recording(r), MeetingStateChange::Ended { .. }) if r.auto_started => {
            FsmAction::StopAutoRecording
        }
        _ => FsmAction::Ignore,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn recording_state_from(state: &SessionState) -> RecordingState {
    match state {
        SessionState::Idle => RecordingState::Idle,
        SessionState::Recording(r) => RecordingState::Recording {
            meeting_id: r.tag.clone(),
        },
        SessionState::Transcribing(t) => RecordingState::Transcribing {
            meeting_id: t.tag.clone(),
        },
    }
}

fn state_label(state: &SessionState) -> &'static str {
    match state {
        SessionState::Idle => "idle",
        SessionState::Recording(_) => "recording",
        SessionState::Transcribing(_) => "transcribing",
    }
}

/// Orchestrator-local unique tag for one recording session. Used as the audio
/// filename and as the `RecordingState` payload. NOT the storage primary key —
/// that's allocated inside `storage::save_meeting`.
fn orchestrator_tag() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:032x}")
}

fn audio_path_for(tag: &str) -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("cannot resolve home directory"))?;
    Ok(home.join(".noru").join("audio").join(format!("{tag}.wav")))
}

fn now_iso8601() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    epoch_to_iso8601(secs)
}

/// Howard Hinnant civil_from_days algorithm. Same formatter `detect.rs` uses;
/// keeping a local copy so the orchestrator doesn't need pub(crate) leakage
/// from detect and the Phase-1 dep lock stays intact (no chrono/time).
fn epoch_to_iso8601(timestamp: i64) -> String {
    let days = timestamp.div_euclid(86_400);
    let secs_of_day = timestamp.rem_euclid(86_400);
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day / 60) % 60;
    let second = secs_of_day % 60;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y_raw = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y_raw + 1 } else { y_raw };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, m, d, hour, minute, second
    )
}

// ---------------------------------------------------------------------------
// Tests — the FSM `decide` function is pure and testable without any threads,
// audio devices, or sqlite. The `mock_recording` helper fabricates a Recording
// session with placeholder channels so the test runner stays hermetic.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{MeetingState, Platform};

    fn zoom_state() -> MeetingState {
        MeetingState {
            in_meeting: true,
            platform: Some(Platform::Zoom),
            confidence: 1.0,
            since: None,
        }
    }

    fn idle_state() -> MeetingState {
        MeetingState {
            in_meeting: false,
            platform: None,
            confidence: 0.0,
            since: None,
        }
    }

    fn mock_recording(auto_started: bool) -> SessionState {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let (_tx, rx) = mpsc::channel::<Result<Vec<f32>>>();
        SessionState::Recording(Recording {
            tag: "test".into(),
            started_at: "2026-04-11T00:00:00Z".into(),
            audio_path: PathBuf::from("/tmp/noru-test.wav"),
            auto_started,
            stop_signal,
            samples_rx: rx,
            _capture_thread: thread::spawn(|| {}),
        })
    }

    #[test]
    fn idle_plus_started_starts_auto() {
        let state = SessionState::Idle;
        let change = MeetingStateChange::Started { state: zoom_state() };
        assert_eq!(decide(&state, &change), FsmAction::StartAutoRecording);
    }

    #[test]
    fn auto_recording_plus_ended_stops() {
        let state = mock_recording(true);
        let change = MeetingStateChange::Ended { state: idle_state() };
        assert_eq!(decide(&state, &change), FsmAction::StopAutoRecording);
    }

    #[test]
    fn manual_recording_ignores_ended() {
        let state = mock_recording(false);
        let change = MeetingStateChange::Ended { state: idle_state() };
        assert_eq!(decide(&state, &change), FsmAction::Ignore);
    }

    #[test]
    fn recording_ignores_new_started() {
        // Second Zoom meeting popping up while we're already mid-capture is a
        // v1.1 concern. Ignore it cleanly.
        let state = mock_recording(true);
        let change = MeetingStateChange::Started { state: zoom_state() };
        assert_eq!(decide(&state, &change), FsmAction::Ignore);
    }

    #[test]
    fn transcribing_ignores_stale_events() {
        let state = SessionState::Transcribing(Transcribing {
            tag: "test".into(),
        });
        assert_eq!(
            decide(&state, &MeetingStateChange::Ended { state: idle_state() }),
            FsmAction::Ignore
        );
        assert_eq!(
            decide(&state, &MeetingStateChange::Started { state: zoom_state() }),
            FsmAction::Ignore
        );
    }

    #[test]
    fn idle_ignores_unmatched_ended() {
        let state = SessionState::Idle;
        let change = MeetingStateChange::Ended { state: idle_state() };
        assert_eq!(decide(&state, &change), FsmAction::Ignore);
    }

    #[test]
    fn epoch_to_iso8601_anchors() {
        // Well-known unix anchors so the test doesn't depend on me doing
        // calendar math in my head.
        assert_eq!(epoch_to_iso8601(0), "1970-01-01T00:00:00Z");
        assert_eq!(epoch_to_iso8601(1_000_000_000), "2001-09-09T01:46:40Z");
        // 2026-04-11T00:00:00Z (verified against `date -u -d '2026-04-11' +%s`)
        assert_eq!(epoch_to_iso8601(1_775_865_600), "2026-04-11T00:00:00Z");
    }

    #[test]
    fn orchestrator_tag_is_32_hex_chars() {
        let tag = orchestrator_tag();
        assert_eq!(tag.len(), 32);
        assert!(tag.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
