pub mod ai;
pub mod audio;
pub mod auth;
pub mod commands;
pub mod detect;
pub mod models;
pub mod storage;
pub mod transcribe;
pub mod types;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::list_transcripts,
            commands::get_transcript,
            commands::delete_transcript,
            commands::recording_state,
            commands::start_recording,
            commands::stop_recording,
            commands::detect_meeting,
            commands::auth_status,
            commands::auth_sign_in,
            commands::auth_sign_out,
            commands::ai_summarize,
            commands::ai_action_items,
            commands::ai_key_decisions,
            commands::get_settings,
            commands::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running noru");
}
