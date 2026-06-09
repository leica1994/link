use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::AppHandle;

use crate::app_paths;
use crate::ai::AiService;

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
            ",
        )
        .map_err(|error| format!("无法初始化设置数据库: {error}"))?;

    migrate_dubbing_models_table(connection)?;

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

fn migrate_dubbing_models_table(connection: &Connection) -> Result<(), String> {
    let mut statement = connection
        .prepare("PRAGMA index_list(dubbing_models)")
        .map_err(|error| format!("无法检查配音模型表结构: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(2)? != 0,
                row.get::<_, String>(3).unwrap_or_default(),
            ))
        })
        .map_err(|error| format!("无法检查配音模型表结构: {error}"))?;

    let mut has_model_unique_index = false;
    for row in rows {
        let (is_unique, origin) =
            row.map_err(|error| format!("无法解析配音模型表结构: {error}"))?;
        if is_unique && origin == "u" {
            has_model_unique_index = true;
            break;
        }
    }

    if !has_model_unique_index {
        return Ok(());
    }

    drop(statement);

    connection
        .execute_batch(
            "
            DROP TABLE IF EXISTS dubbing_models_migration;

            CREATE TABLE dubbing_models_migration (
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

            INSERT INTO dubbing_models_migration (
                id,
                engine,
                model_key,
                display_name,
                locale,
                gender,
                enabled,
                metadata,
                created_at,
                updated_at
            )
            SELECT
                id,
                engine,
                model_key,
                display_name,
                locale,
                gender,
                enabled,
                metadata,
                created_at,
                updated_at
            FROM dubbing_models;

            DROP TABLE dubbing_models;
            ALTER TABLE dubbing_models_migration RENAME TO dubbing_models;
            ",
        )
        .map_err(|error| format!("无法迁移配音模型表结构: {error}"))?;

    Ok(())
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
