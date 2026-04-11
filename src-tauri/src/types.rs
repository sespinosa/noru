use serde::{Deserialize, Serialize};

pub type MeetingId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Zoom,
    Meet,
    Teams,
    Slack,
    Discord,
    Webex,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMeeting {
    pub started_at: String,
    pub ended_at: Option<String>,
    pub platform: Option<Platform>,
    pub audio_path: Option<String>,
    pub segments: Vec<TranscriptSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meeting {
    pub id: MeetingId,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub platform: Option<Platform>,
    pub audio_path: Option<String>,
    pub segments: Vec<TranscriptSegment>,
    pub summary: Option<String>,
    pub action_items: Option<Vec<String>>,
    pub key_decisions: Option<Vec<String>>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingSummary {
    pub id: MeetingId,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub platform: Option<Platform>,
    pub duration_ms: Option<i64>,
    pub word_count: usize,
    pub has_summary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingState {
    pub in_meeting: bool,
    pub platform: Option<Platform>,
    pub confidence: f32,
    pub since: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum MeetingStateChange {
    Started { state: MeetingState },
    Ended { state: MeetingState },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AuthStatus {
    SignedOut,
    Refreshing,
    Signed { account_email: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthFlowHandle {
    pub flow_id: String,
    pub authorize_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RecordingState {
    Idle,
    Recording { meeting_id: MeetingId },
    Transcribing { meeting_id: MeetingId },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDownloadProgress {
    pub model: String,
    pub percent: u8,
    pub downloaded: u64,
    pub total: Option<u64>,
}
