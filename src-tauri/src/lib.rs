mod ai;
mod app_paths;
mod app_log;
mod dubbing;
mod settings;
mod subtitle_ai;
mod subtitle_translation;
mod transcription;

use ai::{check_llm_connection, AiService};
use app_log::{open_log_directory, AppLogger};
use dubbing::{
    add_dubbing_model, delete_dubbing_model, list_dubbing_models, list_dubbing_voices,
    prepare_dubbing_material, preview_dubbing_voice, set_dubbing_model_enabled, start_dubbing_task,
};
use settings::{load_settings, save_settings, SettingsStore};
use subtitle_translation::{load_subtitle_preview, start_subtitle_translation};
use tauri::{Manager, WebviewWindowBuilder};
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

            let window_config = app
                .config()
                .app
                .windows
                .first()
                .ok_or_else(|| setup_error("缺少主窗口配置"))?
                .clone();
            let webview_data_dir = app_paths::webview_data_dir().map_err(setup_error)?;
            WebviewWindowBuilder::from_config(app.handle(), &window_config)?
                .data_directory(webview_data_dir)
                .build()?;

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
            list_dubbing_models,
            list_dubbing_voices,
            add_dubbing_model,
            set_dubbing_model_enabled,
            delete_dubbing_model,
            preview_dubbing_voice,
            prepare_dubbing_material,
            start_dubbing_task,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_error(message: impl Into<String>) -> Box<dyn std::error::Error> {
    Box::new(std::io::Error::new(
        std::io::ErrorKind::Other,
        message.into(),
    ))
}
