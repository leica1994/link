use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::AppHandle;

use crate::ai::AiService;
use crate::app_paths;

const DATABASE_FILE_NAME: &str = "settings.db";
const LLM_SERVICES: [&str; 3] = ["openai", "openai-responses", "anthropic"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub reasoning_effort: String,
    pub is_streaming: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: String,
    pub transcription_model: String,
    pub source_language: String,
    pub transcription_format: String,
    pub translation_format: String,
    #[serde(default = "default_subtitle_style_id")]
    pub selected_subtitle_style_id: String,
    pub is_smart_segmentation_enabled: bool,
    pub selected_llm_service: String,
    pub llm_configs: HashMap<String, LlmConfig>,
    pub translation_service: String,
    pub needs_reflection_translation: bool,
    pub translation_batch_size: u32,
    pub translation_thread_count: u32,
    pub video_content_type: String,
    pub output_mode: String,
    pub is_subtitle_correction_enabled: bool,
    pub is_subtitle_translation_enabled: bool,
    pub is_post_translation_optimization_enabled: bool,
    pub target_language: String,
    pub dubbing_tts_interval_ms: u32,
    pub dubbing_reference_audio_source: String,
    pub dubbing_custom_reference_audio_path: String,
    pub dubbing_is_background_music_enabled: bool,
    pub dubbing_background_music_volume: f64,
    pub home_workbench_translation_enabled: bool,
    pub home_workbench_dubbing_enabled: bool,
    pub home_workbench_export_dir: String,
    pub ytdlp_proxy: String,
    pub ytdlp_cookies_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YtdlpCookiesImportResult {
    pub cookies_path: String,
}

pub struct SettingsStore {
    connection: Mutex<Connection>,
}

impl SettingsStore {
    pub fn new(_app: &AppHandle) -> Result<Self, String> {
        let database_path = app_paths::settings_database_path(DATABASE_FILE_NAME)?;
        let connection = Connection::open(database_path)
            .map_err(|error| format!("无法打开设置数据库: {error}"))?;

        initialize_database(&connection)?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub(crate) fn load(&self) -> Result<AppSettings, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|error| format!("设置数据库锁定失败: {error}"))?;
        let setting_values = read_settings_map(&connection)?;
        let llm_configs = read_llm_configs(&connection)?;

        Ok(AppSettings {
            theme: read_string_setting(&setting_values, "theme", "light"),
            transcription_model: read_string_setting(
                &setting_values,
                "transcription_model",
                "bilibili",
            ),
            source_language: read_string_setting(&setting_values, "source_language", "auto"),
            transcription_format: read_string_setting(
                &setting_values,
                "transcription_format",
                "srt",
            ),
            translation_format: read_string_setting(&setting_values, "translation_format", "ass"),
            selected_subtitle_style_id: read_string_setting(
                &setting_values,
                "selected_subtitle_style_id",
                "default",
            ),
            is_smart_segmentation_enabled: read_bool_setting(
                &setting_values,
                "is_smart_segmentation_enabled",
                true,
            ),
            selected_llm_service: read_string_setting(
                &setting_values,
                "selected_llm_service",
                "openai",
            ),
            llm_configs,
            translation_service: read_string_setting(&setting_values, "translation_service", "llm"),
            needs_reflection_translation: read_bool_setting(
                &setting_values,
                "needs_reflection_translation",
                true,
            ),
            translation_batch_size: read_u32_setting(&setting_values, "translation_batch_size", 30),
            translation_thread_count: read_u32_setting(
                &setting_values,
                "translation_thread_count",
                10,
            ),
            video_content_type: read_string_setting(
                &setting_values,
                "video_content_type",
                "general",
            ),
            output_mode: read_string_setting(&setting_values, "output_mode", "bilingual"),
            is_subtitle_correction_enabled: read_bool_setting(
                &setting_values,
                "is_subtitle_correction_enabled",
                true,
            ),
            is_subtitle_translation_enabled: read_bool_setting(
                &setting_values,
                "is_subtitle_translation_enabled",
                true,
            ),
            is_post_translation_optimization_enabled: read_bool_setting(
                &setting_values,
                "is_post_translation_optimization_enabled",
                true,
            ),
            target_language: read_string_setting(&setting_values, "target_language", "zh-Hans"),
            dubbing_tts_interval_ms: read_u32_setting(
                &setting_values,
                "dubbing_tts_interval_ms",
                150,
            ),
            dubbing_reference_audio_source: read_string_setting(
                &setting_values,
                "dubbing_reference_audio_source",
                "existing-dubbing",
            ),
            dubbing_custom_reference_audio_path: read_string_setting(
                &setting_values,
                "dubbing_custom_reference_audio_path",
                "",
            ),
            dubbing_is_background_music_enabled: read_bool_setting(
                &setting_values,
                "dubbing_is_background_music_enabled",
                true,
            ),
            dubbing_background_music_volume: read_f64_setting(
                &setting_values,
                "dubbing_background_music_volume",
                0.5,
            ),
            home_workbench_translation_enabled: read_bool_setting(
                &setting_values,
                "home_workbench_translation_enabled",
                true,
            ),
            home_workbench_dubbing_enabled: read_bool_setting(
                &setting_values,
                "home_workbench_dubbing_enabled",
                false,
            ),
            home_workbench_export_dir: read_string_setting(
                &setting_values,
                "home_workbench_export_dir",
                "",
            ),
            ytdlp_proxy: read_string_setting(&setting_values, "ytdlp_proxy", ""),
            ytdlp_cookies_path: read_string_setting(&setting_values, "ytdlp_cookies_path", ""),
        })
    }

    pub(crate) fn with_connection<T>(
        &self,
        operation: impl FnOnce(&Connection) -> Result<T, String>,
    ) -> Result<T, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|error| format!("设置数据库锁定失败: {error}"))?;

        operation(&connection)
    }

    pub(crate) fn set_selected_subtitle_style_id(&self, style_id: &str) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .map_err(|error| format!("设置数据库锁定失败: {error}"))?;

        connection
            .execute(
                "
                INSERT INTO app_settings (key, value)
                VALUES ('selected_subtitle_style_id', ?1)
                ON CONFLICT(key) DO UPDATE SET value = excluded.value
                ",
                params![style_id],
            )
            .map(|_| ())
            .map_err(|error| format!("无法保存当前字幕样式: {error}"))
    }

    pub(crate) fn set_ytdlp_cookies_path(&self, cookies_path: &str) -> Result<(), String> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|error| format!("设置数据库锁定失败: {error}"))?;
        let transaction = connection
            .transaction()
            .map_err(|error| format!("无法开始保存 yt-dlp Cookies: {error}"))?;

        upsert_setting(&transaction, "ytdlp_cookies_path", cookies_path)?;

        transaction
            .commit()
            .map_err(|error| format!("无法提交 yt-dlp Cookies 设置: {error}"))
    }

    pub(crate) fn save(&self, settings: &AppSettings) -> Result<(), String> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|error| format!("设置数据库锁定失败: {error}"))?;
        let transaction = connection
            .transaction()
            .map_err(|error| format!("无法开始保存设置: {error}"))?;

        upsert_setting(&transaction, "theme", &settings.theme)?;
        upsert_setting(
            &transaction,
            "transcription_model",
            &settings.transcription_model,
        )?;
        upsert_setting(&transaction, "source_language", &settings.source_language)?;
        upsert_setting(
            &transaction,
            "transcription_format",
            &settings.transcription_format,
        )?;
        upsert_setting(
            &transaction,
            "translation_format",
            &settings.translation_format,
        )?;
        upsert_setting(
            &transaction,
            "is_smart_segmentation_enabled",
            bool_to_text(settings.is_smart_segmentation_enabled),
        )?;
        upsert_setting(
            &transaction,
            "selected_llm_service",
            &settings.selected_llm_service,
        )?;
        upsert_setting(
            &transaction,
            "translation_service",
            &settings.translation_service,
        )?;
        upsert_setting(
            &transaction,
            "needs_reflection_translation",
            bool_to_text(settings.needs_reflection_translation),
        )?;
        upsert_setting(
            &transaction,
            "translation_batch_size",
            &settings.translation_batch_size.to_string(),
        )?;
        upsert_setting(
            &transaction,
            "translation_thread_count",
            &settings.translation_thread_count.to_string(),
        )?;
        upsert_setting(
            &transaction,
            "video_content_type",
            &settings.video_content_type,
        )?;
        upsert_setting(&transaction, "output_mode", &settings.output_mode)?;
        upsert_setting(
            &transaction,
            "is_subtitle_correction_enabled",
            bool_to_text(settings.is_subtitle_correction_enabled),
        )?;
        upsert_setting(
            &transaction,
            "is_subtitle_translation_enabled",
            bool_to_text(settings.is_subtitle_translation_enabled),
        )?;
        upsert_setting(
            &transaction,
            "is_post_translation_optimization_enabled",
            bool_to_text(settings.is_post_translation_optimization_enabled),
        )?;
        upsert_setting(&transaction, "target_language", &settings.target_language)?;
        upsert_setting(
            &transaction,
            "dubbing_tts_interval_ms",
            &settings.dubbing_tts_interval_ms.to_string(),
        )?;
        upsert_setting(
            &transaction,
            "dubbing_reference_audio_source",
            &settings.dubbing_reference_audio_source,
        )?;
        upsert_setting(
            &transaction,
            "dubbing_custom_reference_audio_path",
            &settings.dubbing_custom_reference_audio_path,
        )?;
        upsert_setting(
            &transaction,
            "dubbing_is_background_music_enabled",
            bool_to_text(settings.dubbing_is_background_music_enabled),
        )?;
        upsert_setting(
            &transaction,
            "dubbing_background_music_volume",
            &settings.dubbing_background_music_volume.to_string(),
        )?;
        upsert_setting(
            &transaction,
            "home_workbench_translation_enabled",
            bool_to_text(settings.home_workbench_translation_enabled),
        )?;
        upsert_setting(
            &transaction,
            "home_workbench_dubbing_enabled",
            bool_to_text(settings.home_workbench_dubbing_enabled),
        )?;
        upsert_setting(
            &transaction,
            "home_workbench_export_dir",
            &settings.home_workbench_export_dir,
        )?;
        upsert_setting(&transaction, "ytdlp_proxy", &settings.ytdlp_proxy)?;
        upsert_setting(
            &transaction,
            "ytdlp_cookies_path",
            &settings.ytdlp_cookies_path,
        )?;

        for (service, config) in normalize_llm_configs(&settings.llm_configs) {
            transaction
                .execute(
                    "
                    INSERT INTO llm_configs (
                        service,
                        base_url,
                        api_key,
                        model,
                        reasoning_effort,
                        is_streaming
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    ON CONFLICT(service) DO UPDATE SET
                        base_url = excluded.base_url,
                        api_key = excluded.api_key,
                        model = excluded.model,
                        reasoning_effort = excluded.reasoning_effort,
                        is_streaming = excluded.is_streaming
                    ",
                    params![
                        service,
                        config.base_url,
                        config.api_key,
                        config.model,
                        config.reasoning_effort,
                        if config.is_streaming { 1 } else { 0 },
                    ],
                )
                .map_err(|error| format!("无法保存 LLM 配置: {error}"))?;
        }

        transaction
            .commit()
            .map_err(|error| format!("无法提交设置: {error}"))?;

        Ok(())
    }
}

#[tauri::command]
pub fn load_settings(store: tauri::State<'_, SettingsStore>) -> Result<AppSettings, String> {
    store.load()
}

#[tauri::command]
pub fn save_settings(
    store: tauri::State<'_, SettingsStore>,
    ai_service: tauri::State<'_, AiService>,
    settings: AppSettings,
) -> Result<(), String> {
    store.save(&settings)?;
    ai_service.update_thread_count(settings.translation_thread_count)
}

#[tauri::command]
pub fn import_ytdlp_cookies(
    store: tauri::State<'_, SettingsStore>,
    source_path: String,
) -> Result<YtdlpCookiesImportResult, String> {
    let existing_cookies_path = store.load()?.ytdlp_cookies_path;
    if !existing_cookies_path.trim().is_empty() {
        return Err("请先移除已上传的 Cookies 文件，再上传新的 Cookies".to_string());
    }

    let source_path = PathBuf::from(source_path);
    let source_path = fs::canonicalize(&source_path)
        .map_err(|error| format!("无法读取 Cookies 文件: {error}"))?;
    let source_metadata =
        fs::metadata(&source_path).map_err(|error| format!("无法读取 Cookies 文件: {error}"))?;

    if !source_metadata.is_file() {
        return Err("Cookies 只能选择文件".to_string());
    }

    if source_metadata.len() == 0 {
        return Err("Cookies 文件为空".to_string());
    }

    let destination_path = app_paths::ytdlp_cookies_path()?;
    let should_copy = fs::canonicalize(&destination_path)
        .map(|existing_path| existing_path != source_path)
        .unwrap_or(true);

    if should_copy {
        fs::copy(&source_path, &destination_path)
            .map_err(|error| format!("无法保存 Cookies 文件: {error}"))?;
    }

    let cookies_path = path_to_string(&destination_path);
    store.set_ytdlp_cookies_path(&cookies_path)?;

    Ok(YtdlpCookiesImportResult { cookies_path })
}

#[tauri::command]
pub fn clear_ytdlp_cookies(store: tauri::State<'_, SettingsStore>) -> Result<(), String> {
    let cookies_path = app_paths::ytdlp_cookies_path()?;

    match fs::remove_file(&cookies_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("无法移除 Cookies 文件: {error}")),
    }

    store.set_ytdlp_cookies_path("")
}

fn initialize_database(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS llm_configs (
                service TEXT PRIMARY KEY NOT NULL,
                base_url TEXT NOT NULL DEFAULT '',
                api_key TEXT NOT NULL DEFAULT '',
                model TEXT NOT NULL DEFAULT '',
                reasoning_effort TEXT NOT NULL DEFAULT 'off',
                is_streaming INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS dubbing_models (
                id TEXT PRIMARY KEY NOT NULL,
                engine TEXT NOT NULL,
                model_key TEXT NOT NULL,
                display_name TEXT NOT NULL,
                locale TEXT NOT NULL DEFAULT '',
                gender TEXT NOT NULL DEFAULT '',
                enabled INTEGER NOT NULL DEFAULT 1,
                metadata TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS dubbing_tasks (
                id TEXT PRIMARY KEY NOT NULL,
                pair_key TEXT NOT NULL UNIQUE,
                video_path TEXT NOT NULL,
                subtitle_path TEXT NOT NULL,
                work_dir TEXT NOT NULL,
                current_stage TEXT NOT NULL DEFAULT 'material',
                status TEXT NOT NULL DEFAULT 'ready',
                message TEXT NOT NULL DEFAULT '',
                progress INTEGER NOT NULL DEFAULT 0,
                options TEXT NOT NULL DEFAULT '{}',
                warnings TEXT NOT NULL DEFAULT '[]',
                error_message TEXT NOT NULL DEFAULT '',
                revision INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS dubbing_task_stages (
                task_id TEXT NOT NULL,
                stage_key TEXT NOT NULL,
                progress INTEGER NOT NULL DEFAULT 0,
                message TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'pending',
                snapshot TEXT NOT NULL DEFAULT '{}',
                updated_at TEXT NOT NULL,
                PRIMARY KEY (task_id, stage_key),
                FOREIGN KEY(task_id) REFERENCES dubbing_tasks(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS dubbing_task_artifacts (
                id TEXT PRIMARY KEY NOT NULL,
                task_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                path TEXT NOT NULL,
                metadata TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(task_id, kind),
                FOREIGN KEY(task_id) REFERENCES dubbing_tasks(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS dubbing_alignment_segments (
                task_id TEXT NOT NULL,
                segment_index INTEGER NOT NULL,
                uid TEXT NOT NULL DEFAULT '',
                source_start_ms INTEGER NOT NULL,
                source_end_ms INTEGER NOT NULL,
                tts_duration_ms INTEGER NOT NULL,
                pause_duration_ms INTEGER NOT NULL,
                aligned_start_ms INTEGER NOT NULL,
                aligned_end_ms INTEGER NOT NULL,
                block_duration_ms INTEGER NOT NULL,
                video_mode TEXT NOT NULL,
                pts REAL NOT NULL,
                freeze_tail_ms INTEGER NOT NULL,
                warning TEXT,
                PRIMARY KEY (task_id, segment_index),
                FOREIGN KEY(task_id) REFERENCES dubbing_tasks(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS youtube_channels (
                id TEXT PRIMARY KEY NOT NULL,
                url TEXT NOT NULL UNIQUE,
                canonical_url TEXT NOT NULL DEFAULT '',
                external_id TEXT NOT NULL DEFAULT '',
                title TEXT NOT NULL DEFAULT '',
                handle TEXT NOT NULL DEFAULT '',
                description TEXT NOT NULL DEFAULT '',
                thumbnail_url TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'idle',
                last_checked_at TEXT,
                last_success_at TEXT,
                last_error TEXT NOT NULL DEFAULT '',
                latest_video_external_id TEXT NOT NULL DEFAULT '',
                video_count INTEGER NOT NULL DEFAULT 0,
                unread_count INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS youtube_videos (
                id TEXT PRIMARY KEY NOT NULL,
                channel_id TEXT NOT NULL,
                external_id TEXT NOT NULL,
                title TEXT NOT NULL DEFAULT '',
                url TEXT NOT NULL DEFAULT '',
                duration REAL,
                published_rank INTEGER NOT NULL DEFAULT 0,
                is_unread INTEGER NOT NULL DEFAULT 1,
                first_seen_at TEXT NOT NULL,
                last_seen_at TEXT NOT NULL,
                metadata TEXT NOT NULL DEFAULT '{}',
                UNIQUE(channel_id, external_id),
                FOREIGN KEY(channel_id) REFERENCES youtube_channels(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS youtube_refresh_runs (
                id TEXT PRIMARY KEY NOT NULL,
                channel_id TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'running',
                processed_count INTEGER NOT NULL DEFAULT 0,
                inserted_count INTEGER NOT NULL DEFAULT 0,
                updated_count INTEGER NOT NULL DEFAULT 0,
                message TEXT NOT NULL DEFAULT '',
                error_message TEXT NOT NULL DEFAULT '',
                started_at TEXT NOT NULL,
                finished_at TEXT,
                FOREIGN KEY(channel_id) REFERENCES youtube_channels(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS home_video_tasks (
                id TEXT PRIMARY KEY NOT NULL,
                url TEXT NOT NULL UNIQUE,
                source_channel_id TEXT NOT NULL DEFAULT '',
                source_video_id TEXT NOT NULL DEFAULT '',
                external_id TEXT NOT NULL DEFAULT '',
                title TEXT NOT NULL DEFAULT '',
                channel_title TEXT NOT NULL DEFAULT '',
                channel_url TEXT NOT NULL DEFAULT '',
                thumbnail_url TEXT NOT NULL DEFAULT '',
                duration REAL,
                webpage_url TEXT NOT NULL DEFAULT '',
                description TEXT NOT NULL DEFAULT '',
                view_count INTEGER,
                like_count INTEGER,
                comment_count INTEGER,
                upload_date TEXT NOT NULL DEFAULT '',
                detail_status TEXT NOT NULL DEFAULT 'pending',
                subtitle_options TEXT NOT NULL DEFAULT '[]',
                metadata TEXT NOT NULL DEFAULT '{}',
                error_message TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                detail_checked_at TEXT
            );

            CREATE TABLE IF NOT EXISTS home_video_task_subtitles (
                id TEXT PRIMARY KEY NOT NULL,
                task_id TEXT NOT NULL,
                language TEXT NOT NULL,
                language_name TEXT NOT NULL DEFAULT '',
                source_kind TEXT NOT NULL DEFAULT 'manual',
                format TEXT NOT NULL DEFAULT '',
                file_path TEXT NOT NULL,
                file_size INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(task_id, language, source_kind),
                FOREIGN KEY(task_id) REFERENCES home_video_tasks(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS home_video_task_videos (
                id TEXT PRIMARY KEY NOT NULL,
                task_id TEXT NOT NULL UNIQUE,
                format TEXT NOT NULL DEFAULT '',
                file_path TEXT NOT NULL,
                file_name TEXT NOT NULL DEFAULT '',
                file_size INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(task_id) REFERENCES home_video_tasks(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS home_workbench_tasks (
                task_id TEXT PRIMARY KEY NOT NULL,
                status TEXT NOT NULL DEFAULT 'idle',
                current_stage TEXT NOT NULL DEFAULT '',
                progress INTEGER NOT NULL DEFAULT 0,
                message TEXT NOT NULL DEFAULT '',
                stages TEXT NOT NULL DEFAULT '{}',
                options TEXT NOT NULL DEFAULT '{}',
                warnings TEXT NOT NULL DEFAULT '[]',
                error_message TEXT NOT NULL DEFAULT '',
                revision INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(task_id) REFERENCES home_video_tasks(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS home_workbench_artifacts (
                id TEXT PRIMARY KEY NOT NULL,
                task_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                path TEXT NOT NULL,
                file_size INTEGER NOT NULL DEFAULT 0,
                metadata TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(task_id, kind),
                FOREIGN KEY(task_id) REFERENCES home_video_tasks(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS content_copy_records (
                id TEXT PRIMARY KEY NOT NULL,
                source TEXT NOT NULL DEFAULT 'copywriting',
                platform TEXT NOT NULL DEFAULT 'bilibili',
                subtitle_path TEXT NOT NULL,
                subtitle_file_name TEXT NOT NULL DEFAULT '',
                subtitle_format TEXT NOT NULL DEFAULT '',
                segment_count INTEGER NOT NULL DEFAULT 0,
                duration_ms INTEGER NOT NULL DEFAULT 0,
                extra_context TEXT NOT NULL DEFAULT '',
                options TEXT NOT NULL DEFAULT '{}',
                result TEXT NOT NULL DEFAULT '{}',
                log_path TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS subtitle_styles (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL UNIQUE,
                is_default INTEGER NOT NULL DEFAULT 0,
                render_mode TEXT NOT NULL DEFAULT 'ass',
                subtitle_layout TEXT NOT NULL DEFAULT 'target-above',
                preview_text_mode TEXT NOT NULL DEFAULT 'medium',
                primary_font_name TEXT NOT NULL DEFAULT 'Arial',
                primary_font_size INTEGER NOT NULL DEFAULT 48,
                primary_color TEXT NOT NULL DEFAULT '#FFFFFF',
                primary_outline_color TEXT NOT NULL DEFAULT '#000000',
                primary_outline_width REAL NOT NULL DEFAULT 2.0,
                primary_spacing REAL NOT NULL DEFAULT 0.0,
                primary_margin_bottom INTEGER NOT NULL DEFAULT 48,
                secondary_font_name TEXT NOT NULL DEFAULT 'Arial',
                secondary_font_size INTEGER NOT NULL DEFAULT 36,
                secondary_color TEXT NOT NULL DEFAULT '#FFFFFF',
                secondary_outline_color TEXT NOT NULL DEFAULT '#000000',
                secondary_outline_width REAL NOT NULL DEFAULT 2.0,
                secondary_spacing REAL NOT NULL DEFAULT 0.0,
                vertical_spacing INTEGER NOT NULL DEFAULT 15,
                rounded_font_name TEXT NOT NULL DEFAULT 'Microsoft YaHei',
                rounded_font_size INTEGER NOT NULL DEFAULT 34,
                rounded_text_color TEXT NOT NULL DEFAULT '#FFFFFF',
                rounded_background_color TEXT NOT NULL DEFAULT '#191919CC',
                rounded_corner_radius INTEGER NOT NULL DEFAULT 14,
                rounded_padding_x INTEGER NOT NULL DEFAULT 24,
                rounded_padding_y INTEGER NOT NULL DEFAULT 14,
                rounded_margin_bottom INTEGER NOT NULL DEFAULT 60,
                rounded_line_spacing INTEGER NOT NULL DEFAULT 10,
                rounded_letter_spacing INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_youtube_channels_updated_at
                ON youtube_channels(updated_at);
            CREATE INDEX IF NOT EXISTS idx_youtube_videos_channel_seen
                ON youtube_videos(channel_id, is_unread, first_seen_at);
            CREATE INDEX IF NOT EXISTS idx_youtube_videos_channel_rank
                ON youtube_videos(channel_id, published_rank);
            CREATE INDEX IF NOT EXISTS idx_youtube_refresh_runs_channel_started
                ON youtube_refresh_runs(channel_id, started_at);
            CREATE INDEX IF NOT EXISTS idx_home_video_tasks_updated_at
                ON home_video_tasks(updated_at);
            CREATE INDEX IF NOT EXISTS idx_home_video_tasks_created_at
                ON home_video_tasks(created_at);
            CREATE INDEX IF NOT EXISTS idx_home_video_tasks_status
                ON home_video_tasks(detail_status, updated_at);
            CREATE INDEX IF NOT EXISTS idx_home_video_task_subtitles_task
                ON home_video_task_subtitles(task_id, updated_at);
            CREATE INDEX IF NOT EXISTS idx_home_video_task_videos_task
                ON home_video_task_videos(task_id, updated_at);
            CREATE INDEX IF NOT EXISTS idx_home_workbench_tasks_updated_at
                ON home_workbench_tasks(updated_at);
            CREATE INDEX IF NOT EXISTS idx_home_workbench_artifacts_task
                ON home_workbench_artifacts(task_id, updated_at);
            CREATE INDEX IF NOT EXISTS idx_content_copy_records_updated_at
                ON content_copy_records(updated_at);
            CREATE INDEX IF NOT EXISTS idx_subtitle_styles_name
                ON subtitle_styles(name);
            ",
        )
        .map_err(|error| format!("无法初始化设置数据库: {error}"))?;

    ensure_column(
        connection,
        "dubbing_models",
        "scheduler_weight",
        "ALTER TABLE dubbing_models ADD COLUMN scheduler_weight REAL NOT NULL DEFAULT 100.0",
    )?;
    ensure_column(
        connection,
        "dubbing_models",
        "success_count",
        "ALTER TABLE dubbing_models ADD COLUMN success_count INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        connection,
        "dubbing_models",
        "failure_count",
        "ALTER TABLE dubbing_models ADD COLUMN failure_count INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        connection,
        "dubbing_models",
        "consecutive_failures",
        "ALTER TABLE dubbing_models ADD COLUMN consecutive_failures INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        connection,
        "dubbing_models",
        "avg_latency_ms",
        "ALTER TABLE dubbing_models ADD COLUMN avg_latency_ms INTEGER",
    )?;
    ensure_column(
        connection,
        "dubbing_models",
        "cooldown_until",
        "ALTER TABLE dubbing_models ADD COLUMN cooldown_until TEXT",
    )?;
    ensure_column(
        connection,
        "dubbing_models",
        "last_error",
        "ALTER TABLE dubbing_models ADD COLUMN last_error TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        connection,
        "dubbing_models",
        "last_used_at",
        "ALTER TABLE dubbing_models ADD COLUMN last_used_at TEXT",
    )?;
    ensure_column(
        connection,
        "dubbing_models",
        "last_checked_at",
        "ALTER TABLE dubbing_models ADD COLUMN last_checked_at TEXT",
    )?;
    ensure_column(
        connection,
        "youtube_channels",
        "latest_video_external_id",
        "ALTER TABLE youtube_channels ADD COLUMN latest_video_external_id TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        connection,
        "youtube_videos",
        "published_rank",
        "ALTER TABLE youtube_videos ADD COLUMN published_rank INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        connection,
        "subtitle_styles",
        "render_mode",
        "ALTER TABLE subtitle_styles ADD COLUMN render_mode TEXT NOT NULL DEFAULT 'ass'",
    )?;
    ensure_column(
        connection,
        "subtitle_styles",
        "subtitle_layout",
        "ALTER TABLE subtitle_styles ADD COLUMN subtitle_layout TEXT NOT NULL DEFAULT 'target-above'",
    )?;
    ensure_column(
        connection,
        "subtitle_styles",
        "preview_text_mode",
        "ALTER TABLE subtitle_styles ADD COLUMN preview_text_mode TEXT NOT NULL DEFAULT 'medium'",
    )?;
    ensure_column(
        connection,
        "subtitle_styles",
        "primary_margin_bottom",
        "ALTER TABLE subtitle_styles ADD COLUMN primary_margin_bottom INTEGER NOT NULL DEFAULT 48",
    )?;
    ensure_column(
        connection,
        "subtitle_styles",
        "rounded_font_name",
        "ALTER TABLE subtitle_styles ADD COLUMN rounded_font_name TEXT NOT NULL DEFAULT 'Microsoft YaHei'",
    )?;
    ensure_column(
        connection,
        "subtitle_styles",
        "rounded_font_size",
        "ALTER TABLE subtitle_styles ADD COLUMN rounded_font_size INTEGER NOT NULL DEFAULT 34",
    )?;
    ensure_column(
        connection,
        "subtitle_styles",
        "rounded_text_color",
        "ALTER TABLE subtitle_styles ADD COLUMN rounded_text_color TEXT NOT NULL DEFAULT '#FFFFFF'",
    )?;
    ensure_column(
        connection,
        "subtitle_styles",
        "rounded_background_color",
        "ALTER TABLE subtitle_styles ADD COLUMN rounded_background_color TEXT NOT NULL DEFAULT '#191919CC'",
    )?;
    ensure_column(
        connection,
        "subtitle_styles",
        "rounded_corner_radius",
        "ALTER TABLE subtitle_styles ADD COLUMN rounded_corner_radius INTEGER NOT NULL DEFAULT 14",
    )?;
    ensure_column(
        connection,
        "subtitle_styles",
        "rounded_padding_x",
        "ALTER TABLE subtitle_styles ADD COLUMN rounded_padding_x INTEGER NOT NULL DEFAULT 24",
    )?;
    ensure_column(
        connection,
        "subtitle_styles",
        "rounded_padding_y",
        "ALTER TABLE subtitle_styles ADD COLUMN rounded_padding_y INTEGER NOT NULL DEFAULT 14",
    )?;
    ensure_column(
        connection,
        "subtitle_styles",
        "rounded_margin_bottom",
        "ALTER TABLE subtitle_styles ADD COLUMN rounded_margin_bottom INTEGER NOT NULL DEFAULT 60",
    )?;
    ensure_column(
        connection,
        "subtitle_styles",
        "rounded_line_spacing",
        "ALTER TABLE subtitle_styles ADD COLUMN rounded_line_spacing INTEGER NOT NULL DEFAULT 10",
    )?;
    ensure_column(
        connection,
        "subtitle_styles",
        "rounded_letter_spacing",
        "ALTER TABLE subtitle_styles ADD COLUMN rounded_letter_spacing INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        connection,
        "content_copy_records",
        "source",
        "ALTER TABLE content_copy_records ADD COLUMN source TEXT NOT NULL DEFAULT 'copywriting'",
    )?;
    connection
        .execute(
            "
            CREATE INDEX IF NOT EXISTS idx_content_copy_records_source_updated_at
                ON content_copy_records(source, updated_at)
            ",
            [],
        )
        .map_err(|error| format!("无法初始化文案历史来源索引: {error}"))?;
    mark_workbench_content_copy_records(connection)?;

    for service in LLM_SERVICES {
        connection
            .execute(
                "
                INSERT OR IGNORE INTO llm_configs (
                    service,
                    base_url,
                    api_key,
                    model,
                    reasoning_effort,
                    is_streaming
                )
                VALUES (?1, '', '', '', 'off', 1)
                ",
                params![service],
            )
            .map_err(|error| format!("无法初始化 LLM 配置: {error}"))?;
    }

    // 初始化默认字幕样式
    connection
        .execute(
            "
            INSERT OR IGNORE INTO subtitle_styles (
                id,
                name,
                is_default,
                render_mode,
                subtitle_layout,
                preview_text_mode,
                primary_font_name,
                primary_font_size,
                primary_color,
                primary_outline_color,
                primary_outline_width,
                primary_spacing,
                primary_margin_bottom,
                secondary_font_name,
                secondary_font_size,
                secondary_color,
                secondary_outline_color,
                secondary_outline_width,
                secondary_spacing,
                vertical_spacing,
                rounded_font_name,
                rounded_font_size,
                rounded_text_color,
                rounded_background_color,
                rounded_corner_radius,
                rounded_padding_x,
                rounded_padding_y,
                rounded_margin_bottom,
                rounded_line_spacing,
                rounded_letter_spacing,
                created_at,
                updated_at
            )
            VALUES (
                'default',
                '默认样式',
                1,
                'ass',
                'target-above',
                'medium',
                'Microsoft YaHei',
                48,
                '#FFFFFF',
                '#000000',
                2.0,
                0.0,
                48,
                'Microsoft YaHei',
                36,
                '#FFFFFF',
                '#000000',
                2.0,
                0.0,
                15,
                'Microsoft YaHei',
                34,
                '#FFFFFF',
                '#191919CC',
                14,
                24,
                14,
                60,
                10,
                0,
                datetime('now'),
                datetime('now')
            )
            ",
            [],
        )
        .map_err(|error| format!("无法初始化默认字幕样式: {error}"))?;
    connection
        .execute(
            "
            INSERT OR IGNORE INTO subtitle_styles (
                id,
                name,
                is_default,
                render_mode,
                subtitle_layout,
                preview_text_mode,
                primary_font_name,
                primary_font_size,
                primary_color,
                primary_outline_color,
                primary_outline_width,
                primary_spacing,
                primary_margin_bottom,
                secondary_font_name,
                secondary_font_size,
                secondary_color,
                secondary_outline_color,
                secondary_outline_width,
                secondary_spacing,
                vertical_spacing,
                rounded_font_name,
                rounded_font_size,
                rounded_text_color,
                rounded_background_color,
                rounded_corner_radius,
                rounded_padding_x,
                rounded_padding_y,
                rounded_margin_bottom,
                rounded_line_spacing,
                rounded_letter_spacing,
                created_at,
                updated_at
            )
            VALUES (
                'rounded-default',
                '圆角背景',
                0,
                'rounded',
                'target-above',
                'medium',
                'Microsoft YaHei',
                44,
                '#FFFFFF',
                '#000000',
                2.0,
                0.0,
                48,
                'Microsoft YaHei',
                32,
                '#FFFFFF',
                '#000000',
                2.0,
                0.0,
                15,
                'Microsoft YaHei',
                34,
                '#FFFFFF',
                '#191919CC',
                14,
                24,
                14,
                60,
                10,
                0,
                datetime('now'),
                datetime('now')
            )
            ",
            [],
        )
        .map_err(|error| format!("无法初始化圆角字幕样式: {error}"))?;
    connection
        .execute(
            "
            INSERT OR IGNORE INTO subtitle_styles (
                id,
                name,
                is_default,
                render_mode,
                subtitle_layout,
                preview_text_mode,
                primary_font_name,
                primary_font_size,
                primary_color,
                primary_outline_color,
                primary_outline_width,
                primary_spacing,
                primary_margin_bottom,
                secondary_font_name,
                secondary_font_size,
                secondary_color,
                secondary_outline_color,
                secondary_outline_width,
                secondary_spacing,
                vertical_spacing,
                rounded_font_name,
                rounded_font_size,
                rounded_text_color,
                rounded_background_color,
                rounded_corner_radius,
                rounded_padding_x,
                rounded_padding_y,
                rounded_margin_bottom,
                rounded_line_spacing,
                rounded_letter_spacing,
                created_at,
                updated_at
            )
            VALUES (
                'anime',
                '活力描边',
                0,
                'ass',
                'target-above',
                'short',
                'Microsoft YaHei',
                46,
                '#FFF5F3',
                '#F58709',
                2.6,
                2.6,
                40,
                'Microsoft YaHei',
                26,
                '#FFFFFF',
                '#F58709',
                2.0,
                0.0,
                14,
                'Microsoft YaHei',
                34,
                '#FFFFFF',
                '#191919CC',
                14,
                24,
                14,
                60,
                10,
                0,
                datetime('now'),
                datetime('now')
            )
            ",
            [],
        )
        .map_err(|error| format!("无法初始化活力字幕样式: {error}"))?;

    Ok(())
}

fn ensure_column(
    connection: &Connection,
    table: &str,
    column: &str,
    alter_sql: &str,
) -> Result<(), String> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|error| format!("无法检查数据库字段: {error}"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| format!("无法读取数据库字段: {error}"))?;

    for value in columns {
        if value.map_err(|error| format!("无法解析数据库字段: {error}"))? == column {
            return Ok(());
        }
    }

    connection
        .execute(alter_sql, [])
        .map(|_| ())
        .map_err(|error| format!("无法迁移数据库字段 {table}.{column}: {error}"))
}

fn mark_workbench_content_copy_records(connection: &Connection) -> Result<(), String> {
    connection
        .execute(
            "
            UPDATE content_copy_records
            SET source = 'workbench'
            WHERE source = 'copywriting'
              AND EXISTS (
                  SELECT 1
                  FROM home_workbench_tasks
                  WHERE home_workbench_tasks.stages LIKE '%' || content_copy_records.id || '%'
              )
            ",
            [],
        )
        .map(|_| ())
        .map_err(|error| format!("无法迁移工作台文案历史: {error}"))
}

fn read_settings_map(connection: &Connection) -> Result<HashMap<String, String>, String> {
    let mut statement = connection
        .prepare("SELECT key, value FROM app_settings")
        .map_err(|error| format!("无法读取设置: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("无法读取设置: {error}"))?;

    let mut settings = HashMap::new();

    for row in rows {
        let (key, value) = row.map_err(|error| format!("无法解析设置: {error}"))?;
        settings.insert(key, value);
    }

    Ok(settings)
}

fn read_llm_configs(connection: &Connection) -> Result<HashMap<String, LlmConfig>, String> {
    let mut configs = default_llm_configs();
    let mut statement = connection
        .prepare(
            "
            SELECT service, base_url, api_key, model, reasoning_effort, is_streaming
            FROM llm_configs
            ",
        )
        .map_err(|error| format!("无法读取 LLM 配置: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                LlmConfig {
                    base_url: row.get(1)?,
                    api_key: row.get(2)?,
                    model: row.get(3)?,
                    reasoning_effort: row.get(4)?,
                    is_streaming: row.get::<_, i64>(5)? != 0,
                },
            ))
        })
        .map_err(|error| format!("无法读取 LLM 配置: {error}"))?;

    for row in rows {
        let (service, config) = row.map_err(|error| format!("无法解析 LLM 配置: {error}"))?;
        configs.insert(service, config);
    }

    Ok(configs)
}

fn default_llm_configs() -> HashMap<String, LlmConfig> {
    LLM_SERVICES
        .iter()
        .map(|service| {
            (
                (*service).to_string(),
                LlmConfig {
                    base_url: String::new(),
                    api_key: String::new(),
                    model: String::new(),
                    reasoning_effort: "off".to_string(),
                    is_streaming: true,
                },
            )
        })
        .collect()
}

fn normalize_llm_configs(configs: &HashMap<String, LlmConfig>) -> HashMap<String, LlmConfig> {
    let mut normalized = default_llm_configs();

    for (service, config) in configs {
        normalized.insert(service.clone(), config.clone());
    }

    normalized
}

fn read_string_setting(settings: &HashMap<String, String>, key: &str, fallback: &str) -> String {
    settings
        .get(key)
        .cloned()
        .unwrap_or_else(|| fallback.to_string())
}

fn read_bool_setting(settings: &HashMap<String, String>, key: &str, fallback: bool) -> bool {
    settings
        .get(key)
        .map(|value| value == "true")
        .unwrap_or(fallback)
}

fn read_u32_setting(settings: &HashMap<String, String>, key: &str, fallback: u32) -> u32 {
    settings
        .get(key)
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(fallback)
}

fn read_f64_setting(settings: &HashMap<String, String>, key: &str, fallback: f64) -> f64 {
    settings
        .get(key)
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(fallback)
}

fn bool_to_text(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

fn default_subtitle_style_id() -> String {
    "default".to_string()
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn upsert_setting(
    transaction: &rusqlite::Transaction<'_>,
    key: &str,
    value: &str,
) -> Result<(), String> {
    transaction
        .execute(
            "
            INSERT INTO app_settings (key, value)
            VALUES (?1, ?2)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            ",
            params![key, value],
        )
        .map_err(|error| format!("无法保存设置 {key}: {error}"))?;

    Ok(())
}
