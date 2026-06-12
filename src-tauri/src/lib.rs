mod ai;
mod app_log;
mod app_paths;
mod dubbing;
mod dubbing_alignment;
mod dubbing_compose;
mod home_tasks;
mod htdemucs;
mod settings;
mod subtitle_ai;
mod subtitle_translation;
mod transcription;
mod youtube_monitor;

use ai::{check_llm_connection, AiService};
use app_log::{open_log_directory, AppLogger};
use dubbing::{
    add_dubbing_model, delete_dubbing_model, list_dubbing_models, list_dubbing_voices,
    load_dubbing_reference_audio, prepare_dubbing_material, preview_dubbing_voice,
    set_dubbing_model_enabled, start_dubbing_task, DubbingTtsScheduler,
};
use home_tasks::{
    add_home_video_task, delete_home_video_task, download_home_video_task_subtitle,
    get_home_video_task, list_home_video_tasks, refresh_home_video_task_detail,
};
use settings::{load_settings, save_settings, SettingsStore};
use subtitle_translation::{
    load_subtitle_preview, save_subtitle_translation_file, start_subtitle_translation,
};
use tauri::{Manager, WebviewWindowBuilder};
use transcription::{save_subtitle_segments_file, save_transcription_file, start_transcription};
use youtube_monitor::{
    add_youtube_channel, delete_youtube_channel, get_ytdlp_status, list_youtube_channels,
    list_youtube_videos, mark_youtube_channel_seen, refresh_youtube_channel, YoutubeMonitorService,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
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
            let dubbing_tts_scheduler = DubbingTtsScheduler::new();
            let youtube_monitor_service = YoutubeMonitorService::new();
            app.manage(store);
            app.manage(ai_service);
            app.manage(app_logger);
            app.manage(dubbing_tts_scheduler);
            app.manage(youtube_monitor_service);

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
            save_subtitle_segments_file,
            load_subtitle_preview,
            start_subtitle_translation,
            save_subtitle_translation_file,
            list_dubbing_models,
            list_dubbing_voices,
            add_dubbing_model,
            set_dubbing_model_enabled,
            delete_dubbing_model,
            preview_dubbing_voice,
            load_dubbing_reference_audio,
            prepare_dubbing_material,
            start_dubbing_task,
            list_home_video_tasks,
            add_home_video_task,
            get_home_video_task,
            delete_home_video_task,
            refresh_home_video_task_detail,
            download_home_video_task_subtitle,
            get_ytdlp_status,
            list_youtube_channels,
            add_youtube_channel,
            delete_youtube_channel,
            list_youtube_videos,
            mark_youtube_channel_seen,
            refresh_youtube_channel,
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
