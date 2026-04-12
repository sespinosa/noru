use tauri::ipc::InvokeError;
use tauri::Emitter;

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
pub async fn recording_state(
    orchestrator: tauri::State<'_, crate::orchestrator::Orchestrator>,
) -> Result<RecordingState, InvokeError> {
    Ok(orchestrator.recording_state())
}

#[tauri::command]
pub async fn start_recording(
    manual: bool,
    orchestrator: tauri::State<'_, crate::orchestrator::Orchestrator>,
) -> Result<RecordingState, InvokeError> {
    let orch = orchestrator.inner().clone();
    blocking(move || orch.start_recording(manual).map_err(err)).await
}

#[tauri::command]
pub async fn stop_recording(
    orchestrator: tauri::State<'_, crate::orchestrator::Orchestrator>,
) -> Result<RecordingState, InvokeError> {
    let orch = orchestrator.inner().clone();
    blocking(move || orch.stop_recording().map_err(err)).await
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

// ---------- settings ----------

#[tauri::command]
pub async fn get_autostart(
    app: tauri::AppHandle,
) -> Result<bool, InvokeError> {
    use tauri_plugin_autostart::ManagerExt;
    let mgr = app.autolaunch();
    mgr.is_enabled().map_err(|e| InvokeError::from(e.to_string()))
}

#[tauri::command]
pub async fn set_autostart(
    enabled: bool,
    app: tauri::AppHandle,
) -> Result<(), InvokeError> {
    use tauri_plugin_autostart::ManagerExt;
    let mgr = app.autolaunch();
    if enabled {
        mgr.enable().map_err(|e| InvokeError::from(e.to_string()))
    } else {
        mgr.disable().map_err(|e| InvokeError::from(e.to_string()))
    }
}

#[tauri::command]
pub async fn list_audio_input_devices() -> Result<Vec<AudioDevice>, InvokeError> {
    blocking(|| {
        use cpal::traits::{DeviceTrait, HostTrait};
        let host = cpal::default_host();
        let default_name = host
            .default_input_device()
            .and_then(|d| d.name().ok());
        let devices = host
            .input_devices()
            .map_err(|e| InvokeError::from(format!("enumerate audio devices: {e}")))?;
        let mut out = Vec::new();
        for d in devices {
            if let Ok(name) = d.name() {
                let is_default = default_name.as_deref() == Some(&name);
                out.push(AudioDevice { name, is_default });
            }
        }
        Ok(out)
    })
    .await
}

#[tauri::command]
pub async fn known_platforms() -> Result<Vec<Platform>, InvokeError> {
    Ok(crate::detect::known_platforms().to_vec())
}

#[tauri::command]
pub async fn download_model(
    model: String,
    app: tauri::AppHandle,
) -> Result<(), InvokeError> {
    use crate::types::ModelDownloadProgress;
    blocking(move || {
        let app_ref = &app;
        crate::models::resolve(&model, |p| {
            let _ = app_ref.emit(
                "models://download_progress",
                ModelDownloadProgress {
                    model: model.clone(),
                    percent: p.percent,
                    downloaded: p.downloaded,
                    total: p.total,
                },
            );
        })
        .map(|_| ())
        .map_err(err)
    })
    .await
}

// ---------- preferences ----------

#[tauri::command]
pub async fn get_preference(key: String) -> Result<Option<serde_json::Value>, InvokeError> {
    crate::prefs::get(&key).map_err(err)
}

#[tauri::command]
pub async fn set_preference(key: String, value: serde_json::Value) -> Result<(), InvokeError> {
    crate::prefs::set(&key, value).map_err(err)
}

#[tauri::command]
pub async fn list_preferences() -> Result<std::collections::HashMap<String, serde_json::Value>, InvokeError> {
    crate::prefs::list().map_err(err)
}

// ---------- dialog ----------

#[tauri::command]
pub async fn choose_folder(
    title: Option<String>,
    app: tauri::AppHandle,
) -> Result<Option<String>, InvokeError> {
    use tauri_plugin_dialog::DialogExt;
    let mut builder = app.dialog().file();
    if let Some(t) = title {
        builder = builder.set_title(t);
    }
    let path = builder.blocking_pick_folder();
    Ok(path.map(|p| p.to_string()))
}
