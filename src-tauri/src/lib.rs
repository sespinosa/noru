pub mod ai;
pub mod audio;
pub mod auth;
pub mod commands;
pub mod detect;
pub mod models;
pub mod orchestrator;
pub mod prefs;
pub mod storage;
pub mod transcribe;
pub mod types;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(e) = storage::init() {
        eprintln!("failed to initialize storage: {e:#}");
        std::process::exit(1);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Prefs need app_data_dir which is only available after Tauri setup.
            let data_dir = app.path().app_data_dir().unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_default()
                    .join(".noru")
            });
            if let Err(e) = prefs::init(data_dir) {
                eprintln!("warning: prefs init failed: {e:#}");
                // Non-fatal — the app runs fine without prefs, settings
                // just won't persist across restarts.
            }

            let orchestrator = orchestrator::Orchestrator::new(app.handle().clone());
            app.manage(orchestrator);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_meetings,
            commands::get_meeting,
            commands::delete_meeting,
            commands::detect_poll,
            commands::recording_state,
            commands::start_recording,
            commands::stop_recording,
            commands::auth_status,
            commands::auth_start_login,
            commands::auth_sign_out,
            commands::ai_summarize,
            commands::ai_extract_action_items,
            commands::ai_extract_key_decisions,
            commands::get_autostart,
            commands::set_autostart,
            commands::list_audio_input_devices,
            commands::known_platforms,
            commands::download_model,
            commands::get_preference,
            commands::set_preference,
            commands::list_preferences,
            commands::choose_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running noru");
}
