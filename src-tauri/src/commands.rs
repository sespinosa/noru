use tauri::ipc::InvokeError;

use crate::types::{AuthStatus, DetectedMeeting, RecordingState, Settings, Transcript, TranscriptSummary};

fn err(e: anyhow::Error) -> InvokeError {
    InvokeError::from(e.to_string())
}

#[tauri::command]
pub async fn list_transcripts() -> Result<Vec<TranscriptSummary>, InvokeError> {
    tokio::task::spawn_blocking(|| {
        let storage = crate::storage::Storage::open().map_err(err)?;
        storage.list_transcripts().map_err(err)
    })
    .await
    .map_err(|e| InvokeError::from(e.to_string()))?
}

#[tauri::command]
pub async fn get_transcript(id: i64) -> Result<Transcript, InvokeError> {
    tokio::task::spawn_blocking(move || {
        let storage = crate::storage::Storage::open().map_err(err)?;
        storage.get_transcript(id).map_err(err)
    })
    .await
    .map_err(|e| InvokeError::from(e.to_string()))?
}

#[tauri::command]
pub async fn delete_transcript(id: i64) -> Result<(), InvokeError> {
    tokio::task::spawn_blocking(move || {
        let storage = crate::storage::Storage::open().map_err(err)?;
        storage.delete_transcript(id).map_err(err)
    })
    .await
    .map_err(|e| InvokeError::from(e.to_string()))?
}

#[tauri::command]
pub async fn recording_state() -> Result<RecordingState, InvokeError> {
    Ok(RecordingState::Idle)
}

#[tauri::command]
pub async fn start_recording() -> Result<RecordingState, InvokeError> {
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

#[tauri::command]
pub async fn detect_meeting() -> Result<Option<DetectedMeeting>, InvokeError> {
    tokio::task::spawn_blocking(|| {
        let detector = crate::detect::Detector::new();
        detector.poll().map_err(err)
    })
    .await
    .map_err(|e| InvokeError::from(e.to_string()))?
}

#[tauri::command]
pub async fn auth_status() -> Result<AuthStatus, InvokeError> {
    tokio::task::spawn_blocking(|| {
        let auth = crate::auth::Auth::new();
        auth.status().map_err(err)
    })
    .await
    .map_err(|e| InvokeError::from(e.to_string()))?
}

#[tauri::command]
pub async fn auth_sign_in() -> Result<String, InvokeError> {
    tokio::task::spawn_blocking(|| {
        let auth = crate::auth::Auth::new();
        auth.start_sign_in().map_err(err)
    })
    .await
    .map_err(|e| InvokeError::from(e.to_string()))?
}

#[tauri::command]
pub async fn auth_sign_out() -> Result<(), InvokeError> {
    tokio::task::spawn_blocking(|| {
        let auth = crate::auth::Auth::new();
        auth.sign_out().map_err(err)
    })
    .await
    .map_err(|e| InvokeError::from(e.to_string()))?
}

#[tauri::command]
pub async fn ai_summarize(transcript_id: i64) -> Result<String, InvokeError> {
    tokio::task::spawn_blocking(move || {
        let storage = crate::storage::Storage::open().map_err(err)?;
        let transcript = storage.get_transcript(transcript_id).map_err(err)?;
        let auth = crate::auth::Auth::new();
        let ai = crate::ai::Ai::new(&auth);
        let result = ai.summarize(&transcript).map_err(err)?;
        storage
            .save_ai_result(transcript_id, crate::storage::AiResultKind::Summary, &result)
            .map_err(err)?;
        Ok(result)
    })
    .await
    .map_err(|e| InvokeError::from(e.to_string()))?
}

#[tauri::command]
pub async fn ai_action_items(transcript_id: i64) -> Result<Vec<String>, InvokeError> {
    tokio::task::spawn_blocking(move || {
        let storage = crate::storage::Storage::open().map_err(err)?;
        let transcript = storage.get_transcript(transcript_id).map_err(err)?;
        let auth = crate::auth::Auth::new();
        let ai = crate::ai::Ai::new(&auth);
        let result = ai.action_items(&transcript).map_err(err)?;
        let joined = serde_json::to_string(&result)
            .map_err(|e| InvokeError::from(e.to_string()))?;
        storage
            .save_ai_result(
                transcript_id,
                crate::storage::AiResultKind::ActionItems,
                &joined,
            )
            .map_err(err)?;
        Ok(result)
    })
    .await
    .map_err(|e| InvokeError::from(e.to_string()))?
}

#[tauri::command]
pub async fn ai_key_decisions(transcript_id: i64) -> Result<Vec<String>, InvokeError> {
    tokio::task::spawn_blocking(move || {
        let storage = crate::storage::Storage::open().map_err(err)?;
        let transcript = storage.get_transcript(transcript_id).map_err(err)?;
        let auth = crate::auth::Auth::new();
        let ai = crate::ai::Ai::new(&auth);
        let result = ai.key_decisions(&transcript).map_err(err)?;
        let joined = serde_json::to_string(&result)
            .map_err(|e| InvokeError::from(e.to_string()))?;
        storage
            .save_ai_result(
                transcript_id,
                crate::storage::AiResultKind::KeyDecisions,
                &joined,
            )
            .map_err(err)?;
        Ok(result)
    })
    .await
    .map_err(|e| InvokeError::from(e.to_string()))?
}

#[tauri::command]
pub async fn get_settings() -> Result<Settings, InvokeError> {
    Err(InvokeError::from(
        "get_settings not implemented (Phase 2 frontend teammate stores via storage)".to_string(),
    ))
}

#[tauri::command]
pub async fn save_settings(_settings: Settings) -> Result<(), InvokeError> {
    Err(InvokeError::from(
        "save_settings not implemented (Phase 2 frontend teammate stores via storage)".to_string(),
    ))
}

