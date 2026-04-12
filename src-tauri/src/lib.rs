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

// Tray icon images — raw 32x32 RGBA baked into the binary.
// Generated from PNGs via: `convert foo.png -depth 8 RGBA:foo.rgba`
const TRAY_ICON_IDLE: &[u8] = include_bytes!("../icons/tray-idle.rgba");
const TRAY_ICON_RECORDING: &[u8] = include_bytes!("../icons/tray-recording.rgba");
const TRAY_ICON_SIZE: u32 = 32;

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
            // --- Prefs ---
            let data_dir = app.path().app_data_dir().unwrap_or_else(|_| {
                dirs::home_dir().unwrap_or_default().join(".noru")
            });
            if let Err(e) = prefs::init(data_dir) {
                eprintln!("warning: prefs init failed: {e:#}");
            }

            // --- Orchestrator ---
            let orchestrator = orchestrator::Orchestrator::new(app.handle().clone());
            app.manage(orchestrator);

            // --- Tray ---
            build_tray(app)?;

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

fn build_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder};
    use tauri::tray::TrayIconBuilder;

    let open = MenuItemBuilder::with_id("open", "Open noru").build(app)?;
    let start = MenuItemBuilder::with_id("start", "Start Recording").build(app)?;
    let stop = MenuItemBuilder::with_id("stop", "Stop Recording").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&open)
        .separator()
        .item(&start)
        .item(&stop)
        .separator()
        .item(&quit)
        .build()?;

    let icon = tauri::image::Image::new(TRAY_ICON_IDLE, TRAY_ICON_SIZE, TRAY_ICON_SIZE);

    let _tray = TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("noru")
        .menu(&menu)
        .on_menu_event(move |app, event| {
            let id = event.id();
            match id.as_ref() {
                "open" => {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
                "start" => {
                    let orch = app.state::<orchestrator::Orchestrator>();
                    if let Err(e) = orch.start_recording(true) {
                        eprintln!("tray: start recording failed: {e:#}");
                    }
                    update_tray(app);
                }
                "stop" => {
                    let orch = app.state::<orchestrator::Orchestrator>();
                    if let Err(e) = orch.stop_recording() {
                        eprintln!("tray: stop recording failed: {e:#}");
                    }
                    update_tray(app);
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click { .. } = event {
                let app = tray.app_handle();
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

/// Update tray icon + tooltip to reflect the current orchestrator state.
/// Called after tray menu actions; also callable from orchestrator events.
pub fn update_tray(app: &tauri::AppHandle) {
    let orch = app.state::<orchestrator::Orchestrator>();
    let state = orch.recording_state();

    let (tooltip, icon_bytes) = match &state {
        types::RecordingState::Idle => ("noru", TRAY_ICON_IDLE),
        types::RecordingState::Recording { .. } => ("noru (recording...)", TRAY_ICON_RECORDING),
        types::RecordingState::Transcribing { .. } => ("noru (transcribing...)", TRAY_ICON_IDLE),
    };

    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(tooltip));
        let img = tauri::image::Image::new(icon_bytes, TRAY_ICON_SIZE, TRAY_ICON_SIZE);
        let _ = tray.set_icon(Some(img));
    }
}
