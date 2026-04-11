use tauri::ipc::InvokeError;

use crate::storage;
use crate::types::{
    AudioDevice, AuthFlowHandle, AuthStatus, Meeting, MeetingId, MeetingState, MeetingSummary,
    Platform, RecordingState,
};

fn err(e: anyhow::Error) -> InvokeError {
    InvokeError::from(e.to_string())
}

async fn blocking<T, F>(f: F) -> Result<T, InvokeError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, InvokeError> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| InvokeError::from(e.to_string()))?
}

// ---------- meetings / storage ----------

#[tauri::command]
pub async fn list_meetings(limit: usize, offset: usize) -> Result<Vec<MeetingSummary>, InvokeError> {
    blocking(move || storage::list_meetings(limit, offset).map_err(err)).await
}

#[tauri::command]
pub async fn get_meeting(id: MeetingId) -> Result<Option<Meeting>, InvokeError> {
    blocking(move || storage::get_meeting(&id).map_err(err)).await
}

#[tauri::command]
pub async fn delete_meeting(id: MeetingId) -> Result<(), InvokeError> {
    blocking(move || storage::delete_meeting(&id).map_err(err)).await
}

// ---------- detection ----------

#[tauri::command]
pub async fn detect_poll() -> Result<MeetingState, InvokeError> {
    blocking(|| crate::detect::poll().map_err(err)).await
}

// ---------- recording (Phase 3 orchestrator owns the state machine) ----------

#[tauri::command]
pub async fn recording_state() -> Result<RecordingState, InvokeError> {
    Ok(RecordingState::Idle)
}

#[tauri::command]
pub async fn start_recording(_manual: bool) -> Result<RecordingState, InvokeError> {
    Err(InvokeError::from(
        "start_recording not implemented (Phase 3 orchestrator)".to_string(),
    ))
}

#[tauri::command]
pub async fn stop_recording() -> Result<RecordingState, InvokeError> {
    Err(InvokeError::from(
        "stop_recording not implemented (Phase 3 orchestrator)".to_string(),
    ))
}

// ---------- auth ----------

#[tauri::command]
pub async fn auth_status() -> Result<AuthStatus, InvokeError> {
    blocking(|| crate::auth::status().map_err(err)).await
}

#[tauri::command]
pub async fn auth_start_login() -> Result<AuthFlowHandle, InvokeError> {
    blocking(|| crate::auth::start_login().map_err(err)).await
}

#[tauri::command]
pub async fn auth_sign_out() -> Result<(), InvokeError> {
    blocking(|| crate::auth::sign_out().map_err(err)).await
}

// ---------- ai ----------

#[tauri::command]
pub async fn ai_summarize(meeting_id: MeetingId) -> Result<String, InvokeError> {
    blocking(move || {
        let meeting = storage::get_meeting(&meeting_id)
            .map_err(err)?
            .ok_or_else(|| InvokeError::from(format!("meeting {meeting_id} not found")))?;
        let transcript_text = transcript_to_text(&meeting);
        let result = crate::ai::summarize(&transcript_text).map_err(err)?;
        storage::update_summary(&meeting_id, &result).map_err(err)?;
        Ok(result)
    })
    .await
}

#[tauri::command]
pub async fn ai_extract_action_items(meeting_id: MeetingId) -> Result<Vec<String>, InvokeError> {
    blocking(move || {
        let meeting = storage::get_meeting(&meeting_id)
            .map_err(err)?
            .ok_or_else(|| InvokeError::from(format!("meeting {meeting_id} not found")))?;
        let transcript_text = transcript_to_text(&meeting);
        let result = crate::ai::extract_action_items(&transcript_text).map_err(err)?;
        storage::update_action_items(&meeting_id, &result).map_err(err)?;
        Ok(result)
    })
    .await
}

#[tauri::command]
pub async fn ai_extract_key_decisions(meeting_id: MeetingId) -> Result<Vec<String>, InvokeError> {
    blocking(move || {
        let meeting = storage::get_meeting(&meeting_id)
            .map_err(err)?
            .ok_or_else(|| InvokeError::from(format!("meeting {meeting_id} not found")))?;
        let transcript_text = transcript_to_text(&meeting);
        let result = crate::ai::extract_key_decisions(&transcript_text).map_err(err)?;
        storage::update_key_decisions(&meeting_id, &result).map_err(err)?;
        Ok(result)
    })
    .await
}

fn transcript_to_text(meeting: &Meeting) -> String {
    meeting
        .segments
        .iter()
        .map(|s| s.text.trim())
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------- settings (lead-owned stubs, filled in Phase 3) ----------

#[tauri::command]
pub async fn get_autostart() -> Result<bool, InvokeError> {
    Err(InvokeError::from(
        "get_autostart not implemented (Phase 3 polish)".to_string(),
    ))
}

#[tauri::command]
pub async fn set_autostart(_enabled: bool) -> Result<(), InvokeError> {
    Err(InvokeError::from(
        "set_autostart not implemented (Phase 3 polish)".to_string(),
    ))
}

#[tauri::command]
pub async fn list_audio_input_devices() -> Result<Vec<AudioDevice>, InvokeError> {
    Err(InvokeError::from(
        "list_audio_input_devices not implemented (Phase 3 polish)".to_string(),
    ))
}

#[tauri::command]
pub async fn known_platforms() -> Result<Vec<Platform>, InvokeError> {
    Ok(crate::detect::known_platforms().to_vec())
}

#[tauri::command]
pub async fn download_model(_model: String) -> Result<(), InvokeError> {
    Err(InvokeError::from(
        "download_model not implemented (Phase 3 polish)".to_string(),
    ))
}
