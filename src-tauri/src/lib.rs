mod ai;
mod app_log;
mod settings;
mod subtitle_ai;
mod subtitle_translation;
mod transcription;

use ai::{check_llm_connection, AiService};
use app_log::{open_log_directory, AppLogger};
use settings::{load_settings, save_settings, SettingsStore};
use subtitle_translation::{load_subtitle_preview, start_subtitle_translation};
use tauri::Manager;
use transcription::{save_transcription_file, start_transcription};

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
            let ai_service =
                AiService::new(settings.translation_thread_count).map_err(|error| {
                    Box::<dyn std::error::Error>::from(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        error,
                    ))
                })?;
            let app_logger = AppLogger::new(app.handle()).map_err(|error| {
                Box::<dyn std::error::Error>::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    error,
                ))
            })?;
            app.manage(store);
            app.manage(ai_service);
            app.manage(app_logger);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_settings,
            save_settings,
            check_llm_connection,
            open_log_directory,
            start_transcription,
            save_transcription_file,
            load_subtitle_preview,
            start_subtitle_translation,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
