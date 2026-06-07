mod ai;
mod settings;
mod transcription;

use ai::{check_llm_connection, AiService};
use settings::{load_settings, save_settings, SettingsStore};
use transcription::{save_transcription_file, start_transcription};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let store = SettingsStore::new(app.handle()).map_err(|error| {
                Box::<dyn std::error::Error>::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    error,
                ))
            })?;
            let settings = store.load().map_err(|error| {
                Box::<dyn std::error::Error>::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    error,
                ))
            })?;
            let ai_service = AiService::new(settings.translation_thread_count).map_err(|error| {
                Box::<dyn std::error::Error>::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    error,
                ))
            })?;
            app.manage(store);
            app.manage(ai_service);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_settings,
            save_settings,
            check_llm_connection,
            start_transcription,
            save_transcription_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
