use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub id: i64,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub platform: Option<String>,
    pub title: Option<String>,
    pub audio_path: Option<String>,
    pub segments: Vec<TranscriptSegment>,
    pub summary: Option<String>,
    pub action_items: Option<Vec<String>>,
    pub key_decisions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSummary {
    pub id: i64,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub platform: Option<String>,
    pub title: Option<String>,
    pub duration_ms: Option<i64>,
    pub word_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingState {
    Idle,
    Recording { transcript_id: i64 },
    Transcribing { transcript_id: i64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedMeeting {
    pub platform: String,
    pub process_name: String,
    pub window_title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStatus {
    pub signed_in: bool,
    pub email: Option<String>,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub general: GeneralSettings,
    pub recording: RecordingSettings,
    pub whisper: WhisperSettings,
    pub ai: AiSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralSettings {
    pub autostart: bool,
    pub transcripts_dir: String,
    pub theme: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingSettings {
    pub enabled_platforms: Vec<String>,
    pub input_device: Option<String>,
    pub system_audio_device: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperSettings {
    pub model: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSettings {
    pub enabled: bool,
}
