use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    pub youtube_monitor_proxy: String,
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
            translation_format: read_string_setting(&setting_values, "translation_format", "srt"),
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
            youtube_monitor_proxy: read_string_setting(
                &setting_values,
                "youtube_monitor_proxy",
                "",
            ),
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

    fn save(&self, settings: &AppSettings) -> Result<(), String> {
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
            "youtube_monitor_proxy",
            &settings.youtube_monitor_proxy,
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
                upload_date TEXT,
                timestamp INTEGER,
                duration REAL,
                view_count INTEGER,
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

            CREATE INDEX IF NOT EXISTS idx_youtube_channels_updated_at
                ON youtube_channels(updated_at);
            CREATE INDEX IF NOT EXISTS idx_youtube_videos_channel_seen
                ON youtube_videos(channel_id, is_unread, first_seen_at);
            CREATE INDEX IF NOT EXISTS idx_youtube_videos_channel_timestamp
                ON youtube_videos(channel_id, timestamp, upload_date);
            CREATE INDEX IF NOT EXISTS idx_youtube_videos_channel_rank
                ON youtube_videos(channel_id, published_rank);
            CREATE INDEX IF NOT EXISTS idx_youtube_refresh_runs_channel_started
                ON youtube_refresh_runs(channel_id, started_at);
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
