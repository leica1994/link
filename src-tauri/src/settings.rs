use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::ai::AiService;
use crate::app_log::AppLogger;
use crate::app_paths;

const DATABASE_FILE_NAME: &str = "settings.db";
const LLM_SERVICES: [&str; 3] = ["openai", "openai-responses", "anthropic"];
const SETTINGS_BACKUP_SCHEMA_VERSION: u32 = 2;
const MIN_SETTINGS_BACKUP_SCHEMA_VERSION: u32 = 1;
const YOUTUBE_CHANNEL_STATUS_IDLE: &str = "idle";
const DUBBING_MODELS_EVENT: &str = "dubbing-models-updated";

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
    pub is_ai_subtitle_review_enabled: bool,
    pub ai_subtitle_review_mode: String,
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
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsBackup {
    schema_version: u32,
    exported_at: String,
    settings: AppSettings,
    youtube_monitor: SettingsBackupYoutubeMonitor,
    #[serde(default)]
    subtitle_styles: SettingsBackupSubtitleStyles,
    #[serde(default)]
    dubbing: SettingsBackupDubbing,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsBackupYoutubeMonitor {
    channels: Vec<BackupYoutubeChannel>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsBackupSubtitleStyles {
    styles: Vec<BackupSubtitleStyle>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsBackupDubbing {
    models: Vec<BackupDubbingModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupYoutubeChannel {
    id: String,
    url: String,
    canonical_url: String,
    external_id: String,
    title: String,
    handle: String,
    description: String,
    thumbnail_url: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupSubtitleStyle {
    id: String,
    name: String,
    is_default: bool,
    render_mode: String,
    subtitle_layout: String,
    preview_text_mode: String,
    primary_font_name: String,
    primary_font_size: u32,
    primary_color: String,
    primary_outline_color: String,
    primary_outline_width: f64,
    primary_spacing: f64,
    primary_margin_bottom: u32,
    secondary_font_name: String,
    secondary_font_size: u32,
    secondary_color: String,
    secondary_outline_color: String,
    secondary_outline_width: f64,
    secondary_spacing: f64,
    vertical_spacing: u32,
    rounded_font_name: String,
    rounded_font_size: u32,
    rounded_text_color: String,
    rounded_background_color: String,
    rounded_corner_radius: u32,
    rounded_padding_x: u32,
    rounded_padding_y: u32,
    rounded_margin_bottom: u32,
    rounded_line_spacing: u32,
    rounded_letter_spacing: u32,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupDubbingModel {
    id: String,
    engine: String,
    model_key: String,
    display_name: String,
    locale: String,
    gender: String,
    enabled: bool,
    metadata: Value,
    scheduler_weight: f64,
    success_count: u64,
    failure_count: u64,
    consecutive_failures: u64,
    avg_latency_ms: Option<u64>,
    cooldown_until: Option<String>,
    last_error: String,
    last_used_at: Option<String>,
    last_checked_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsBackupSummary {
    pub setting_count: usize,
    pub channel_count: usize,
    pub added_channel_count: usize,
    pub updated_channel_count: usize,
    pub subtitle_style_count: usize,
    pub added_subtitle_style_count: usize,
    pub updated_subtitle_style_count: usize,
    pub dubbing_model_count: usize,
    pub added_dubbing_model_count: usize,
    pub updated_dubbing_model_count: usize,
}

#[derive(Debug, Default)]
struct SettingsBackupImportSummary {
    added_channel_count: usize,
    updated_channel_count: usize,
    added_subtitle_style_count: usize,
    updated_subtitle_style_count: usize,
    added_dubbing_model_count: usize,
    updated_dubbing_model_count: usize,
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
            is_ai_subtitle_review_enabled: read_bool_setting(
                &setting_values,
                "is_ai_subtitle_review_enabled",
                true,
            ),
            ai_subtitle_review_mode: read_string_setting(
                &setting_values,
                "ai_subtitle_review_mode",
                "expert",
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

    pub(crate) fn save(&self, settings: &AppSettings) -> Result<(), String> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|error| format!("设置数据库锁定失败: {error}"))?;
        let transaction = connection
            .transaction()
            .map_err(|error| format!("无法开始保存设置: {error}"))?;

        save_settings_in_transaction(&transaction, settings)?;

        transaction
            .commit()
            .map_err(|error| format!("无法提交设置: {error}"))?;

        Ok(())
    }

    fn export_backup(&self) -> Result<SettingsBackup, String> {
        let settings = self.load()?;
        let (channels, subtitle_styles, dubbing_models) = self.with_connection(|connection| {
            Ok((
                read_backup_youtube_channels(connection)?,
                read_backup_subtitle_styles(connection)?,
                read_backup_dubbing_models(connection)?,
            ))
        })?;

        Ok(SettingsBackup {
            schema_version: SETTINGS_BACKUP_SCHEMA_VERSION,
            exported_at: Utc::now().to_rfc3339(),
            settings,
            youtube_monitor: SettingsBackupYoutubeMonitor { channels },
            subtitle_styles: SettingsBackupSubtitleStyles {
                styles: subtitle_styles,
            },
            dubbing: SettingsBackupDubbing {
                models: dubbing_models,
            },
        })
    }

    fn import_backup(
        &self,
        backup: &SettingsBackup,
    ) -> Result<SettingsBackupImportSummary, String> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|error| format!("设置数据库锁定失败: {error}"))?;
        let transaction = connection
            .transaction()
            .map_err(|error| format!("无法开始导入设置备份: {error}"))?;

        save_settings_in_transaction(&transaction, &backup.settings)?;
        let mut summary =
            import_backup_youtube_channels(&transaction, &backup.youtube_monitor.channels)?;
        let subtitle_summary =
            import_backup_subtitle_styles(&transaction, &backup.subtitle_styles.styles)?;
        summary.added_subtitle_style_count = subtitle_summary.added_subtitle_style_count;
        summary.updated_subtitle_style_count = subtitle_summary.updated_subtitle_style_count;
        ensure_selected_backup_subtitle_style(
            &transaction,
            &backup.settings.selected_subtitle_style_id,
        )?;
        let dubbing_summary = import_backup_dubbing_models(&transaction, &backup.dubbing.models)?;
        summary.added_dubbing_model_count = dubbing_summary.added_dubbing_model_count;
        summary.updated_dubbing_model_count = dubbing_summary.updated_dubbing_model_count;

        transaction
            .commit()
            .map_err(|error| format!("无法提交设置备份导入: {error}"))?;

        Ok(summary)
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
    app_logger: tauri::State<'_, AppLogger>,
    settings: AppSettings,
) -> Result<(), String> {
    app_logger.info(
        "settings",
        "save_start",
        "开始保存应用设置",
        serde_json::json!({
            "selectedLlmService": &settings.selected_llm_service,
            "translationThreadCount": settings.translation_thread_count,
            "translationBatchSize": settings.translation_batch_size,
            "smartSegmentation": settings.is_smart_segmentation_enabled,
            "subtitleCorrection": settings.is_subtitle_correction_enabled,
            "subtitleTranslation": settings.is_subtitle_translation_enabled,
            "aiSubtitleReview": settings.is_ai_subtitle_review_enabled,
        }),
    );

    if let Err(error) = store.save(&settings) {
        app_logger.error(
            "settings",
            "save_failed",
            "保存应用设置失败",
            serde_json::json!({ "error": &error }),
        );
        return Err(error);
    }

    if let Err(error) = ai_service.update_thread_count(settings.translation_thread_count) {
        app_logger.error(
            "settings",
            "ai_concurrency_update_failed",
            "保存设置后更新 AI 并发限制失败",
            serde_json::json!({ "error": &error }),
        );
        return Err(error);
    }

    app_logger.info(
        "settings",
        "save_success",
        "应用设置已保存",
        serde_json::json!({
            "selectedLlmService": &settings.selected_llm_service,
            "translationThreadCount": settings.translation_thread_count,
        }),
    );
    Ok(())
}

#[tauri::command]
pub fn export_settings_backup(
    store: tauri::State<'_, SettingsStore>,
    app_logger: tauri::State<'_, AppLogger>,
    path: String,
) -> Result<SettingsBackupSummary, String> {
    let path = settings_backup_path(&path)?;
    app_logger.info(
        "settings",
        "backup_export_start",
        "开始导出设置备份",
        serde_json::json!({ "path": path.display().to_string() }),
    );

    let backup = match store.export_backup() {
        Ok(backup) => backup,
        Err(error) => {
            app_logger.error(
                "settings",
                "backup_export_failed",
                "读取设置备份数据失败",
                serde_json::json!({ "error": &error }),
            );
            return Err(error);
        }
    };
    let summary = SettingsBackupSummary {
        setting_count: backup_setting_count(),
        channel_count: backup.youtube_monitor.channels.len(),
        added_channel_count: 0,
        updated_channel_count: 0,
        subtitle_style_count: backup.subtitle_styles.styles.len(),
        added_subtitle_style_count: 0,
        updated_subtitle_style_count: 0,
        dubbing_model_count: backup.dubbing.models.len(),
        added_dubbing_model_count: 0,
        updated_dubbing_model_count: 0,
    };
    let content = serde_json::to_string_pretty(&backup)
        .map_err(|error| format!("无法生成设置备份 JSON: {error}"))?;

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|error| format!("无法创建备份目录: {error}"))?;
        }
    }

    if let Err(error) = fs::write(&path, content) {
        let message = format!("无法写入设置备份文件: {error}");
        app_logger.error(
            "settings",
            "backup_export_failed",
            "写入设置备份文件失败",
            serde_json::json!({ "error": &message }),
        );
        return Err(message);
    }

    app_logger.info(
        "settings",
        "backup_export_success",
        "设置备份已导出",
        serde_json::json!({
            "settingCount": summary.setting_count,
            "channelCount": summary.channel_count,
            "subtitleStyleCount": summary.subtitle_style_count,
            "dubbingModelCount": summary.dubbing_model_count,
        }),
    );
    Ok(summary)
}

#[tauri::command]
pub fn import_settings_backup(
    app: AppHandle,
    store: tauri::State<'_, SettingsStore>,
    ai_service: tauri::State<'_, AiService>,
    app_logger: tauri::State<'_, AppLogger>,
    path: String,
) -> Result<SettingsBackupSummary, String> {
    let path = settings_backup_path(&path)?;
    app_logger.info(
        "settings",
        "backup_import_start",
        "开始导入设置备份",
        serde_json::json!({ "path": path.display().to_string() }),
    );

    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) => {
            let message = format!("无法读取设置备份文件: {error}");
            app_logger.error(
                "settings",
                "backup_import_failed",
                "读取设置备份文件失败",
                serde_json::json!({ "error": &message }),
            );
            return Err(message);
        }
    };
    let backup: SettingsBackup = match serde_json::from_str(&content) {
        Ok(backup) => backup,
        Err(error) => {
            let message = format!("设置备份 JSON 格式无效: {error}");
            app_logger.error(
                "settings",
                "backup_import_failed",
                "解析设置备份失败",
                serde_json::json!({ "error": &message }),
            );
            return Err(message);
        }
    };
    if backup.schema_version < MIN_SETTINGS_BACKUP_SCHEMA_VERSION
        || backup.schema_version > SETTINGS_BACKUP_SCHEMA_VERSION
    {
        return Err(format!("不支持的设置备份版本: {}", backup.schema_version));
    }

    let channel_count = backup.youtube_monitor.channels.len();
    let subtitle_style_count = backup.subtitle_styles.styles.len();
    let dubbing_model_count = backup.dubbing.models.len();
    let translation_thread_count = backup.settings.translation_thread_count;
    let import_summary = match store.import_backup(&backup) {
        Ok(summary) => summary,
        Err(error) => {
            app_logger.error(
                "settings",
                "backup_import_failed",
                "写入设置备份数据失败",
                serde_json::json!({ "error": &error }),
            );
            return Err(error);
        }
    };

    if let Err(error) = ai_service.update_thread_count(translation_thread_count) {
        app_logger.error(
            "settings",
            "backup_import_ai_concurrency_update_failed",
            "导入设置备份后更新 AI 并发限制失败",
            serde_json::json!({ "error": &error }),
        );
        return Err(error);
    }

    let summary = SettingsBackupSummary {
        setting_count: backup_setting_count(),
        channel_count,
        added_channel_count: import_summary.added_channel_count,
        updated_channel_count: import_summary.updated_channel_count,
        subtitle_style_count,
        added_subtitle_style_count: import_summary.added_subtitle_style_count,
        updated_subtitle_style_count: import_summary.updated_subtitle_style_count,
        dubbing_model_count,
        added_dubbing_model_count: import_summary.added_dubbing_model_count,
        updated_dubbing_model_count: import_summary.updated_dubbing_model_count,
    };
    if summary.added_dubbing_model_count > 0 || summary.updated_dubbing_model_count > 0 {
        let _ = app.emit(DUBBING_MODELS_EVENT, serde_json::json!({}));
    }
    app_logger.info(
        "settings",
        "backup_import_success",
        "设置备份已导入",
        serde_json::json!({
            "settingCount": summary.setting_count,
            "channelCount": summary.channel_count,
            "addedChannelCount": summary.added_channel_count,
            "updatedChannelCount": summary.updated_channel_count,
            "subtitleStyleCount": summary.subtitle_style_count,
            "addedSubtitleStyleCount": summary.added_subtitle_style_count,
            "updatedSubtitleStyleCount": summary.updated_subtitle_style_count,
            "dubbingModelCount": summary.dubbing_model_count,
            "addedDubbingModelCount": summary.added_dubbing_model_count,
            "updatedDubbingModelCount": summary.updated_dubbing_model_count,
        }),
    );
    Ok(summary)
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

            DELETE FROM app_settings
            WHERE key = 'ytdlp_cookies_path';

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

            CREATE TABLE IF NOT EXISTS home_video_task_download_states (
                task_id TEXT PRIMARY KEY NOT NULL,
                downloaded_bytes INTEGER NOT NULL DEFAULT 0,
                total_bytes INTEGER,
                progress INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT '',
                message TEXT NOT NULL DEFAULT '',
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

            CREATE TABLE IF NOT EXISTS home_workbench_checkpoints (
                task_id TEXT NOT NULL,
                scope TEXT NOT NULL,
                checkpoint_key TEXT NOT NULL,
                input_key TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'active',
                payload TEXT NOT NULL DEFAULT 'null',
                error_message TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY(task_id, scope, checkpoint_key),
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
            CREATE INDEX IF NOT EXISTS idx_youtube_videos_unread_rank
                ON youtube_videos(is_unread, published_rank, first_seen_at);
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
            CREATE INDEX IF NOT EXISTS idx_home_video_task_download_states_updated_at
                ON home_video_task_download_states(updated_at);
            CREATE INDEX IF NOT EXISTS idx_home_workbench_tasks_updated_at
                ON home_workbench_tasks(updated_at);
            CREATE INDEX IF NOT EXISTS idx_home_workbench_artifacts_task
                ON home_workbench_artifacts(task_id, updated_at);
            CREATE INDEX IF NOT EXISTS idx_home_workbench_checkpoints_task
                ON home_workbench_checkpoints(task_id, scope, updated_at);
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

fn save_settings_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    settings: &AppSettings,
) -> Result<(), String> {
    upsert_setting(transaction, "theme", &settings.theme)?;
    upsert_setting(
        transaction,
        "transcription_model",
        &settings.transcription_model,
    )?;
    upsert_setting(transaction, "source_language", &settings.source_language)?;
    upsert_setting(
        transaction,
        "transcription_format",
        &settings.transcription_format,
    )?;
    upsert_setting(
        transaction,
        "translation_format",
        &settings.translation_format,
    )?;
    upsert_setting(
        transaction,
        "selected_subtitle_style_id",
        &settings.selected_subtitle_style_id,
    )?;
    upsert_setting(
        transaction,
        "is_smart_segmentation_enabled",
        bool_to_text(settings.is_smart_segmentation_enabled),
    )?;
    upsert_setting(
        transaction,
        "selected_llm_service",
        &settings.selected_llm_service,
    )?;
    upsert_setting(
        transaction,
        "translation_service",
        &settings.translation_service,
    )?;
    upsert_setting(
        transaction,
        "needs_reflection_translation",
        bool_to_text(settings.needs_reflection_translation),
    )?;
    upsert_setting(
        transaction,
        "translation_batch_size",
        &settings.translation_batch_size.to_string(),
    )?;
    upsert_setting(
        transaction,
        "translation_thread_count",
        &settings.translation_thread_count.to_string(),
    )?;
    upsert_setting(
        transaction,
        "video_content_type",
        &settings.video_content_type,
    )?;
    upsert_setting(transaction, "output_mode", &settings.output_mode)?;
    upsert_setting(
        transaction,
        "is_subtitle_correction_enabled",
        bool_to_text(settings.is_subtitle_correction_enabled),
    )?;
    upsert_setting(
        transaction,
        "is_subtitle_translation_enabled",
        bool_to_text(settings.is_subtitle_translation_enabled),
    )?;
    upsert_setting(
        transaction,
        "is_ai_subtitle_review_enabled",
        bool_to_text(settings.is_ai_subtitle_review_enabled),
    )?;
    upsert_setting(
        transaction,
        "ai_subtitle_review_mode",
        &settings.ai_subtitle_review_mode,
    )?;
    upsert_setting(transaction, "target_language", &settings.target_language)?;
    upsert_setting(
        transaction,
        "dubbing_tts_interval_ms",
        &settings.dubbing_tts_interval_ms.to_string(),
    )?;
    upsert_setting(
        transaction,
        "dubbing_reference_audio_source",
        &settings.dubbing_reference_audio_source,
    )?;
    upsert_setting(
        transaction,
        "dubbing_custom_reference_audio_path",
        &settings.dubbing_custom_reference_audio_path,
    )?;
    upsert_setting(
        transaction,
        "dubbing_is_background_music_enabled",
        bool_to_text(settings.dubbing_is_background_music_enabled),
    )?;
    upsert_setting(
        transaction,
        "dubbing_background_music_volume",
        &settings.dubbing_background_music_volume.to_string(),
    )?;
    upsert_setting(
        transaction,
        "home_workbench_translation_enabled",
        bool_to_text(settings.home_workbench_translation_enabled),
    )?;
    upsert_setting(
        transaction,
        "home_workbench_dubbing_enabled",
        bool_to_text(settings.home_workbench_dubbing_enabled),
    )?;
    upsert_setting(
        transaction,
        "home_workbench_export_dir",
        &settings.home_workbench_export_dir,
    )?;
    upsert_setting(transaction, "ytdlp_proxy", &settings.ytdlp_proxy)?;

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

    Ok(())
}

fn read_backup_youtube_channels(
    connection: &Connection,
) -> Result<Vec<BackupYoutubeChannel>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, url, canonical_url, external_id, title, handle, description,
                   thumbnail_url, created_at, updated_at
            FROM youtube_channels
            ORDER BY datetime(created_at) ASC, title COLLATE NOCASE ASC
            ",
        )
        .map_err(|error| format!("无法读取监控博主: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok(BackupYoutubeChannel {
                id: row.get(0)?,
                url: row.get(1)?,
                canonical_url: row.get(2)?,
                external_id: row.get(3)?,
                title: row.get(4)?,
                handle: row.get(5)?,
                description: row.get(6)?,
                thumbnail_url: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })
        .map_err(|error| format!("无法读取监控博主: {error}"))?;

    let mut channels = Vec::new();
    for row in rows {
        channels.push(row.map_err(|error| format!("无法解析监控博主: {error}"))?);
    }

    Ok(channels)
}

fn import_backup_youtube_channels(
    transaction: &rusqlite::Transaction<'_>,
    channels: &[BackupYoutubeChannel],
) -> Result<SettingsBackupImportSummary, String> {
    let mut summary = SettingsBackupImportSummary::default();

    for channel in channels {
        let prepared = prepare_backup_youtube_channel(channel)?;
        let existing_id = find_existing_backup_youtube_channel(transaction, &prepared)?;

        if let Some(existing_id) = existing_id {
            transaction
                .execute(
                    "
                    UPDATE youtube_channels
                    SET url = ?1,
                        canonical_url = ?2,
                        external_id = ?3,
                        title = ?4,
                        handle = ?5,
                        description = ?6,
                        thumbnail_url = ?7,
                        status = ?8,
                        last_error = '',
                        updated_at = ?9
                    WHERE id = ?10
                    ",
                    params![
                        prepared.url,
                        prepared.canonical_url,
                        prepared.external_id,
                        prepared.title,
                        prepared.handle,
                        prepared.description,
                        prepared.thumbnail_url,
                        YOUTUBE_CHANNEL_STATUS_IDLE,
                        prepared.updated_at,
                        existing_id,
                    ],
                )
                .map_err(|error| format!("无法更新监控博主备份数据: {error}"))?;
            summary.updated_channel_count += 1;
            continue;
        }

        transaction
            .execute(
                "
                INSERT INTO youtube_channels (
                    id, url, canonical_url, external_id, title, handle, description,
                    thumbnail_url, status, last_checked_at, last_success_at, last_error,
                    latest_video_external_id, video_count, unread_count, created_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, NULL, '', '', 0, 0, ?10, ?11)
                ",
                params![
                    prepared.id,
                    prepared.url,
                    prepared.canonical_url,
                    prepared.external_id,
                    prepared.title,
                    prepared.handle,
                    prepared.description,
                    prepared.thumbnail_url,
                    YOUTUBE_CHANNEL_STATUS_IDLE,
                    prepared.created_at,
                    prepared.updated_at,
                ],
            )
            .map_err(|error| format!("无法新增监控博主备份数据: {error}"))?;
        summary.added_channel_count += 1;
    }

    Ok(summary)
}

fn find_existing_backup_youtube_channel(
    transaction: &rusqlite::Transaction<'_>,
    channel: &BackupYoutubeChannel,
) -> Result<Option<String>, String> {
    for (column, value) in [
        ("url", channel.url.as_str()),
        ("canonical_url", channel.canonical_url.as_str()),
        ("external_id", channel.external_id.as_str()),
        ("id", channel.id.as_str()),
    ] {
        if value.trim().is_empty() {
            continue;
        }

        let existing_id = transaction
            .query_row(
                &format!("SELECT id FROM youtube_channels WHERE {column} = ?1 LIMIT 1"),
                params![value],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("无法检查监控博主是否已存在: {error}"))?;

        if existing_id.is_some() {
            return Ok(existing_id);
        }
    }

    Ok(None)
}

fn prepare_backup_youtube_channel(
    channel: &BackupYoutubeChannel,
) -> Result<BackupYoutubeChannel, String> {
    let url = channel.url.trim();
    if url.is_empty() {
        return Err("设置备份中存在缺少 URL 的监控博主".to_string());
    }

    let now = Utc::now().to_rfc3339();
    Ok(BackupYoutubeChannel {
        id: read_backup_id(&channel.id),
        url: url.to_string(),
        canonical_url: channel.canonical_url.trim().to_string(),
        external_id: channel.external_id.trim().to_string(),
        title: channel.title.trim().to_string(),
        handle: channel.handle.trim().to_string(),
        description: channel.description.trim().to_string(),
        thumbnail_url: channel.thumbnail_url.trim().to_string(),
        created_at: read_backup_timestamp(&channel.created_at, &now),
        updated_at: read_backup_timestamp(&channel.updated_at, &now),
    })
}

fn read_backup_subtitle_styles(
    connection: &Connection,
) -> Result<Vec<BackupSubtitleStyle>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, name, is_default,
                   render_mode, subtitle_layout, preview_text_mode,
                   primary_font_name, primary_font_size, primary_color,
                   primary_outline_color, primary_outline_width, primary_spacing,
                   primary_margin_bottom,
                   secondary_font_name, secondary_font_size, secondary_color,
                   secondary_outline_color, secondary_outline_width, secondary_spacing,
                   vertical_spacing,
                   rounded_font_name, rounded_font_size, rounded_text_color,
                   rounded_background_color, rounded_corner_radius, rounded_padding_x,
                   rounded_padding_y, rounded_margin_bottom, rounded_line_spacing,
                   rounded_letter_spacing,
                   created_at, updated_at
            FROM subtitle_styles
            ORDER BY is_default DESC, datetime(created_at) ASC, name COLLATE NOCASE ASC
            ",
        )
        .map_err(|error| format!("无法读取字幕样式备份: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok(BackupSubtitleStyle {
                id: row.get(0)?,
                name: row.get(1)?,
                is_default: row.get::<_, i64>(2)? != 0,
                render_mode: row.get(3)?,
                subtitle_layout: row.get(4)?,
                preview_text_mode: row.get(5)?,
                primary_font_name: row.get(6)?,
                primary_font_size: row.get(7)?,
                primary_color: row.get(8)?,
                primary_outline_color: row.get(9)?,
                primary_outline_width: row.get(10)?,
                primary_spacing: row.get(11)?,
                primary_margin_bottom: row.get(12)?,
                secondary_font_name: row.get(13)?,
                secondary_font_size: row.get(14)?,
                secondary_color: row.get(15)?,
                secondary_outline_color: row.get(16)?,
                secondary_outline_width: row.get(17)?,
                secondary_spacing: row.get(18)?,
                vertical_spacing: row.get(19)?,
                rounded_font_name: row.get(20)?,
                rounded_font_size: row.get(21)?,
                rounded_text_color: row.get(22)?,
                rounded_background_color: row.get(23)?,
                rounded_corner_radius: row.get(24)?,
                rounded_padding_x: row.get(25)?,
                rounded_padding_y: row.get(26)?,
                rounded_margin_bottom: row.get(27)?,
                rounded_line_spacing: row.get(28)?,
                rounded_letter_spacing: row.get(29)?,
                created_at: row.get(30)?,
                updated_at: row.get(31)?,
            })
        })
        .map_err(|error| format!("无法读取字幕样式备份: {error}"))?;

    let mut styles = Vec::new();
    for row in rows {
        styles.push(row.map_err(|error| format!("无法解析字幕样式备份: {error}"))?);
    }

    Ok(styles)
}

fn import_backup_subtitle_styles(
    transaction: &rusqlite::Transaction<'_>,
    styles: &[BackupSubtitleStyle],
) -> Result<SettingsBackupImportSummary, String> {
    let mut summary = SettingsBackupImportSummary::default();
    if styles.is_empty() {
        return Ok(summary);
    }

    let mut default_style_id = None;

    for style in styles {
        let prepared = prepare_backup_subtitle_style(style)?;
        if prepared.is_default && default_style_id.is_none() {
            default_style_id = Some(prepared.id.clone());
        }

        let existing_id = find_existing_backup_subtitle_style(transaction, &prepared)?;
        if let Some(existing_id) = existing_id {
            rename_conflicting_subtitle_style_name(transaction, &prepared.name, &existing_id)?;
            update_backup_subtitle_style(transaction, &existing_id, &prepared)?;
            summary.updated_subtitle_style_count += 1;
            continue;
        }

        insert_backup_subtitle_style(transaction, &prepared)?;
        summary.added_subtitle_style_count += 1;
    }

    if let Some(style_id) = default_style_id {
        set_single_default_subtitle_style(transaction, &style_id)?;
    } else {
        ensure_any_default_subtitle_style(transaction)?;
    }

    Ok(summary)
}

fn prepare_backup_subtitle_style(
    style: &BackupSubtitleStyle,
) -> Result<BackupSubtitleStyle, String> {
    let name = style.name.trim();
    if name.is_empty() {
        return Err("设置备份中存在缺少名称的字幕样式".to_string());
    }

    let now = Utc::now().to_rfc3339();
    Ok(BackupSubtitleStyle {
        id: read_backup_id(&style.id),
        name: name.to_string(),
        is_default: style.is_default,
        render_mode: read_backup_string(&style.render_mode, "ass"),
        subtitle_layout: read_backup_string(&style.subtitle_layout, "target-above"),
        preview_text_mode: read_backup_string(&style.preview_text_mode, "medium"),
        primary_font_name: read_backup_string(&style.primary_font_name, "Microsoft YaHei"),
        primary_font_size: style.primary_font_size,
        primary_color: read_backup_string(&style.primary_color, "#FFFFFF"),
        primary_outline_color: read_backup_string(&style.primary_outline_color, "#000000"),
        primary_outline_width: style.primary_outline_width,
        primary_spacing: style.primary_spacing,
        primary_margin_bottom: style.primary_margin_bottom,
        secondary_font_name: read_backup_string(&style.secondary_font_name, "Microsoft YaHei"),
        secondary_font_size: style.secondary_font_size,
        secondary_color: read_backup_string(&style.secondary_color, "#FFFFFF"),
        secondary_outline_color: read_backup_string(&style.secondary_outline_color, "#000000"),
        secondary_outline_width: style.secondary_outline_width,
        secondary_spacing: style.secondary_spacing,
        vertical_spacing: style.vertical_spacing,
        rounded_font_name: read_backup_string(&style.rounded_font_name, "Microsoft YaHei"),
        rounded_font_size: style.rounded_font_size,
        rounded_text_color: read_backup_string(&style.rounded_text_color, "#FFFFFF"),
        rounded_background_color: read_backup_string(&style.rounded_background_color, "#191919CC"),
        rounded_corner_radius: style.rounded_corner_radius,
        rounded_padding_x: style.rounded_padding_x,
        rounded_padding_y: style.rounded_padding_y,
        rounded_margin_bottom: style.rounded_margin_bottom,
        rounded_line_spacing: style.rounded_line_spacing,
        rounded_letter_spacing: style.rounded_letter_spacing,
        created_at: read_backup_timestamp(&style.created_at, &now),
        updated_at: read_backup_timestamp(&style.updated_at, &now),
    })
}

fn find_existing_backup_subtitle_style(
    transaction: &rusqlite::Transaction<'_>,
    style: &BackupSubtitleStyle,
) -> Result<Option<String>, String> {
    if let Some(existing_id) = transaction
        .query_row(
            "SELECT id FROM subtitle_styles WHERE id = ?1 LIMIT 1",
            params![&style.id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("无法检查字幕样式是否已存在: {error}"))?
    {
        return Ok(Some(existing_id));
    }

    transaction
        .query_row(
            "SELECT id FROM subtitle_styles WHERE name = ?1 LIMIT 1",
            params![&style.name],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("无法检查字幕样式名称是否已存在: {error}"))
}

fn rename_conflicting_subtitle_style_name(
    transaction: &rusqlite::Transaction<'_>,
    name: &str,
    keep_id: &str,
) -> Result<(), String> {
    let conflicting_id = transaction
        .query_row(
            "SELECT id FROM subtitle_styles WHERE name = ?1 AND id <> ?2 LIMIT 1",
            params![name, keep_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("无法检查字幕样式名称冲突: {error}"))?;

    if let Some(conflicting_id) = conflicting_id {
        let renamed = unique_subtitle_style_name(transaction, name, Some(&conflicting_id))?;
        transaction
            .execute(
                "UPDATE subtitle_styles SET name = ?2, updated_at = ?3 WHERE id = ?1",
                params![conflicting_id, renamed, Utc::now().to_rfc3339()],
            )
            .map_err(|error| format!("无法保留本机同名字幕样式: {error}"))?;
    }

    Ok(())
}

fn unique_subtitle_style_name(
    transaction: &rusqlite::Transaction<'_>,
    name: &str,
    exclude_id: Option<&str>,
) -> Result<String, String> {
    for index in 1..=100 {
        let candidate = if index == 1 {
            format!("{name}（本机保留）")
        } else {
            format!("{name}（本机保留 {index}）")
        };

        let exists = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM subtitle_styles WHERE name = ?1 AND (?2 IS NULL OR id <> ?2))",
                params![candidate, exclude_id],
                |row| row.get::<_, i64>(0),
            )
            .map(|value| value != 0)
            .map_err(|error| format!("无法生成字幕样式保留名称: {error}"))?;

        if !exists {
            return Ok(candidate);
        }
    }

    Err("无法生成不冲突的字幕样式名称".to_string())
}

fn update_backup_subtitle_style(
    transaction: &rusqlite::Transaction<'_>,
    existing_id: &str,
    style: &BackupSubtitleStyle,
) -> Result<(), String> {
    transaction
        .execute(
            "
            UPDATE subtitle_styles
            SET id = ?1,
                name = ?2,
                is_default = 0,
                render_mode = ?3,
                subtitle_layout = ?4,
                preview_text_mode = ?5,
                primary_font_name = ?6,
                primary_font_size = ?7,
                primary_color = ?8,
                primary_outline_color = ?9,
                primary_outline_width = ?10,
                primary_spacing = ?11,
                primary_margin_bottom = ?12,
                secondary_font_name = ?13,
                secondary_font_size = ?14,
                secondary_color = ?15,
                secondary_outline_color = ?16,
                secondary_outline_width = ?17,
                secondary_spacing = ?18,
                vertical_spacing = ?19,
                rounded_font_name = ?20,
                rounded_font_size = ?21,
                rounded_text_color = ?22,
                rounded_background_color = ?23,
                rounded_corner_radius = ?24,
                rounded_padding_x = ?25,
                rounded_padding_y = ?26,
                rounded_margin_bottom = ?27,
                rounded_line_spacing = ?28,
                rounded_letter_spacing = ?29,
                created_at = ?30,
                updated_at = ?31
            WHERE id = ?32
            ",
            params![
                &style.id,
                &style.name,
                &style.render_mode,
                &style.subtitle_layout,
                &style.preview_text_mode,
                &style.primary_font_name,
                style.primary_font_size,
                &style.primary_color,
                &style.primary_outline_color,
                style.primary_outline_width,
                style.primary_spacing,
                style.primary_margin_bottom,
                &style.secondary_font_name,
                style.secondary_font_size,
                &style.secondary_color,
                &style.secondary_outline_color,
                style.secondary_outline_width,
                style.secondary_spacing,
                style.vertical_spacing,
                &style.rounded_font_name,
                style.rounded_font_size,
                &style.rounded_text_color,
                &style.rounded_background_color,
                style.rounded_corner_radius,
                style.rounded_padding_x,
                style.rounded_padding_y,
                style.rounded_margin_bottom,
                style.rounded_line_spacing,
                style.rounded_letter_spacing,
                &style.created_at,
                &style.updated_at,
                existing_id,
            ],
        )
        .map(|_| ())
        .map_err(|error| format!("无法更新字幕样式备份数据: {error}"))
}

fn insert_backup_subtitle_style(
    transaction: &rusqlite::Transaction<'_>,
    style: &BackupSubtitleStyle,
) -> Result<(), String> {
    transaction
        .execute(
            "
            INSERT INTO subtitle_styles (
                id, name, is_default,
                render_mode, subtitle_layout, preview_text_mode,
                primary_font_name, primary_font_size, primary_color,
                primary_outline_color, primary_outline_width, primary_spacing,
                primary_margin_bottom,
                secondary_font_name, secondary_font_size, secondary_color,
                secondary_outline_color, secondary_outline_width, secondary_spacing,
                vertical_spacing,
                rounded_font_name, rounded_font_size, rounded_text_color,
                rounded_background_color, rounded_corner_radius, rounded_padding_x,
                rounded_padding_y, rounded_margin_bottom, rounded_line_spacing,
                rounded_letter_spacing,
                created_at, updated_at
            )
            VALUES (
                ?1, ?2, 0, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23,
                ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31
            )
            ",
            params![
                &style.id,
                &style.name,
                &style.render_mode,
                &style.subtitle_layout,
                &style.preview_text_mode,
                &style.primary_font_name,
                style.primary_font_size,
                &style.primary_color,
                &style.primary_outline_color,
                style.primary_outline_width,
                style.primary_spacing,
                style.primary_margin_bottom,
                &style.secondary_font_name,
                style.secondary_font_size,
                &style.secondary_color,
                &style.secondary_outline_color,
                style.secondary_outline_width,
                style.secondary_spacing,
                style.vertical_spacing,
                &style.rounded_font_name,
                style.rounded_font_size,
                &style.rounded_text_color,
                &style.rounded_background_color,
                style.rounded_corner_radius,
                style.rounded_padding_x,
                style.rounded_padding_y,
                style.rounded_margin_bottom,
                style.rounded_line_spacing,
                style.rounded_letter_spacing,
                &style.created_at,
                &style.updated_at,
            ],
        )
        .map(|_| ())
        .map_err(|error| format!("无法新增字幕样式备份数据: {error}"))
}

fn set_single_default_subtitle_style(
    transaction: &rusqlite::Transaction<'_>,
    style_id: &str,
) -> Result<(), String> {
    let fallback_id = if subtitle_style_exists(transaction, style_id)? {
        style_id.to_string()
    } else {
        fallback_subtitle_style_id(transaction)?
    };

    transaction
        .execute(
            "UPDATE subtitle_styles SET is_default = CASE WHEN id = ?1 THEN 1 ELSE 0 END",
            params![fallback_id],
        )
        .map(|_| ())
        .map_err(|error| format!("无法恢复默认字幕样式: {error}"))
}

fn ensure_any_default_subtitle_style(
    transaction: &rusqlite::Transaction<'_>,
) -> Result<(), String> {
    let has_default = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM subtitle_styles WHERE is_default = 1)",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value != 0)
        .map_err(|error| format!("无法检查默认字幕样式: {error}"))?;

    if has_default {
        return Ok(());
    }

    let fallback_id = fallback_subtitle_style_id(transaction)?;
    set_single_default_subtitle_style(transaction, &fallback_id)
}

fn ensure_selected_backup_subtitle_style(
    transaction: &rusqlite::Transaction<'_>,
    selected_style_id: &str,
) -> Result<(), String> {
    let selected_style_id = selected_style_id.trim();
    let restored_id = if !selected_style_id.is_empty()
        && subtitle_style_exists(transaction, selected_style_id)?
    {
        selected_style_id.to_string()
    } else {
        fallback_subtitle_style_id(transaction)?
    };

    upsert_setting(transaction, "selected_subtitle_style_id", &restored_id)
}

fn subtitle_style_exists(
    transaction: &rusqlite::Transaction<'_>,
    style_id: &str,
) -> Result<bool, String> {
    transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM subtitle_styles WHERE id = ?1)",
            params![style_id],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value != 0)
        .map_err(|error| format!("无法检查字幕样式是否存在: {error}"))
}

fn fallback_subtitle_style_id(transaction: &rusqlite::Transaction<'_>) -> Result<String, String> {
    for sql in [
        "SELECT id FROM subtitle_styles WHERE id = 'default' LIMIT 1",
        "SELECT id FROM subtitle_styles WHERE is_default = 1 ORDER BY datetime(created_at) ASC LIMIT 1",
        "SELECT id FROM subtitle_styles ORDER BY datetime(created_at) ASC LIMIT 1",
    ] {
        if let Some(id) = transaction
            .query_row(sql, [], |row| row.get::<_, String>(0))
            .optional()
            .map_err(|error| format!("无法读取备用字幕样式: {error}"))?
        {
            return Ok(id);
        }
    }

    Err("未找到可用字幕样式".to_string())
}

fn read_backup_dubbing_models(connection: &Connection) -> Result<Vec<BackupDubbingModel>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, engine, model_key, display_name, locale, gender, enabled, metadata,
                   scheduler_weight, success_count, failure_count, consecutive_failures,
                   avg_latency_ms, cooldown_until, last_error, last_used_at, last_checked_at,
                   created_at, updated_at
            FROM dubbing_models
            ORDER BY datetime(created_at) ASC, display_name COLLATE NOCASE ASC
            ",
        )
        .map_err(|error| format!("无法读取配音模型备份: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            let metadata_text: String = row.get(7)?;
            let metadata =
                serde_json::from_str(&metadata_text).unwrap_or_else(|_| serde_json::json!({}));
            Ok(BackupDubbingModel {
                id: row.get(0)?,
                engine: row.get(1)?,
                model_key: row.get(2)?,
                display_name: row.get(3)?,
                locale: row.get(4)?,
                gender: row.get(5)?,
                enabled: row.get::<_, i64>(6)? != 0,
                metadata,
                scheduler_weight: row.get(8)?,
                success_count: row.get::<_, i64>(9)?.max(0) as u64,
                failure_count: row.get::<_, i64>(10)?.max(0) as u64,
                consecutive_failures: row.get::<_, i64>(11)?.max(0) as u64,
                avg_latency_ms: row
                    .get::<_, Option<i64>>(12)?
                    .map(|value| value.max(0) as u64),
                cooldown_until: row.get(13)?,
                last_error: row.get(14)?,
                last_used_at: row.get(15)?,
                last_checked_at: row.get(16)?,
                created_at: row.get(17)?,
                updated_at: row.get(18)?,
            })
        })
        .map_err(|error| format!("无法读取配音模型备份: {error}"))?;

    let mut models = Vec::new();
    for row in rows {
        models.push(row.map_err(|error| format!("无法解析配音模型备份: {error}"))?);
    }

    Ok(models)
}

fn import_backup_dubbing_models(
    transaction: &rusqlite::Transaction<'_>,
    models: &[BackupDubbingModel],
) -> Result<SettingsBackupImportSummary, String> {
    let mut summary = SettingsBackupImportSummary::default();

    for model in models {
        let prepared = prepare_backup_dubbing_model(model)?;
        let existing_id = find_existing_backup_dubbing_model(transaction, &prepared)?;
        if let Some(existing_id) = existing_id {
            update_backup_dubbing_model(transaction, &existing_id, &prepared)?;
            summary.updated_dubbing_model_count += 1;
            continue;
        }

        insert_backup_dubbing_model(transaction, &prepared)?;
        summary.added_dubbing_model_count += 1;
    }

    Ok(summary)
}

fn prepare_backup_dubbing_model(model: &BackupDubbingModel) -> Result<BackupDubbingModel, String> {
    let engine = model.engine.trim();
    let model_key = model.model_key.trim();
    if engine.is_empty() || model_key.is_empty() {
        return Err("设置备份中存在缺少引擎或模型标识的配音模型".to_string());
    }

    let now = Utc::now().to_rfc3339();
    Ok(BackupDubbingModel {
        id: read_backup_id(&model.id),
        engine: engine.to_string(),
        model_key: model_key.to_string(),
        display_name: read_backup_string(&model.display_name, model_key),
        locale: model.locale.trim().to_string(),
        gender: model.gender.trim().to_string(),
        enabled: model.enabled,
        metadata: model.metadata.clone(),
        scheduler_weight: model.scheduler_weight.clamp(10.0, 200.0),
        success_count: model.success_count,
        failure_count: model.failure_count,
        consecutive_failures: model.consecutive_failures,
        avg_latency_ms: model.avg_latency_ms,
        cooldown_until: read_optional_backup_timestamp(&model.cooldown_until),
        last_error: model.last_error.trim().to_string(),
        last_used_at: read_optional_backup_timestamp(&model.last_used_at),
        last_checked_at: read_optional_backup_timestamp(&model.last_checked_at),
        created_at: read_backup_timestamp(&model.created_at, &now),
        updated_at: read_backup_timestamp(&model.updated_at, &now),
    })
}

fn find_existing_backup_dubbing_model(
    transaction: &rusqlite::Transaction<'_>,
    model: &BackupDubbingModel,
) -> Result<Option<String>, String> {
    if let Some(existing_id) = transaction
        .query_row(
            "SELECT id FROM dubbing_models WHERE id = ?1 LIMIT 1",
            params![&model.id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("无法检查配音模型是否已存在: {error}"))?
    {
        return Ok(Some(existing_id));
    }

    let mut statement = transaction
        .prepare(
            "
            SELECT id, metadata
            FROM dubbing_models
            WHERE engine = ?1 AND model_key = ?2
            ORDER BY datetime(created_at) ASC
            ",
        )
        .map_err(|error| format!("无法检查配音模型合集: {error}"))?;
    let rows = statement
        .query_map(params![&model.engine, &model.model_key], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("无法检查配音模型合集: {error}"))?;

    for row in rows {
        let (id, metadata_text) = row.map_err(|error| format!("无法解析配音模型合集: {error}"))?;
        let metadata =
            serde_json::from_str::<Value>(&metadata_text).unwrap_or_else(|_| serde_json::json!({}));
        if metadata == model.metadata {
            return Ok(Some(id));
        }
    }

    Ok(None)
}

fn update_backup_dubbing_model(
    transaction: &rusqlite::Transaction<'_>,
    existing_id: &str,
    model: &BackupDubbingModel,
) -> Result<(), String> {
    let metadata = serde_json::to_string(&model.metadata)
        .map_err(|error| format!("无法序列化配音模型: {error}"))?;
    transaction
        .execute(
            "
            UPDATE dubbing_models
            SET id = ?1,
                engine = ?2,
                model_key = ?3,
                display_name = ?4,
                locale = ?5,
                gender = ?6,
                enabled = ?7,
                metadata = ?8,
                scheduler_weight = ?9,
                success_count = ?10,
                failure_count = ?11,
                consecutive_failures = ?12,
                avg_latency_ms = ?13,
                cooldown_until = ?14,
                last_error = ?15,
                last_used_at = ?16,
                last_checked_at = ?17,
                created_at = ?18,
                updated_at = ?19
            WHERE id = ?20
            ",
            params![
                &model.id,
                &model.engine,
                &model.model_key,
                &model.display_name,
                &model.locale,
                &model.gender,
                if model.enabled { 1 } else { 0 },
                metadata,
                model.scheduler_weight,
                u64_to_i64(model.success_count),
                u64_to_i64(model.failure_count),
                u64_to_i64(model.consecutive_failures),
                model.avg_latency_ms.map(u64_to_i64),
                model.cooldown_until.as_deref(),
                &model.last_error,
                model.last_used_at.as_deref(),
                model.last_checked_at.as_deref(),
                &model.created_at,
                &model.updated_at,
                existing_id,
            ],
        )
        .map(|_| ())
        .map_err(|error| format!("无法更新配音模型备份数据: {error}"))
}

fn insert_backup_dubbing_model(
    transaction: &rusqlite::Transaction<'_>,
    model: &BackupDubbingModel,
) -> Result<(), String> {
    let metadata = serde_json::to_string(&model.metadata)
        .map_err(|error| format!("无法序列化配音模型: {error}"))?;
    transaction
        .execute(
            "
            INSERT INTO dubbing_models (
                id, engine, model_key, display_name, locale, gender, enabled, metadata,
                scheduler_weight, success_count, failure_count, consecutive_failures,
                avg_latency_ms, cooldown_until, last_error, last_used_at, last_checked_at,
                created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
            ",
            params![
                &model.id,
                &model.engine,
                &model.model_key,
                &model.display_name,
                &model.locale,
                &model.gender,
                if model.enabled { 1 } else { 0 },
                metadata,
                model.scheduler_weight,
                u64_to_i64(model.success_count),
                u64_to_i64(model.failure_count),
                u64_to_i64(model.consecutive_failures),
                model.avg_latency_ms.map(u64_to_i64),
                model.cooldown_until.as_deref(),
                &model.last_error,
                model.last_used_at.as_deref(),
                model.last_checked_at.as_deref(),
                &model.created_at,
                &model.updated_at,
            ],
        )
        .map(|_| ())
        .map_err(|error| format!("无法新增配音模型备份数据: {error}"))
}

fn read_backup_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Uuid::new_v4().to_string()
    } else {
        trimmed.to_string()
    }
}

fn read_backup_string(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn read_optional_backup_timestamp(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn u64_to_i64(value: u64) -> i64 {
    value.min(i64::MAX as u64) as i64
}

fn read_backup_timestamp(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn settings_backup_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("请选择设置备份文件".to_string());
    }

    Ok(PathBuf::from(trimmed))
}

fn backup_setting_count() -> usize {
    29
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
