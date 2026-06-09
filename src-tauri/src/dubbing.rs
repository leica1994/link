use base64::{engine::general_purpose, Engine as _};
use chrono::{FixedOffset, Utc};
use reqwest::blocking::multipart::{Form, Part};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use stem_splitter_core::{
    set_download_progress_callback, set_split_progress_callback, split_file, SplitOptions,
    SplitProgress,
};
use tauri::{AppHandle, Emitter, Manager};
use tungstenite::client::IntoClientRequest;
use tungstenite::{connect, Message};
use uuid::Uuid;

use crate::app_log::AppLogger;
use crate::app_paths;
use crate::settings::SettingsStore;
use crate::transcription::{serialize_subtitle, SubtitleFormat, TranscriptionSegment};

const EDGE_TTS_ENGINE: &str = "edge-tts";
const EDGE_TTS_ENGINE_LABEL: &str = "EDGE-TTS";
const EDGE_TTS_BASE_URL: &str = "speech.platform.bing.com/consumer/speech/synthesize/readaloud";
const EDGE_TTS_TRUSTED_CLIENT_TOKEN: &str = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";
const EDGE_TTS_SEC_MS_GEC_VERSION: &str = "1-143.0.3650.75";
const EDGE_TTS_CHROMIUM_MAJOR_VERSION: &str = "143";
const WINDOWS_EPOCH_SECONDS: u64 = 11_644_473_600;
const NANO_AI_TTS_ENGINE: &str = "nano-ai-tts";
const NANO_AI_TTS_ENGINE_LABEL: &str = "纳米AI TTS";
const NANO_AI_TTS_BASE_URL: &str = "https://bot.n.cn";
const NANO_AI_TTS_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/117.0.0.0 Safari/537.36";
const INDEX_TTS2_ENGINE: &str = "index-tts-2";
const INDEX_TTS2_ENGINE_LABEL: &str = "Index-TTS 2.0";
const INDEX_TTS2_ENDPOINT: &str = "http://127.0.0.1:7860";
const INDEX_TTS2_API_NAME: &str = "gen_single";
const INDEX_TTS2_SAMPLE_AUDIO: &[u8] = include_bytes!("../assets/audio_sample.mp3");
const DUBBING_PROGRESS_EVENT: &str = "dubbing-progress";
const DUBBING_STAGE_MATERIAL: &str = "material";
const DUBBING_STAGE_SUBTITLE_PREPROCESS: &str = "subtitle-preprocess";
const DUBBING_STAGE_MEDIA_SEPARATION: &str = "media-separation";
const DUBBING_STAGE_TTS_SYNTHESIS: &str = "tts-synthesis";
const DUBBING_STAGE_AUDIO_MERGE: &str = "audio-merge";
const DUBBING_STAGE_VIDEO_COMPOSE: &str = "video-compose";
const DUBBING_STATUS_READY: &str = "ready";
const DUBBING_STATUS_RUNNING: &str = "running";
const DUBBING_STATUS_PREPROCESSED: &str = "preprocessed";
const DUBBING_STATUS_FAILED: &str = "failed";
const DUBBING_STATUS_INTERRUPTED: &str = "interrupted";
const DUBBING_ARTIFACT_SOURCE_VIDEO: &str = "source-video";
const DUBBING_ARTIFACT_SOURCE_SUBTITLE: &str = "source-subtitle";
const DUBBING_ARTIFACT_PREPROCESSED_SUBTITLE: &str = "preprocessed-subtitle";
const DUBBING_ARTIFACT_MUTED_VIDEO: &str = "muted-video";
const DUBBING_ARTIFACT_SOURCE_AUDIO: &str = "source-audio";
const DUBBING_ARTIFACT_BACKGROUND_MUSIC: &str = "background-music";
const DUBBING_VIDEO_EXTENSIONS: &[&str] =
    &["mp4", "mov", "mkv", "avi", "flv", "wmv", "webm", "m4v"];
const DUBBING_SUBTITLE_EXTENSIONS: &[&str] = &[
    "srt", "vtt", "ass", "ssa", "lrc", "sbv", "smi", "sami", "ttml", "dfxp", "txt",
];
const MIN_DUBBING_SUBTITLE_DURATION_MS: u64 = 100;
const DEFAULT_DUBBING_TEXT_DURATION_MS: u64 = 3_000;
const BACKGROUND_MUSIC_MODEL: &str = "htdemucs_ort_v1";
const BACKGROUND_MUSIC_MODEL_MANIFEST_URL: &str =
    "https://hf-mirror.com/gentij/htdemucs-ort/resolve/main/manifest.json";
const BACKGROUND_MUSIC_MODEL_ARTIFACT_URL: &str =
    "https://hf-mirror.com/gentij/htdemucs-ort/resolve/main/htdemucs.ort";
const BACKGROUND_MUSIC_MODEL_CACHE_FILE: &str = "HTDemucs-ORT-09dc1655.ort";
const STEM_SPLITTER_CACHE_DIR_ENV: &str = "STEM_SPLITTER_CORE_CACHE_DIR";
const BACKGROUND_MUSIC_MODEL_SHA256: &str =
    "09dc165512d8ef7480bcb2cacea9dda82d571f8dbf421d8c44a2ca5568bec729";
const BACKGROUND_MUSIC_MODEL_SIZE_BYTES: u64 = 209_884_896;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingVoiceOption {
    pub engine: String,
    pub engine_label: String,
    pub model_key: String,
    pub display_name: String,
    pub locale: String,
    pub gender: String,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingModel {
    pub id: String,
    pub engine: String,
    pub engine_label: String,
    pub model_key: String,
    pub display_name: String,
    pub locale: String,
    pub gender: String,
    pub enabled: bool,
    pub metadata: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListDubbingVoicesRequest {
    pub engine: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddDubbingModelRequest {
    pub engine: String,
    pub model_key: String,
    pub endpoint: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetDubbingModelEnabledRequest {
    pub id: String,
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteDubbingModelRequest {
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewDubbingVoiceRequest {
    pub engine: String,
    pub model_key: String,
    pub locale: Option<String>,
    pub endpoint: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewDubbingVoiceResult {
    pub audio_data_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareDubbingMaterialRequest {
    pub video_path: String,
    pub subtitle_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDubbingTaskRequest {
    pub task_id: String,
    #[serde(default)]
    pub options: DubbingTaskOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingTaskOptions {
    pub tts_interval_ms: u32,
    pub reference_audio_source: String,
    pub is_background_music_enabled: bool,
    pub background_music_volume: f64,
}

impl Default for DubbingTaskOptions {
    fn default() -> Self {
        Self {
            tts_interval_ms: 150,
            reference_audio_source: "existing-dubbing".to_string(),
            is_background_music_enabled: true,
            background_music_volume: 0.5,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingProgressStage {
    pub progress: u8,
    pub message: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DubbingStageProgress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub material: Option<DubbingProgressStage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle_preprocess: Option<DubbingProgressStage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_separation: Option<DubbingProgressStage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tts_synthesis: Option<DubbingProgressStage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_merge: Option<DubbingProgressStage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_compose: Option<DubbingProgressStage>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingTaskArtifact {
    pub kind: String,
    pub path: String,
    pub metadata: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingTaskSnapshot {
    pub id: String,
    pub pair_key: String,
    pub video_path: String,
    pub subtitle_path: String,
    pub work_dir: String,
    pub current_stage: String,
    pub status: String,
    pub progress: u8,
    pub message: String,
    pub stages: DubbingStageProgress,
    pub artifacts: Vec<DubbingTaskArtifact>,
    pub segments: Vec<TranscriptionSegment>,
    pub warnings: Vec<String>,
    pub error_message: String,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
}

struct DubbingTaskRecord {
    id: String,
    pair_key: String,
    video_path: String,
    subtitle_path: String,
    work_dir: String,
    current_stage: String,
    status: String,
    progress: u8,
    message: String,
    warnings: Vec<String>,
    error_message: String,
    revision: u64,
    created_at: String,
    updated_at: String,
}

struct DubbingSubtitlePreprocessResult {
    segments: Vec<TranscriptionSegment>,
    output_path: PathBuf,
    warnings: Vec<String>,
}

struct DubbingMediaSeparationResult {
    muted_video_path: PathBuf,
    source_audio_path: PathBuf,
    background_music_path: Option<PathBuf>,
    warnings: Vec<String>,
}

struct StemSplitPaths {
    drums_path: PathBuf,
    bass_path: PathBuf,
    other_path: PathBuf,
}

struct DubbingSubtitleInput {
    segments: Vec<TranscriptionSegment>,
    warnings: Vec<String>,
}

struct AssMergedCue {
    start_time: u64,
    end_time: u64,
    primary: Vec<String>,
    secondary: Vec<String>,
    other: Vec<String>,
}

trait DubbingEngine {
    fn list_voices(&self) -> Result<Vec<DubbingVoiceOption>, String>;
    fn synthesize_preview(
        &self,
        model_key: &str,
        locale: Option<&str>,
        endpoint: Option<&str>,
    ) -> Result<Vec<u8>, String>;
}

struct EdgeTtsEngine;
struct NanoAiTtsEngine;
struct IndexTts2Engine;

struct IndexTts2Template {
    model_key: &'static str,
    display_name: &'static str,
    locale: &'static str,
    emo_control_method: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct EdgeTtsVoice {
    short_name: String,
    friendly_name: String,
    locale: String,
    gender: String,
    #[serde(default)]
    voice_tag: Value,
}

#[derive(Debug, Deserialize)]
struct NanoAiPlatformResponse {
    data: NanoAiPlatformData,
}

#[derive(Debug, Deserialize)]
struct NanoAiPlatformData {
    #[serde(default)]
    list: Vec<NanoAiRobot>,
}

#[derive(Debug, Deserialize)]
struct NanoAiRobot {
    #[serde(default)]
    tag: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    icon: String,
}

#[derive(Debug, Deserialize)]
struct GradioConfig {
    #[serde(default)]
    protocol: String,
    #[serde(default)]
    api_prefix: String,
    #[serde(default)]
    components: Vec<GradioComponent>,
    #[serde(default)]
    dependencies: Vec<GradioDependency>,
}

#[derive(Debug, Deserialize)]
struct GradioDependency {
    #[serde(default)]
    id: Option<usize>,
    #[serde(default)]
    api_name: Value,
    #[serde(default)]
    inputs: Vec<usize>,
}

#[derive(Debug, Deserialize)]
struct GradioComponent {
    id: usize,
    #[serde(rename = "type")]
    component_type: String,
    #[serde(default)]
    props: GradioComponentProps,
}

#[derive(Debug, Default, Deserialize)]
struct GradioComponentProps {
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    key: Value,
    #[serde(default)]
    value: Value,
    #[serde(default)]
    choices: Value,
}

#[derive(Debug, Deserialize)]
struct GradioQueueJoinResponse {
    event_id: String,
}

#[tauri::command]
pub fn list_dubbing_models(
    store: tauri::State<'_, SettingsStore>,
) -> Result<Vec<DubbingModel>, String> {
    read_dubbing_models(&store)
}

#[tauri::command]
pub fn list_dubbing_voices(
    request: ListDubbingVoicesRequest,
) -> Result<Vec<DubbingVoiceOption>, String> {
    engine_for(&request.engine)?.list_voices()
}

#[tauri::command]
pub fn prepare_dubbing_material(
    app: AppHandle,
    store: tauri::State<'_, SettingsStore>,
    request: PrepareDubbingMaterialRequest,
) -> Result<DubbingTaskSnapshot, String> {
    let video_path = canonical_material_path(&request.video_path, "视频文件不存在")?;
    let subtitle_path = canonical_material_path(&request.subtitle_path, "字幕文件不存在")?;
    ensure_supported_extension(&video_path, DUBBING_VIDEO_EXTENSIONS, "不支持的视频格式")?;
    ensure_supported_extension(
        &subtitle_path,
        DUBBING_SUBTITLE_EXTENSIONS,
        "不支持的字幕格式",
    )?;

    let pair_key = dubbing_pair_key(&video_path, &subtitle_path);
    let work_dir = dubbing_work_dir(&app, &pair_key)?;
    let source_dir = work_dir.join("source");
    fs::create_dir_all(&source_dir).map_err(|error| format!("无法创建配音素材目录: {error}"))?;

    let source_video_path = source_dir.join(format!(
        "video.{}",
        path_extension(&video_path).unwrap_or_else(|| "mp4".to_string())
    ));
    let source_subtitle_path = source_dir.join(format!(
        "subtitle.{}",
        path_extension(&subtitle_path).unwrap_or_else(|| "srt".to_string())
    ));
    link_or_copy_if_stale(&video_path, &source_video_path)?;
    link_or_copy_if_stale(&subtitle_path, &source_subtitle_path)?;

    let now = Utc::now().to_rfc3339();
    store.with_connection(|connection| {
        let existing = read_dubbing_task_record_by_pair_key(connection, &pair_key)?;
        let should_mark_interrupted = existing
            .as_ref()
            .is_some_and(|task| task.status == DUBBING_STATUS_RUNNING);
        let interrupted_stage = existing
            .as_ref()
            .map(|task| task.current_stage.clone())
            .unwrap_or_else(|| DUBBING_STAGE_MATERIAL.to_string());
        let task_id = existing
            .as_ref()
            .map(|task| task.id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let status = existing
            .as_ref()
            .map(|task| {
                if task.status == DUBBING_STATUS_RUNNING {
                    DUBBING_STATUS_INTERRUPTED.to_string()
                } else {
                    task.status.clone()
                }
            })
            .unwrap_or_else(|| DUBBING_STATUS_READY.to_string());
        let current_stage = existing
            .as_ref()
            .map(|task| task.current_stage.clone())
            .unwrap_or_else(|| DUBBING_STAGE_MATERIAL.to_string());
        let progress = existing.as_ref().map(|task| task.progress).unwrap_or(100);
        let message = existing
            .as_ref()
            .map(|task| {
                if task.status == DUBBING_STATUS_RUNNING {
                    "任务已中断，可继续配音".to_string()
                } else {
                    task.message.clone()
                }
            })
            .filter(|message| !message.trim().is_empty())
            .unwrap_or_else(|| "素材准备完成".to_string());
        let error_message = existing
            .as_ref()
            .filter(|task| task.status != DUBBING_STATUS_RUNNING)
            .map(|task| task.error_message.clone())
            .unwrap_or_default();
        let warnings = existing
            .as_ref()
            .map(|task| serde_json::to_string(&task.warnings).unwrap_or_else(|_| "[]".to_string()))
            .unwrap_or_else(|| "[]".to_string());
        let revision = existing.as_ref().map(|task| task.revision).unwrap_or(0) + 1;

        if existing.is_some() {
            connection
                .execute(
                    "
                    UPDATE dubbing_tasks
                    SET video_path = ?2,
                        subtitle_path = ?3,
                        work_dir = ?4,
                        current_stage = ?5,
                        status = ?6,
                        message = ?7,
                        progress = ?8,
                        warnings = ?9,
                        error_message = ?10,
                        revision = ?11,
                        updated_at = ?12
                    WHERE id = ?1
                    ",
                    params![
                        task_id,
                        path_to_string(&video_path),
                        path_to_string(&subtitle_path),
                        path_to_string(&work_dir),
                        current_stage,
                        status,
                        message,
                        progress,
                        warnings,
                        error_message,
                        revision,
                        now,
                    ],
                )
                .map_err(|error| format!("无法更新配音任务: {error}"))?;
        } else {
            connection
                .execute(
                    "
                    INSERT INTO dubbing_tasks (
                        id,
                        pair_key,
                        video_path,
                        subtitle_path,
                        work_dir,
                        current_stage,
                        status,
                        message,
                        progress,
                        options,
                        warnings,
                        error_message,
                        revision,
                        created_at,
                        updated_at
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '{}', ?10, '', ?11, ?12, ?12)
                    ",
                    params![
                        task_id,
                        pair_key,
                        path_to_string(&video_path),
                        path_to_string(&subtitle_path),
                        path_to_string(&work_dir),
                        DUBBING_STAGE_MATERIAL,
                        DUBBING_STATUS_READY,
                        "素材准备完成",
                        100,
                        warnings,
                        revision,
                        now,
                    ],
                )
                .map_err(|error| format!("无法创建配音任务: {error}"))?;
        }

        upsert_dubbing_stage(
            connection,
            &task_id,
            DUBBING_STAGE_MATERIAL,
            100,
            "素材准备完成",
            "done",
            json!({}),
            &now,
        )?;
        insert_dubbing_stage_if_missing(
            connection,
            &task_id,
            DUBBING_STAGE_SUBTITLE_PREPROCESS,
            0,
            "等待开始字幕预处理",
            "pending",
            &now,
        )?;
        insert_dubbing_stage_if_missing(
            connection,
            &task_id,
            DUBBING_STAGE_MEDIA_SEPARATION,
            0,
            "等待音视频分离",
            "pending",
            &now,
        )?;
        if should_mark_interrupted && interrupted_stage != DUBBING_STAGE_MATERIAL {
            upsert_dubbing_stage(
                connection,
                &task_id,
                &interrupted_stage,
                progress,
                "任务已中断，可继续配音",
                "interrupted",
                json!({}),
                &now,
            )?;
        }
        upsert_dubbing_artifact(
            connection,
            &task_id,
            DUBBING_ARTIFACT_SOURCE_VIDEO,
            &source_video_path,
            material_metadata(&video_path)?,
            &now,
        )?;
        upsert_dubbing_artifact(
            connection,
            &task_id,
            DUBBING_ARTIFACT_SOURCE_SUBTITLE,
            &source_subtitle_path,
            material_metadata(&subtitle_path)?,
            &now,
        )?;

        read_dubbing_task_snapshot_by_id(connection, &task_id)
    })
}

#[tauri::command]
pub async fn start_dubbing_task(
    app: AppHandle,
    request: StartDubbingTaskRequest,
) -> Result<DubbingTaskSnapshot, String> {
    match tauri::async_runtime::spawn_blocking(move || start_dubbing_task_blocking(app, request))
        .await
    {
        Ok(result) => result,
        Err(error) => Err(format!("配音任务执行失败: {error}")),
    }
}

fn start_dubbing_task_blocking(
    app: AppHandle,
    request: StartDubbingTaskRequest,
) -> Result<DubbingTaskSnapshot, String> {
    let store = app.state::<SettingsStore>();
    let app_logger = app.state::<AppLogger>();
    let log_session = app_logger.start_session("dubbing")?;
    log_session.info(
        "request_received",
        "收到配音任务请求",
        json!({
            "taskId": &request.task_id,
            "backgroundMusic": request.options.is_background_music_enabled,
        }),
    );
    let options = request.options;
    save_dubbing_task_options(&store, &request.task_id, &options)?;

    let mut snapshot = store.with_connection(|connection| {
        read_dubbing_task_snapshot_by_id(connection, &request.task_id)
    })?;

    if is_media_separation_done(&snapshot, &options) {
        log_session.info(
            "media_separation_reused",
            "复用已完成的音视频分离结果",
            json!({ "taskId": &request.task_id }),
        );
        return Ok(snapshot);
    }

    if is_subtitle_preprocess_done(&snapshot) {
        log_session.info(
            "subtitle_preprocess_reused",
            "复用已完成的字幕预处理结果",
            json!({ "taskId": &request.task_id }),
        );
    } else {
        let now = Utc::now().to_rfc3339();
        snapshot = store.with_connection(|connection| {
            update_dubbing_task_state(
                connection,
                &request.task_id,
                DUBBING_STAGE_SUBTITLE_PREPROCESS,
                DUBBING_STATUS_RUNNING,
                0,
                "字幕预处理中",
                "",
                None,
                &now,
            )?;
            upsert_dubbing_stage(
                connection,
                &request.task_id,
                DUBBING_STAGE_SUBTITLE_PREPROCESS,
                0,
                "字幕预处理中",
                "active",
                json!({}),
                &now,
            )?;
            read_dubbing_task_snapshot_by_id(connection, &request.task_id)
        })?;
        emit_dubbing_progress(&app, &snapshot);

        let result = run_subtitle_preprocess(&snapshot);
        match result {
            Ok(preprocess_result) => {
                let now = Utc::now().to_rfc3339();
                snapshot = store.with_connection(|connection| {
                    upsert_dubbing_artifact(
                        connection,
                        &request.task_id,
                        DUBBING_ARTIFACT_PREPROCESSED_SUBTITLE,
                        &preprocess_result.output_path,
                        json!({
                            "format": "srt",
                            "segmentCount": preprocess_result.segments.len(),
                            "warnings": &preprocess_result.warnings,
                        }),
                        &now,
                    )?;
                    update_dubbing_task_state(
                        connection,
                        &request.task_id,
                        DUBBING_STAGE_SUBTITLE_PREPROCESS,
                        DUBBING_STATUS_RUNNING,
                        100,
                        "字幕预处理完成",
                        "",
                        Some(&preprocess_result.warnings),
                        &now,
                    )?;
                    upsert_dubbing_stage(
                        connection,
                        &request.task_id,
                        DUBBING_STAGE_SUBTITLE_PREPROCESS,
                        100,
                        "字幕预处理完成",
                        "done",
                        json!({
                            "segmentCount": preprocess_result.segments.len(),
                            "outputPath": path_to_string(&preprocess_result.output_path),
                            "warnings": &preprocess_result.warnings,
                        }),
                        &now,
                    )?;
                    read_dubbing_task_snapshot_by_id(connection, &request.task_id)
                })?;
                emit_dubbing_progress(&app, &snapshot);
                log_session.info(
                    "subtitle_preprocess_completed",
                    "字幕预处理完成",
                    json!({
                        "taskId": &request.task_id,
                        "segmentCount": snapshot.segments.len(),
                    }),
                );
            }
            Err(error) => {
                let now = Utc::now().to_rfc3339();
                let failed_snapshot = store.with_connection(|connection| {
                    update_dubbing_task_state(
                        connection,
                        &request.task_id,
                        DUBBING_STAGE_SUBTITLE_PREPROCESS,
                        DUBBING_STATUS_FAILED,
                        0,
                        "字幕预处理失败",
                        &error,
                        None,
                        &now,
                    )?;
                    upsert_dubbing_stage(
                        connection,
                        &request.task_id,
                        DUBBING_STAGE_SUBTITLE_PREPROCESS,
                        0,
                        "字幕预处理失败",
                        "failed",
                        json!({ "error": &error }),
                        &now,
                    )?;
                    read_dubbing_task_snapshot_by_id(connection, &request.task_id)
                })?;
                emit_dubbing_progress(&app, &failed_snapshot);
                log_session.error(
                    "subtitle_preprocess_failed",
                    "字幕预处理失败",
                    json!({
                        "taskId": &request.task_id,
                        "error": &error,
                    }),
                );
                return Err(error);
            }
        }
    }

    let now = Utc::now().to_rfc3339();
    snapshot = store.with_connection(|connection| {
        update_dubbing_task_state(
            connection,
            &request.task_id,
            DUBBING_STAGE_MEDIA_SEPARATION,
            DUBBING_STATUS_RUNNING,
            0,
            "音视频分离中",
            "",
            None,
            &now,
        )?;
        upsert_dubbing_stage(
            connection,
            &request.task_id,
            DUBBING_STAGE_MEDIA_SEPARATION,
            0,
            "音视频分离中",
            "active",
            json!({ "backgroundMusic": options.is_background_music_enabled }),
            &now,
        )?;
        read_dubbing_task_snapshot_by_id(connection, &request.task_id)
    })?;
    emit_dubbing_progress(&app, &snapshot);

    let media_input_snapshot = snapshot.clone();
    let media_progress =
        MediaSeparationProgress::new(app.clone(), request.task_id.clone(), options.clone());
    let media_result = run_media_separation(&media_input_snapshot, &options, &media_progress);

    match media_result {
        Ok(media_result) => {
            snapshot = media_progress.snapshot().unwrap_or(snapshot);
            let now = Utc::now().to_rfc3339();
            let warnings = deduplicate_warnings(
                snapshot
                    .warnings
                    .iter()
                    .cloned()
                    .chain(media_result.warnings.clone())
                    .collect(),
            );
            snapshot = store.with_connection(|connection| {
                upsert_dubbing_artifact(
                    connection,
                    &request.task_id,
                    DUBBING_ARTIFACT_MUTED_VIDEO,
                    &media_result.muted_video_path,
                    json!({
                        "source": DUBBING_ARTIFACT_SOURCE_VIDEO,
                        "videoOnly": true,
                    }),
                    &now,
                )?;
                upsert_dubbing_artifact(
                    connection,
                    &request.task_id,
                    DUBBING_ARTIFACT_SOURCE_AUDIO,
                    &media_result.source_audio_path,
                    json!({
                        "format": "wav",
                        "sampleRate": 44100,
                    }),
                    &now,
                )?;
                if let Some(background_music_path) = &media_result.background_music_path {
                    upsert_dubbing_artifact(
                        connection,
                        &request.task_id,
                        DUBBING_ARTIFACT_BACKGROUND_MUSIC,
                        background_music_path,
                        json!({
                            "format": "wav",
                            "method": "stem-splitter-core",
                            "model": BACKGROUND_MUSIC_MODEL,
                            "stems": ["drums", "bass", "other"],
                            "volume": options.background_music_volume,
                        }),
                        &now,
                    )?;
                } else {
                    delete_dubbing_artifact(
                        connection,
                        &request.task_id,
                        DUBBING_ARTIFACT_BACKGROUND_MUSIC,
                    )?;
                }
                update_dubbing_task_state(
                    connection,
                    &request.task_id,
                    DUBBING_STAGE_MEDIA_SEPARATION,
                    DUBBING_STATUS_PREPROCESSED,
                    100,
                    "音视频分离完成",
                    "",
                    Some(&warnings),
                    &now,
                )?;
                upsert_dubbing_stage(
                    connection,
                    &request.task_id,
                    DUBBING_STAGE_MEDIA_SEPARATION,
                    100,
                    "音视频分离完成",
                    "done",
                    json!({
                        "mutedVideoPath": path_to_string(&media_result.muted_video_path),
                        "sourceAudioPath": path_to_string(&media_result.source_audio_path),
                        "backgroundMusicPath": media_result.background_music_path.as_ref().map(|path| path_to_string(path)),
                        "backgroundMusic": options.is_background_music_enabled,
                        "warnings": &media_result.warnings,
                    }),
                    &now,
                )?;
                read_dubbing_task_snapshot_by_id(connection, &request.task_id)
            })?;
            emit_dubbing_progress(&app, &snapshot);
            log_session.info(
                "media_separation_completed",
                "音视频分离完成",
                json!({
                    "taskId": &request.task_id,
                    "backgroundMusic": options.is_background_music_enabled,
                }),
            );
            Ok(snapshot)
        }
        Err(error) => {
            let now = Utc::now().to_rfc3339();
            let failed_snapshot = store.with_connection(|connection| {
                update_dubbing_task_state(
                    connection,
                    &request.task_id,
                    DUBBING_STAGE_MEDIA_SEPARATION,
                    DUBBING_STATUS_FAILED,
                    0,
                    "音视频分离失败",
                    &error,
                    None,
                    &now,
                )?;
                upsert_dubbing_stage(
                    connection,
                    &request.task_id,
                    DUBBING_STAGE_MEDIA_SEPARATION,
                    0,
                    "音视频分离失败",
                    "failed",
                    json!({ "error": &error }),
                    &now,
                )?;
                read_dubbing_task_snapshot_by_id(connection, &request.task_id)
            })?;
            emit_dubbing_progress(&app, &failed_snapshot);
            log_session.error(
                "media_separation_failed",
                "音视频分离失败",
                json!({
                    "taskId": &request.task_id,
                    "error": &error,
                }),
            );
            Err(error)
        }
    }
}

#[tauri::command]
pub fn add_dubbing_model(
    store: tauri::State<'_, SettingsStore>,
    request: AddDubbingModelRequest,
) -> Result<DubbingModel, String> {
    let mut voice = engine_for(&request.engine)?
        .list_voices()?
        .into_iter()
        .find(|voice| voice.model_key == request.model_key)
        .ok_or_else(|| "未找到该语音".to_string())?;
    apply_dubbing_model_options(&mut voice, request.endpoint.as_deref())?;

    insert_dubbing_model(&store, voice)
}

#[tauri::command]
pub fn set_dubbing_model_enabled(
    store: tauri::State<'_, SettingsStore>,
    request: SetDubbingModelEnabledRequest,
) -> Result<DubbingModel, String> {
    let updated_at = Utc::now().to_rfc3339();

    store.with_connection(|connection| {
        let changed = connection
            .execute(
                "
                UPDATE dubbing_models
                SET enabled = ?2, updated_at = ?3
                WHERE id = ?1
                ",
                params![request.id, if request.enabled { 1 } else { 0 }, updated_at],
            )
            .map_err(|error| format!("无法更新配音模型: {error}"))?;

        if changed == 0 {
            return Err("未找到该配音模型".to_string());
        }

        read_dubbing_model_by_id(connection, &request.id)
    })
}

#[tauri::command]
pub fn delete_dubbing_model(
    store: tauri::State<'_, SettingsStore>,
    request: DeleteDubbingModelRequest,
) -> Result<(), String> {
    store.with_connection(|connection| {
        let changed = connection
            .execute(
                "DELETE FROM dubbing_models WHERE id = ?1",
                params![request.id],
            )
            .map_err(|error| format!("无法删除配音模型: {error}"))?;

        if changed == 0 {
            return Err("未找到该配音模型".to_string());
        }

        Ok(())
    })
}

#[tauri::command]
pub fn preview_dubbing_voice(
    request: PreviewDubbingVoiceRequest,
) -> Result<PreviewDubbingVoiceResult, String> {
    let audio = engine_for(&request.engine)?.synthesize_preview(
        &request.model_key,
        request.locale.as_deref(),
        request.endpoint.as_deref(),
    )?;
    let mime_type = audio_mime_type(&audio);
    let audio_data_url = format!(
        "data:{mime_type};base64,{}",
        general_purpose::STANDARD.encode(audio)
    );

    Ok(PreviewDubbingVoiceResult { audio_data_url })
}

fn engine_for(engine: &str) -> Result<Box<dyn DubbingEngine>, String> {
    match engine {
        EDGE_TTS_ENGINE => Ok(Box::new(EdgeTtsEngine)),
        NANO_AI_TTS_ENGINE => Ok(Box::new(NanoAiTtsEngine)),
        INDEX_TTS2_ENGINE => Ok(Box::new(IndexTts2Engine)),
        _ => Err("不支持的配音引擎".to_string()),
    }
}

fn read_dubbing_models(store: &SettingsStore) -> Result<Vec<DubbingModel>, String> {
    store.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "
                SELECT id, engine, model_key, display_name, locale, gender, enabled, metadata, created_at, updated_at
                FROM dubbing_models
                ORDER BY created_at DESC
                ",
            )
            .map_err(|error| format!("无法读取配音模型: {error}"))?;

        let rows = statement
            .query_map([], map_dubbing_model)
            .map_err(|error| format!("无法读取配音模型: {error}"))?;

        let mut models = Vec::new();
        for row in rows {
            models.push(row.map_err(|error| format!("无法解析配音模型: {error}"))?);
        }

        Ok(models)
    })
}

fn audio_mime_type(audio: &[u8]) -> &'static str {
    if audio.starts_with(b"RIFF") && audio.get(8..12) == Some(b"WAVE") {
        return "audio/wav";
    }

    if audio.starts_with(b"ID3")
        || audio
            .first()
            .zip(audio.get(1))
            .is_some_and(|(first, second)| *first == 0xFF && (*second & 0xE0) == 0xE0)
    {
        return "audio/mpeg";
    }

    if audio.starts_with(b"OggS") {
        return "audio/ogg";
    }

    if audio.starts_with(b"fLaC") {
        return "audio/flac";
    }

    "application/octet-stream"
}

fn apply_dubbing_model_options(
    voice: &mut DubbingVoiceOption,
    endpoint: Option<&str>,
) -> Result<(), String> {
    if voice.engine == INDEX_TTS2_ENGINE {
        let endpoint = normalize_index_tts2_endpoint(endpoint)?;
        voice.metadata = index_tts2_metadata(&endpoint);
    }

    Ok(())
}

fn read_dubbing_model_by_id(
    connection: &rusqlite::Connection,
    id: &str,
) -> Result<DubbingModel, String> {
    connection
        .query_row(
            "
            SELECT id, engine, model_key, display_name, locale, gender, enabled, metadata, created_at, updated_at
            FROM dubbing_models
            WHERE id = ?1
            ",
            params![id],
            map_dubbing_model,
        )
        .optional()
        .map_err(|error| format!("无法读取配音模型: {error}"))?
        .ok_or_else(|| "未找到该配音模型".to_string())
}

fn insert_dubbing_model(
    store: &SettingsStore,
    voice: DubbingVoiceOption,
) -> Result<DubbingModel, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let metadata = serde_json::to_string(&voice.metadata)
        .map_err(|error| format!("无法序列化配音模型: {error}"))?;

    store.with_connection(|connection| {
        connection
            .execute(
                "
                INSERT INTO dubbing_models (
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
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?9)
                ",
                params![
                    id,
                    voice.engine,
                    voice.model_key,
                    voice.display_name,
                    voice.locale,
                    voice.gender,
                    metadata,
                    now,
                    now,
                ],
            )
            .map_err(|error| {
                if error.to_string().contains("UNIQUE") {
                    "该语音模型已添加".to_string()
                } else {
                    format!("无法添加配音模型: {error}")
                }
            })?;

        read_dubbing_model_by_id(connection, &id)
    })
}

fn map_dubbing_model(row: &Row<'_>) -> rusqlite::Result<DubbingModel> {
    let engine: String = row.get(1)?;
    let metadata_text: String = row.get(7)?;
    let metadata = serde_json::from_str(&metadata_text).unwrap_or_else(|_| json!({}));

    Ok(DubbingModel {
        id: row.get(0)?,
        engine: engine.clone(),
        engine_label: engine_label(&engine).to_string(),
        model_key: row.get(2)?,
        display_name: row.get(3)?,
        locale: row.get(4)?,
        gender: row.get(5)?,
        enabled: row.get::<_, i64>(6)? != 0,
        metadata,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn canonical_material_path(path: &str, missing_message: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path);
    if !path.is_file() {
        return Err(missing_message.to_string());
    }

    fs::canonicalize(&path).map_err(|error| format!("无法读取素材路径: {error}"))
}

fn ensure_supported_extension(
    path: &Path,
    supported_extensions: &[&str],
    message: &str,
) -> Result<(), String> {
    let extension = path_extension(path).ok_or_else(|| message.to_string())?;
    if supported_extensions.iter().any(|value| *value == extension) {
        Ok(())
    } else {
        Err(message.to_string())
    }
}

fn path_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
}

fn dubbing_pair_key(video_path: &Path, subtitle_path: &Path) -> String {
    let raw = format!(
        "{}\0{}",
        normalized_pair_path(video_path),
        normalized_pair_path(subtitle_path)
    );
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn normalized_pair_path(path: &Path) -> String {
    let value = path_to_string(path);
    if cfg!(windows) {
        value.to_ascii_lowercase()
    } else {
        value
    }
}

fn dubbing_work_dir(_app: &AppHandle, pair_key: &str) -> Result<PathBuf, String> {
    Ok(app_paths::dubbing_dir()?.join(pair_key))
}

fn link_or_copy_if_stale(source: &Path, destination: &Path) -> Result<(), String> {
    if is_same_size_file(source, destination)? {
        return Ok(());
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("无法创建素材目录: {error}"))?;
    }

    if destination.exists() {
        fs::remove_file(destination).map_err(|error| format!("无法更新素材缓存: {error}"))?;
    }

    match fs::hard_link(source, destination) {
        Ok(_) => Ok(()),
        Err(_) => fs::copy(source, destination)
            .map(|_| ())
            .map_err(|error| format!("无法复制素材到配音工作目录: {error}")),
    }
}

fn is_same_size_file(source: &Path, destination: &Path) -> Result<bool, String> {
    if !destination.is_file() {
        return Ok(false);
    }

    let source_metadata =
        fs::metadata(source).map_err(|error| format!("无法读取素材信息: {error}"))?;
    let destination_metadata =
        fs::metadata(destination).map_err(|error| format!("无法读取素材缓存信息: {error}"))?;

    Ok(source_metadata.len() == destination_metadata.len())
}

fn material_metadata(path: &Path) -> Result<Value, String> {
    let metadata = fs::metadata(path).map_err(|error| format!("无法读取素材信息: {error}"))?;
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or_default();

    Ok(json!({
        "size": metadata.len(),
        "modifiedMs": modified_ms,
    }))
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn read_dubbing_task_record_by_pair_key(
    connection: &rusqlite::Connection,
    pair_key: &str,
) -> Result<Option<DubbingTaskRecord>, String> {
    connection
        .query_row(
            "
            SELECT id, pair_key, video_path, subtitle_path, work_dir, current_stage, status,
                   progress, message, warnings, error_message, revision, created_at, updated_at
            FROM dubbing_tasks
            WHERE pair_key = ?1
            ",
            params![pair_key],
            map_dubbing_task_record,
        )
        .optional()
        .map_err(|error| format!("无法读取配音任务: {error}"))
}

fn read_dubbing_task_record_by_id(
    connection: &rusqlite::Connection,
    task_id: &str,
) -> Result<DubbingTaskRecord, String> {
    connection
        .query_row(
            "
            SELECT id, pair_key, video_path, subtitle_path, work_dir, current_stage, status,
                   progress, message, warnings, error_message, revision, created_at, updated_at
            FROM dubbing_tasks
            WHERE id = ?1
            ",
            params![task_id],
            map_dubbing_task_record,
        )
        .optional()
        .map_err(|error| format!("无法读取配音任务: {error}"))?
        .ok_or_else(|| "未找到配音任务".to_string())
}

fn map_dubbing_task_record(row: &Row<'_>) -> rusqlite::Result<DubbingTaskRecord> {
    let warnings_text: String = row.get(9)?;
    let warnings = serde_json::from_str::<Vec<String>>(&warnings_text).unwrap_or_default();
    let progress = row.get::<_, i64>(7)?.clamp(0, 100) as u8;
    let revision = row.get::<_, i64>(11)?.max(0) as u64;

    Ok(DubbingTaskRecord {
        id: row.get(0)?,
        pair_key: row.get(1)?,
        video_path: row.get(2)?,
        subtitle_path: row.get(3)?,
        work_dir: row.get(4)?,
        current_stage: row.get(5)?,
        status: row.get(6)?,
        progress,
        message: row.get(8)?,
        warnings,
        error_message: row.get(10)?,
        revision,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

fn read_dubbing_task_snapshot_by_id(
    connection: &rusqlite::Connection,
    task_id: &str,
) -> Result<DubbingTaskSnapshot, String> {
    let record = read_dubbing_task_record_by_id(connection, task_id)?;
    let artifacts = read_dubbing_artifacts(connection, task_id)?;
    let segments = read_preprocessed_segments(&artifacts);

    Ok(DubbingTaskSnapshot {
        id: record.id,
        pair_key: record.pair_key,
        video_path: record.video_path,
        subtitle_path: record.subtitle_path,
        work_dir: record.work_dir,
        current_stage: record.current_stage,
        status: record.status,
        progress: record.progress,
        message: record.message,
        stages: read_dubbing_stages(connection, task_id)?,
        artifacts,
        segments,
        warnings: record.warnings,
        error_message: record.error_message,
        revision: record.revision,
        created_at: record.created_at,
        updated_at: record.updated_at,
    })
}

fn read_dubbing_stages(
    connection: &rusqlite::Connection,
    task_id: &str,
) -> Result<DubbingStageProgress, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT stage_key, progress, message, status
            FROM dubbing_task_stages
            WHERE task_id = ?1
            ",
        )
        .map_err(|error| format!("无法读取配音阶段: {error}"))?;
    let rows = statement
        .query_map(params![task_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                DubbingProgressStage {
                    progress: row.get::<_, i64>(1)?.clamp(0, 100) as u8,
                    message: row.get(2)?,
                    status: row.get(3)?,
                },
            ))
        })
        .map_err(|error| format!("无法读取配音阶段: {error}"))?;

    let mut stages = DubbingStageProgress::default();
    for row in rows {
        let (stage_key, stage) = row.map_err(|error| format!("无法解析配音阶段: {error}"))?;
        set_dubbing_stage(&mut stages, &stage_key, stage);
    }

    Ok(stages)
}

fn set_dubbing_stage(
    stages: &mut DubbingStageProgress,
    stage_key: &str,
    stage: DubbingProgressStage,
) {
    match stage_key {
        DUBBING_STAGE_MATERIAL => stages.material = Some(stage),
        DUBBING_STAGE_SUBTITLE_PREPROCESS => stages.subtitle_preprocess = Some(stage),
        DUBBING_STAGE_MEDIA_SEPARATION => stages.media_separation = Some(stage),
        DUBBING_STAGE_TTS_SYNTHESIS => stages.tts_synthesis = Some(stage),
        DUBBING_STAGE_AUDIO_MERGE => stages.audio_merge = Some(stage),
        DUBBING_STAGE_VIDEO_COMPOSE => stages.video_compose = Some(stage),
        _ => {}
    }
}

fn read_dubbing_artifacts(
    connection: &rusqlite::Connection,
    task_id: &str,
) -> Result<Vec<DubbingTaskArtifact>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT kind, path, metadata, created_at, updated_at
            FROM dubbing_task_artifacts
            WHERE task_id = ?1
            ORDER BY created_at ASC
            ",
        )
        .map_err(|error| format!("无法读取配音中间文件: {error}"))?;
    let rows = statement
        .query_map(params![task_id], |row| {
            let metadata_text: String = row.get(2)?;
            Ok(DubbingTaskArtifact {
                kind: row.get(0)?,
                path: row.get(1)?,
                metadata: serde_json::from_str(&metadata_text).unwrap_or_else(|_| json!({})),
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })
        .map_err(|error| format!("无法读取配音中间文件: {error}"))?;

    let mut artifacts = Vec::new();
    for row in rows {
        artifacts.push(row.map_err(|error| format!("无法解析配音中间文件: {error}"))?);
    }

    Ok(artifacts)
}

fn upsert_dubbing_stage(
    connection: &rusqlite::Connection,
    task_id: &str,
    stage_key: &str,
    progress: u8,
    message: &str,
    status: &str,
    snapshot: Value,
    updated_at: &str,
) -> Result<(), String> {
    let snapshot =
        serde_json::to_string(&snapshot).map_err(|error| format!("无法序列化阶段快照: {error}"))?;
    connection
        .execute(
            "
            INSERT INTO dubbing_task_stages (task_id, stage_key, progress, message, status, snapshot, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(task_id, stage_key) DO UPDATE SET
                progress = excluded.progress,
                message = excluded.message,
                status = excluded.status,
                snapshot = excluded.snapshot,
                updated_at = excluded.updated_at
            ",
            params![task_id, stage_key, progress, message, status, snapshot, updated_at],
        )
        .map_err(|error| format!("无法保存配音阶段: {error}"))?;

    Ok(())
}

fn insert_dubbing_stage_if_missing(
    connection: &rusqlite::Connection,
    task_id: &str,
    stage_key: &str,
    progress: u8,
    message: &str,
    status: &str,
    updated_at: &str,
) -> Result<(), String> {
    connection
        .execute(
            "
            INSERT OR IGNORE INTO dubbing_task_stages (task_id, stage_key, progress, message, status, snapshot, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, '{}', ?6)
            ",
            params![task_id, stage_key, progress, message, status, updated_at],
        )
        .map_err(|error| format!("无法初始化配音阶段: {error}"))?;

    Ok(())
}

fn upsert_dubbing_artifact(
    connection: &rusqlite::Connection,
    task_id: &str,
    kind: &str,
    path: &Path,
    metadata: Value,
    updated_at: &str,
) -> Result<(), String> {
    let metadata = serde_json::to_string(&metadata)
        .map_err(|error| format!("无法序列化中间文件信息: {error}"))?;
    connection
        .execute(
            "
            INSERT INTO dubbing_task_artifacts (id, task_id, kind, path, metadata, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
            ON CONFLICT(task_id, kind) DO UPDATE SET
                path = excluded.path,
                metadata = excluded.metadata,
                updated_at = excluded.updated_at
            ",
            params![
                Uuid::new_v4().to_string(),
                task_id,
                kind,
                path_to_string(path),
                metadata,
                updated_at,
            ],
        )
        .map_err(|error| format!("无法保存配音中间文件: {error}"))?;

    Ok(())
}

fn delete_dubbing_artifact(
    connection: &rusqlite::Connection,
    task_id: &str,
    kind: &str,
) -> Result<(), String> {
    connection
        .execute(
            "DELETE FROM dubbing_task_artifacts WHERE task_id = ?1 AND kind = ?2",
            params![task_id, kind],
        )
        .map_err(|error| format!("无法删除配音中间文件记录: {error}"))?;

    Ok(())
}

fn save_dubbing_task_options(
    store: &SettingsStore,
    task_id: &str,
    options: &DubbingTaskOptions,
) -> Result<(), String> {
    let options_text =
        serde_json::to_string(options).map_err(|error| format!("无法序列化配音参数: {error}"))?;
    let now = Utc::now().to_rfc3339();
    store.with_connection(|connection| {
        let changed = connection
            .execute(
                "
                UPDATE dubbing_tasks
                SET options = ?2, updated_at = ?3
                WHERE id = ?1
                ",
                params![task_id, options_text, now],
            )
            .map_err(|error| format!("无法保存配音参数: {error}"))?;
        if changed == 0 {
            Err("未找到配音任务".to_string())
        } else {
            Ok(())
        }
    })
}

fn is_subtitle_preprocess_done(snapshot: &DubbingTaskSnapshot) -> bool {
    let stage_done = snapshot
        .stages
        .subtitle_preprocess
        .as_ref()
        .is_some_and(|stage| stage.status == "done");
    let artifact_exists = snapshot.artifacts.iter().any(|artifact| {
        artifact.kind == DUBBING_ARTIFACT_PREPROCESSED_SUBTITLE
            && Path::new(&artifact.path).is_file()
    });

    stage_done && artifact_exists
}

fn is_media_separation_done(snapshot: &DubbingTaskSnapshot, options: &DubbingTaskOptions) -> bool {
    let stage_done = snapshot
        .stages
        .media_separation
        .as_ref()
        .is_some_and(|stage| stage.status == "done");
    let required_artifacts = [DUBBING_ARTIFACT_MUTED_VIDEO, DUBBING_ARTIFACT_SOURCE_AUDIO]
        .into_iter()
        .all(|kind| dubbing_artifact_file_exists(snapshot, kind));
    let background_ready =
        !options.is_background_music_enabled || background_music_artifact_file_exists(snapshot);

    stage_done && required_artifacts && background_ready
}

fn background_music_artifact_file_exists(snapshot: &DubbingTaskSnapshot) -> bool {
    snapshot.artifacts.iter().any(|artifact| {
        artifact.kind == DUBBING_ARTIFACT_BACKGROUND_MUSIC
            && Path::new(&artifact.path).is_file()
            && artifact.metadata.get("method").and_then(Value::as_str) == Some("stem-splitter-core")
            && artifact.metadata.get("model").and_then(Value::as_str)
                == Some(BACKGROUND_MUSIC_MODEL)
    })
}

fn dubbing_artifact_file_exists(snapshot: &DubbingTaskSnapshot, kind: &str) -> bool {
    snapshot
        .artifacts
        .iter()
        .any(|artifact| artifact.kind == kind && Path::new(&artifact.path).is_file())
}

fn update_dubbing_task_state(
    connection: &rusqlite::Connection,
    task_id: &str,
    current_stage: &str,
    status: &str,
    progress: u8,
    message: &str,
    error_message: &str,
    warnings: Option<&[String]>,
    updated_at: &str,
) -> Result<(), String> {
    let warnings_text = warnings
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| format!("无法序列化配音警告: {error}"))?;
    let changed = connection
        .execute(
            "
            UPDATE dubbing_tasks
            SET current_stage = ?2,
                status = ?3,
                progress = ?4,
                message = ?5,
                error_message = ?6,
                warnings = COALESCE(?7, warnings),
                revision = revision + 1,
                updated_at = ?8
            WHERE id = ?1
            ",
            params![
                task_id,
                current_stage,
                status,
                progress,
                message,
                error_message,
                warnings_text,
                updated_at,
            ],
        )
        .map_err(|error| format!("无法更新配音任务状态: {error}"))?;

    if changed == 0 {
        Err("未找到配音任务".to_string())
    } else {
        Ok(())
    }
}

fn emit_dubbing_progress(app: &AppHandle, snapshot: &DubbingTaskSnapshot) {
    let _ = app.emit(DUBBING_PROGRESS_EVENT, snapshot);
}

#[derive(Clone)]
struct MediaSeparationProgress {
    app: AppHandle,
    task_id: String,
    options: DubbingTaskOptions,
    last_update: Arc<Mutex<(u8, String)>>,
}

impl MediaSeparationProgress {
    fn new(app: AppHandle, task_id: String, options: DubbingTaskOptions) -> Self {
        Self {
            app,
            task_id,
            options,
            last_update: Arc::new(Mutex::new((0, String::new()))),
        }
    }

    fn set(&self, progress: u8, message: &str) -> Result<(), String> {
        let progress = progress.min(99);
        {
            let mut last_update = self
                .last_update
                .lock()
                .map_err(|error| format!("配音进度锁定失败: {error}"))?;
            if progress < last_update.0 || (progress == last_update.0 && message == last_update.1) {
                return Ok(());
            }
            *last_update = (progress, message.to_string());
        }

        emit_media_separation_progress(&self.app, &self.task_id, &self.options, progress, message)
            .map(|_| ())
    }

    fn snapshot(&self) -> Option<DubbingTaskSnapshot> {
        let store = self.app.state::<SettingsStore>();
        store
            .with_connection(|connection| {
                read_dubbing_task_snapshot_by_id(connection, &self.task_id)
            })
            .ok()
    }
}

fn emit_media_separation_progress(
    app: &AppHandle,
    task_id: &str,
    options: &DubbingTaskOptions,
    progress: u8,
    message: &str,
) -> Result<DubbingTaskSnapshot, String> {
    let store = app.state::<SettingsStore>();
    let progress = progress.min(99);
    let now = Utc::now().to_rfc3339();
    let snapshot = store.with_connection(|connection| {
        update_dubbing_task_state(
            connection,
            task_id,
            DUBBING_STAGE_MEDIA_SEPARATION,
            DUBBING_STATUS_RUNNING,
            progress,
            message,
            "",
            None,
            &now,
        )?;
        upsert_dubbing_stage(
            connection,
            task_id,
            DUBBING_STAGE_MEDIA_SEPARATION,
            progress,
            message,
            "active",
            json!({ "backgroundMusic": options.is_background_music_enabled }),
            &now,
        )?;
        read_dubbing_task_snapshot_by_id(connection, task_id)
    })?;
    emit_dubbing_progress(app, &snapshot);

    Ok(snapshot)
}

fn run_subtitle_preprocess(
    snapshot: &DubbingTaskSnapshot,
) -> Result<DubbingSubtitlePreprocessResult, String> {
    let source_subtitle = snapshot
        .artifacts
        .iter()
        .find(|artifact| artifact.kind == DUBBING_ARTIFACT_SOURCE_SUBTITLE)
        .map(|artifact| PathBuf::from(&artifact.path))
        .unwrap_or_else(|| PathBuf::from(&snapshot.subtitle_path));
    let mut input = load_dubbing_subtitle_input(&source_subtitle)?;
    let mut segments = normalize_dubbing_segments(input.segments, &mut input.warnings)?;
    if segments.is_empty() {
        return Err("字幕内容为空".to_string());
    }

    segments.sort_by_key(|segment| (segment.start_time, segment.end_time));
    for (index, segment) in segments.iter_mut().enumerate() {
        if segment.end_time <= segment.start_time {
            return Err(format!("第 {} 条字幕时间轴无效", index + 1));
        }
        segment.uid = format!("dubbing-subtitle-{index}");
        segment.status = "done".to_string();
    }

    let output_dir = PathBuf::from(&snapshot.work_dir).join("subtitle_preprocess");
    fs::create_dir_all(&output_dir).map_err(|error| format!("无法创建字幕预处理目录: {error}"))?;
    let output_path = output_dir.join("subtitle_tts.srt");
    let subtitle_text = serialize_subtitle(&segments, SubtitleFormat::Srt);
    fs::write(&output_path, subtitle_text)
        .map_err(|error| format!("无法保存预处理字幕: {error}"))?;

    Ok(DubbingSubtitlePreprocessResult {
        segments,
        output_path,
        warnings: deduplicate_warnings(input.warnings),
    })
}

fn run_media_separation(
    snapshot: &DubbingTaskSnapshot,
    options: &DubbingTaskOptions,
    progress: &MediaSeparationProgress,
) -> Result<DubbingMediaSeparationResult, String> {
    let source_video = source_video_path(snapshot)?;
    let output_dir = PathBuf::from(&snapshot.work_dir).join("media_separation");
    fs::create_dir_all(&output_dir).map_err(|error| format!("无法创建音视频分离目录: {error}"))?;

    let video_extension = path_extension(&source_video).unwrap_or_else(|| "mp4".to_string());
    let muted_video_path = output_dir.join(format!("video_no_audio.{video_extension}"));
    let source_audio_path = output_dir.join("source_audio.wav");

    progress.set(8, "准备音视频分离")?;
    progress.set(20, "分离无声视频")?;
    export_video_without_audio(&source_video, &muted_video_path)?;
    progress.set(40, "无声视频分离完成")?;

    progress.set(55, "提取源音频")?;
    extract_source_audio(&source_video, &source_audio_path)?;
    progress.set(65, "源音频提取完成")?;

    let warnings = Vec::new();
    let background_music_path = if options.is_background_music_enabled {
        progress.set(68, "准备背景音乐分离")?;
        let background_music_path = output_dir.join("background_music.wav");
        separate_background_music(
            &source_audio_path,
            &output_dir,
            &background_music_path,
            progress,
        )?;
        Some(background_music_path)
    } else {
        progress.set(90, "跳过背景音乐分离")?;
        None
    };

    Ok(DubbingMediaSeparationResult {
        muted_video_path,
        source_audio_path,
        background_music_path,
        warnings: deduplicate_warnings(warnings),
    })
}

fn source_video_path(snapshot: &DubbingTaskSnapshot) -> Result<PathBuf, String> {
    let source_video = snapshot
        .artifacts
        .iter()
        .find(|artifact| artifact.kind == DUBBING_ARTIFACT_SOURCE_VIDEO)
        .map(|artifact| PathBuf::from(&artifact.path))
        .unwrap_or_else(|| PathBuf::from(&snapshot.video_path));

    if source_video.is_file() {
        Ok(source_video)
    } else {
        Err("视频素材缓存不存在".to_string())
    }
}

fn export_video_without_audio(input_path: &Path, output_path: &Path) -> Result<(), String> {
    run_ffmpeg_command(
        Command::new("ffmpeg")
            .arg("-hide_banner")
            .arg("-nostdin")
            .arg("-nostats")
            .arg("-i")
            .arg(input_path)
            .arg("-map")
            .arg("0:v:0")
            .arg("-c:v")
            .arg("copy")
            .arg("-an")
            .arg("-sn")
            .arg("-y")
            .arg(output_path),
        "无声视频分离失败",
    )
}

fn extract_source_audio(input_path: &Path, output_path: &Path) -> Result<(), String> {
    run_ffmpeg_command(
        Command::new("ffmpeg")
            .arg("-hide_banner")
            .arg("-nostdin")
            .arg("-nostats")
            .arg("-i")
            .arg(input_path)
            .arg("-map")
            .arg("0:a:0")
            .arg("-vn")
            .arg("-acodec")
            .arg("pcm_s16le")
            .arg("-ar")
            .arg("44100")
            .arg("-ac")
            .arg("2")
            .arg("-y")
            .arg(output_path),
        "源音频提取失败",
    )
}

fn separate_background_music(
    source_audio_path: &Path,
    output_dir: &Path,
    output_path: &Path,
    progress: &MediaSeparationProgress,
) -> Result<(), String> {
    let stem_output_dir = output_dir.join("stems");
    fs::create_dir_all(&stem_output_dir)
        .map_err(|error| format!("无法创建背景音乐分离目录: {error}"))?;

    let stem_paths = split_background_music_stems(source_audio_path, &stem_output_dir, progress)?;
    progress.set(97, "混合背景音乐音轨")?;
    mix_background_music_stems(&stem_paths, output_path)?;
    ensure_non_empty_file(output_path, "背景音乐分离结果为空")
}

fn split_background_music_stems(
    source_audio_path: &Path,
    output_dir: &Path,
    progress: &MediaSeparationProgress,
) -> Result<StemSplitPaths, String> {
    ensure_background_music_model_cached_from_mirror(progress)?;
    progress.set(78, "启动背景音乐分离模型")?;
    install_background_music_progress_callbacks(progress);

    let result = split_file(
        &path_to_string(source_audio_path),
        SplitOptions {
            output_dir: path_to_string(output_dir),
            model_name: BACKGROUND_MUSIC_MODEL.to_string(),
            manifest_url_override: Some(BACKGROUND_MUSIC_MODEL_MANIFEST_URL.to_string()),
        },
    )
    .map_err(|error| format!("背景音乐分离失败: {error}"))?;

    let stem_paths = StemSplitPaths {
        drums_path: PathBuf::from(result.drums_path),
        bass_path: PathBuf::from(result.bass_path),
        other_path: PathBuf::from(result.other_path),
    };

    ensure_non_empty_file(&stem_paths.drums_path, "鼓组分离结果为空")?;
    ensure_non_empty_file(&stem_paths.bass_path, "贝斯分离结果为空")?;
    ensure_non_empty_file(&stem_paths.other_path, "其他伴奏分离结果为空")?;

    Ok(stem_paths)
}

fn ensure_background_music_model_cached_from_mirror(
    progress: &MediaSeparationProgress,
) -> Result<(), String> {
    let cache_root = background_music_model_cache_root()?;
    set_stem_splitter_cache_dir(&cache_root);
    let model_path = background_music_model_cache_path(&cache_root)?;

    progress.set(69, "检查背景音乐分离模型")?;
    if background_music_model_cache_is_valid(&model_path)? {
        progress.set(77, "背景音乐分离模型已就绪")?;
        return Ok(());
    }

    if model_path.exists() {
        progress.set(69, "更新背景音乐分离模型缓存")?;
        fs::remove_file(&model_path)
            .map_err(|error| format!("无法更新背景音乐分离模型缓存: {error}"))?;
    }

    download_background_music_model_from_mirror(&model_path, progress)?;
    progress.set(76, "校验背景音乐分离模型")?;
    if background_music_model_cache_is_valid(&model_path)? {
        progress.set(77, "背景音乐分离模型已就绪")?;
        Ok(())
    } else {
        Err("背景音乐分离模型校验失败，请重试下载".to_string())
    }
}

fn background_music_model_cache_root() -> Result<PathBuf, String> {
    app_paths::stem_splitter_cache_dir()
}

fn set_stem_splitter_cache_dir(cache_root: &Path) {
    env::set_var(STEM_SPLITTER_CACHE_DIR_ENV, cache_root);
}

fn background_music_model_cache_path(cache_root: &Path) -> Result<PathBuf, String> {
    let cache_dir = cache_root.join("models");
    fs::create_dir_all(&cache_dir)
        .map_err(|error| format!("无法创建背景音乐分离模型缓存目录: {error}"))?;

    Ok(cache_dir.join(BACKGROUND_MUSIC_MODEL_CACHE_FILE))
}

fn background_music_model_cache_is_valid(path: &Path) -> Result<bool, String> {
    if !path.is_file() {
        return Ok(false);
    }

    let metadata =
        fs::metadata(path).map_err(|error| format!("无法读取背景音乐分离模型缓存: {error}"))?;
    if metadata.len() != BACKGROUND_MUSIC_MODEL_SIZE_BYTES {
        return Ok(false);
    }

    Ok(file_sha256_hex(path)? == BACKGROUND_MUSIC_MODEL_SHA256)
}

fn file_sha256_hex(path: &Path) -> Result<String, String> {
    let mut file =
        fs::File::open(path).map_err(|error| format!("无法读取背景音乐分离模型缓存: {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let bytes_read = file
            .read(&mut buffer)
            .map_err(|error| format!("无法校验背景音乐分离模型缓存: {error}"))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn download_background_music_model_from_mirror(
    output_path: &Path,
    progress: &MediaSeparationProgress,
) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("无法创建背景音乐分离模型缓存目录: {error}"))?;
    }

    let temp_path = output_path.with_extension("part");
    if temp_path.exists() {
        fs::remove_file(&temp_path)
            .map_err(|error| format!("无法清理背景音乐分离模型临时文件: {error}"))?;
    }

    let result = (|| {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(60 * 60))
            .build()
            .map_err(|error| format!("无法创建背景音乐分离模型下载客户端: {error}"))?;
        let mut response = client
            .get(BACKGROUND_MUSIC_MODEL_ARTIFACT_URL)
            .send()
            .map_err(|error| format!("无法下载背景音乐分离模型: {error}"))?
            .error_for_status()
            .map_err(|error| format!("背景音乐分离模型下载失败: {error}"))?;
        let total = response
            .content_length()
            .unwrap_or(BACKGROUND_MUSIC_MODEL_SIZE_BYTES);
        let mut file = fs::File::create(&temp_path)
            .map_err(|error| format!("无法写入背景音乐分离模型缓存: {error}"))?;
        let mut downloaded = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];

        progress.set(70, "下载背景音乐分离模型 0%")?;
        loop {
            let bytes_read = response
                .read(&mut buffer)
                .map_err(|error| format!("无法读取背景音乐分离模型下载数据: {error}"))?;
            if bytes_read == 0 {
                break;
            }
            file.write_all(&buffer[..bytes_read])
                .map_err(|error| format!("无法保存背景音乐分离模型缓存: {error}"))?;
            downloaded += bytes_read as u64;
            progress.set(
                background_music_model_download_progress(downloaded, total),
                &format!(
                    "下载背景音乐分离模型 {}%",
                    transfer_percent(downloaded, total)
                ),
            )?;
        }
        file.flush()
            .map_err(|error| format!("无法保存背景音乐分离模型缓存: {error}"))?;

        if downloaded != BACKGROUND_MUSIC_MODEL_SIZE_BYTES {
            return Err(format!(
                "背景音乐分离模型下载不完整，期望 {} 字节，实际 {} 字节",
                BACKGROUND_MUSIC_MODEL_SIZE_BYTES, downloaded
            ));
        }

        fs::rename(&temp_path, output_path)
            .map_err(|error| format!("无法更新背景音乐分离模型缓存: {error}"))
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    result
}

fn install_background_music_progress_callbacks(progress: &MediaSeparationProgress) {
    let download_progress = progress.clone();
    set_download_progress_callback(move |downloaded, total| {
        let total = if total == 0 {
            BACKGROUND_MUSIC_MODEL_SIZE_BYTES
        } else {
            total
        };
        let _ = download_progress.set(
            background_music_model_download_progress(downloaded, total),
            &format!(
                "下载背景音乐分离模型 {}%",
                transfer_percent(downloaded, total)
            ),
        );
    });

    let split_progress = progress.clone();
    set_split_progress_callback(move |event| {
        if let Some((progress, message)) = background_music_split_progress_message(event) {
            let _ = split_progress.set(progress, &message);
        }
    });
}

fn background_music_model_download_progress(downloaded: u64, total: u64) -> u8 {
    let percent = transfer_percent(downloaded, total);
    (70 + ((percent as u16 * 6) / 100) as u8).min(76)
}

fn transfer_percent(done: u64, total: u64) -> u64 {
    if total == 0 {
        return 0;
    }

    ((done.min(total) as f64 / total as f64) * 100.0).round() as u64
}

fn background_music_split_progress_message(event: SplitProgress) -> Option<(u8, String)> {
    match event {
        SplitProgress::Stage(stage) => match stage {
            "resolve_model" => Some((78, "解析背景音乐分离模型".to_string())),
            "engine_preload" => Some((80, "加载背景音乐分离模型".to_string())),
            "read_audio" => Some((82, "读取源音频".to_string())),
            "infer" => Some((84, "分离背景音乐音轨".to_string())),
            "write_stems" => Some((94, "写入分离音轨".to_string())),
            "finalize" => Some((96, "整理分离结果".to_string())),
            _ => None,
        },
        SplitProgress::Chunks { percent, .. } => {
            let progress = (84.0 + (percent.clamp(0.0, 100.0) * 0.10)).round() as u8;
            Some((
                progress.min(94),
                format!("分离背景音乐音轨 {:.0}%", percent),
            ))
        }
        SplitProgress::Writing { stem, percent, .. } => {
            let progress = (94.0 + (percent.clamp(0.0, 100.0) * 0.02)).round() as u8;
            Some((
                progress.min(96),
                format!("写入{}音轨 {:.0}%", stem, percent),
            ))
        }
        SplitProgress::Finished => Some((96, "背景音乐音轨分离完成".to_string())),
    }
}

fn mix_background_music_stems(
    stem_paths: &StemSplitPaths,
    output_path: &Path,
) -> Result<(), String> {
    run_ffmpeg_command(
        Command::new("ffmpeg")
            .arg("-hide_banner")
            .arg("-nostdin")
            .arg("-nostats")
            .arg("-i")
            .arg(&stem_paths.drums_path)
            .arg("-i")
            .arg(&stem_paths.bass_path)
            .arg("-i")
            .arg(&stem_paths.other_path)
            .arg("-filter_complex")
            .arg("[0:a][1:a][2:a]amix=inputs=3:duration=longest:dropout_transition=0:normalize=0,alimiter=limit=0.95[aout]")
            .arg("-map")
            .arg("[aout]")
            .arg("-acodec")
            .arg("pcm_s16le")
            .arg("-ar")
            .arg("44100")
            .arg("-ac")
            .arg("2")
            .arg("-y")
            .arg(output_path),
        "背景音乐混合失败",
    )
}

fn ensure_non_empty_file(path: &Path, message: &str) -> Result<(), String> {
    let metadata = fs::metadata(path).map_err(|error| format!("{message}: {error}"))?;
    if metadata.len() == 0 {
        Err(message.to_string())
    } else {
        Ok(())
    }
}

fn run_ffmpeg_command(command: &mut Command, failure_message: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }

    let output = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("{failure_message}: 无法启动 ffmpeg: {error}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("{failure_message}: {}", stderr.trim()))
    }
}

fn read_preprocessed_segments(artifacts: &[DubbingTaskArtifact]) -> Vec<TranscriptionSegment> {
    artifacts
        .iter()
        .find(|artifact| artifact.kind == DUBBING_ARTIFACT_PREPROCESSED_SUBTITLE)
        .and_then(|artifact| load_dubbing_subtitle_segments(Path::new(&artifact.path)).ok())
        .map(|mut segments| {
            for (index, segment) in segments.iter_mut().enumerate() {
                segment.uid = format!("dubbing-subtitle-{index}");
                segment.status = "done".to_string();
            }
            segments
        })
        .unwrap_or_default()
}

fn load_dubbing_subtitle_segments(path: &Path) -> Result<Vec<TranscriptionSegment>, String> {
    let mut input = load_dubbing_subtitle_input(path)?;
    normalize_dubbing_segments(input.segments, &mut input.warnings)
}

fn load_dubbing_subtitle_input(path: &Path) -> Result<DubbingSubtitleInput, String> {
    if !path.is_file() {
        return Err("字幕文件不存在".to_string());
    }

    let extension = path_extension(path).unwrap_or_default();
    if !DUBBING_SUBTITLE_EXTENSIONS
        .iter()
        .any(|supported| *supported == extension)
    {
        return Err("不支持的字幕格式".to_string());
    }

    let content = read_subtitle_text(path)?;
    let segments = match extension.as_str() {
        "srt" => parse_srt(&content),
        "vtt" => parse_vtt(&content),
        "ass" | "ssa" => parse_ass(&content),
        "lrc" => parse_lrc(&content),
        "sbv" => parse_sbv(&content),
        "smi" | "sami" => parse_sami(&content),
        "ttml" | "dfxp" => parse_ttml(&content),
        "txt" => parse_txt(&content),
        _ => Err("不支持的字幕格式".to_string()),
    }?;

    Ok(DubbingSubtitleInput {
        segments,
        warnings: Vec::new(),
    })
}

fn read_subtitle_text(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("无法读取字幕文件: {error}"))?;
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return decode_utf16(&bytes[2..], true)
            .ok_or_else(|| "无法按 UTF-16 LE 读取字幕文件".to_string());
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return decode_utf16(&bytes[2..], false)
            .ok_or_else(|| "无法按 UTF-16 BE 读取字幕文件".to_string());
    }
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8(bytes[3..].to_vec())
            .map_err(|error| format!("无法按 UTF-8 读取字幕文件: {error}"));
    }
    if let Ok(text) = String::from_utf8(bytes.clone()) {
        return Ok(text);
    }

    let (decoded, _, _) = encoding_rs::GBK.decode(&bytes);
    Ok(decoded.into_owned())
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> Option<String> {
    if bytes.len() % 2 != 0 {
        return None;
    }

    let units = bytes
        .chunks_exact(2)
        .map(|chunk| {
            if little_endian {
                u16::from_le_bytes([chunk[0], chunk[1]])
            } else {
                u16::from_be_bytes([chunk[0], chunk[1]])
            }
        })
        .collect::<Vec<_>>();
    String::from_utf16(&units).ok()
}

fn normalize_dubbing_segments(
    segments: Vec<TranscriptionSegment>,
    warnings: &mut Vec<String>,
) -> Result<Vec<TranscriptionSegment>, String> {
    let mut normalized = Vec::new();

    for (index, segment) in segments.into_iter().enumerate() {
        let text = clean_tts_subtitle_text(&segment.text);
        if text.is_empty() {
            warnings.push(format!("第 {} 条字幕清理后为空，已跳过", index + 1));
            continue;
        }

        let start_time = segment.start_time;
        let mut end_time = segment.end_time;
        if end_time <= start_time {
            end_time = start_time + MIN_DUBBING_SUBTITLE_DURATION_MS;
            warnings.push(format!(
                "第 {} 条字幕时间轴无效，已补齐为 {} 毫秒",
                index + 1,
                MIN_DUBBING_SUBTITLE_DURATION_MS
            ));
        }

        normalized.push(TranscriptionSegment {
            text,
            start_time,
            end_time,
            uid: String::new(),
            status: String::new(),
            words: Vec::new(),
        });
    }

    normalized.sort_by_key(|segment| (segment.start_time, segment.end_time));

    if normalized.is_empty() {
        Err("字幕内容为空".to_string())
    } else {
        Ok(normalized)
    }
}

fn parse_srt(content: &str) -> Result<Vec<TranscriptionSegment>, String> {
    let normalized = content
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut segments = Vec::new();

    for block in normalized.split("\n\n") {
        let lines = block
            .lines()
            .map(str::trim_end)
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>();
        if lines.is_empty() {
            continue;
        }

        let Some(time_line_index) = lines.iter().position(|line| line.contains("-->")) else {
            continue;
        };
        let (start_time, end_time) = parse_time_range(lines[time_line_index])?;
        let text = lines[time_line_index + 1..].join("\n").trim().to_string();
        push_subtitle_segment_raw(&mut segments, text, start_time, end_time);
    }

    Ok(segments)
}

fn parse_vtt(content: &str) -> Result<Vec<TranscriptionSegment>, String> {
    let normalized = content
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut segments = Vec::new();
    let mut block_lines = Vec::new();
    let mut is_note_block = false;
    let mut is_metadata_block = false;

    for line in normalized.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            push_vtt_block(&mut segments, &block_lines)?;
            block_lines.clear();
            is_note_block = false;
            is_metadata_block = false;
        } else if trimmed.starts_with("NOTE") {
            is_note_block = true;
        } else if trimmed.starts_with("STYLE") || trimmed.starts_with("REGION") {
            is_metadata_block = true;
        } else if !is_note_block && !is_metadata_block && !trimmed.starts_with("WEBVTT") {
            block_lines.push(trimmed.to_string());
        }
    }

    push_vtt_block(&mut segments, &block_lines)?;
    Ok(segments)
}

fn push_vtt_block(
    segments: &mut Vec<TranscriptionSegment>,
    lines: &[String],
) -> Result<(), String> {
    if lines.is_empty() {
        return Ok(());
    }

    let Some(time_line_index) = lines.iter().position(|line| line.contains("-->")) else {
        return Ok(());
    };
    let (start_time, end_time) = parse_time_range(&lines[time_line_index])?;
    let text = lines[time_line_index + 1..].join("\n").trim().to_string();
    push_subtitle_segment_raw(segments, text, start_time, end_time);
    Ok(())
}

fn parse_ass(content: &str) -> Result<Vec<TranscriptionSegment>, String> {
    let mut cues: HashMap<(u64, u64), AssMergedCue> = HashMap::new();
    let mut in_events = false;
    let mut start_index = 1usize;
    let mut end_index = 2usize;
    let mut style_index = 3usize;
    let mut text_index = 9usize;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("[Events]") {
            in_events = true;
            continue;
        }

        if !in_events {
            continue;
        }

        if trimmed.starts_with('[') {
            in_events = false;
            continue;
        }

        if trimmed.to_ascii_lowercase().starts_with("format:") {
            let format = trimmed
                .split_once(':')
                .map(|(_, value)| value)
                .unwrap_or_default();
            for (index, field) in format.split(',').map(str::trim).enumerate() {
                match field.to_ascii_lowercase().as_str() {
                    "start" => start_index = index,
                    "end" => end_index = index,
                    "style" => style_index = index,
                    "text" => text_index = index,
                    _ => {}
                }
            }
            continue;
        }

        if !trimmed.to_ascii_lowercase().starts_with("dialogue:") {
            continue;
        }

        let payload = trimmed
            .split_once(':')
            .map(|(_, value)| value.trim_start())
            .unwrap_or_default();
        let fields = split_ass_dialogue_fields(payload, text_index + 1);
        let Some(start_text) = fields.get(start_index) else {
            continue;
        };
        let Some(end_text) = fields.get(end_index) else {
            continue;
        };
        let Some(text) = fields.get(text_index) else {
            continue;
        };
        let start_time = parse_ass_time(start_text)?;
        let end_time = parse_ass_time(end_text)?;
        let style = fields
            .get(style_index)
            .map(|value| value.trim().to_ascii_lowercase())
            .unwrap_or_default();
        let cue = cues
            .entry((start_time, end_time))
            .or_insert_with(|| AssMergedCue {
                start_time,
                end_time,
                primary: Vec::new(),
                secondary: Vec::new(),
                other: Vec::new(),
            });
        let text = clean_ass_text(text);

        if style == "secondary" {
            cue.secondary.push(text);
        } else if style == "default" {
            cue.primary.push(text);
        } else {
            cue.other.push(text);
        }
    }

    let mut segments = Vec::new();
    let mut merged_cues = cues.into_values().collect::<Vec<_>>();
    merged_cues.sort_by_key(|cue| (cue.start_time, cue.end_time));
    for cue in merged_cues {
        let text = cue
            .secondary
            .into_iter()
            .chain(cue.primary)
            .chain(cue.other)
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        push_subtitle_segment_raw(&mut segments, text, cue.start_time, cue.end_time);
    }

    Ok(segments)
}

fn split_ass_dialogue_fields(payload: &str, expected_fields: usize) -> Vec<String> {
    if expected_fields <= 1 {
        return vec![payload.to_string()];
    }

    let mut fields = Vec::with_capacity(expected_fields);
    let mut rest = payload;
    for _ in 0..expected_fields.saturating_sub(1) {
        if let Some((field, next)) = rest.split_once(',') {
            fields.push(field.trim().to_string());
            rest = next;
        } else {
            fields.push(rest.trim().to_string());
            rest = "";
        }
    }
    fields.push(rest.trim().to_string());
    fields
}

fn clean_ass_text(text: &str) -> String {
    let mut cleaned = String::new();
    let mut in_override = false;

    for character in text.replace("\\N", "\n").replace("\\n", "\n").chars() {
        match character {
            '{' => in_override = true,
            '}' => in_override = false,
            _ if !in_override => cleaned.push(character),
            _ => {}
        }
    }

    cleaned.trim().to_string()
}

fn parse_lrc(content: &str) -> Result<Vec<TranscriptionSegment>, String> {
    let normalized = content
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut cues = Vec::<(u64, String)>::new();

    for line in normalized.lines() {
        let mut rest = line.trim();
        let mut starts = Vec::new();
        while rest.starts_with('[') {
            let Some(end_index) = rest.find(']') else {
                break;
            };
            let tag = &rest[1..end_index];
            if let Some(start_time) = parse_lrc_time_tag(tag) {
                starts.push(start_time);
                rest = rest[end_index + 1..].trim_start();
            } else {
                break;
            }
        }

        if starts.is_empty() {
            continue;
        }

        let text = rest.trim().to_string();
        for start_time in starts {
            cues.push((start_time, text.clone()));
        }
    }

    cues.sort_by_key(|(start_time, _)| *start_time);
    let mut segments = Vec::new();
    for index in 0..cues.len() {
        let (start_time, text) = &cues[index];
        let end_time = cues
            .get(index + 1)
            .map(|(next_start, _)| *next_start)
            .filter(|next_start| next_start > start_time)
            .unwrap_or(*start_time + DEFAULT_DUBBING_TEXT_DURATION_MS);
        push_subtitle_segment_raw(&mut segments, text.clone(), *start_time, end_time);
    }

    Ok(segments)
}

fn parse_lrc_time_tag(tag: &str) -> Option<u64> {
    if tag.contains(':') && tag.chars().any(|character| character.is_ascii_digit()) {
        parse_subtitle_time(tag).ok()
    } else {
        None
    }
}

fn parse_sbv(content: &str) -> Result<Vec<TranscriptionSegment>, String> {
    let normalized = content
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut segments = Vec::new();

    for block in normalized.split("\n\n") {
        let lines = block
            .lines()
            .map(str::trim_end)
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>();
        if lines.len() < 2 {
            continue;
        }

        let Some((start_text, end_text)) = lines[0].split_once(',') else {
            continue;
        };
        let start_time = parse_subtitle_time(start_text.trim())?;
        let end_time = parse_subtitle_time(end_text.trim())?;
        let text = lines[1..].join("\n");
        push_subtitle_segment_raw(&mut segments, text, start_time, end_time);
    }

    Ok(segments)
}

fn parse_sami(content: &str) -> Result<Vec<TranscriptionSegment>, String> {
    let positions = find_case_insensitive_positions(content, "<sync");
    let mut cues = Vec::<(u64, String)>::new();

    for (index, start_position) in positions.iter().enumerate() {
        let next_position = positions
            .get(index + 1)
            .copied()
            .unwrap_or_else(|| content.len());
        let Some(tag_end_relative) = content[*start_position..next_position].find('>') else {
            continue;
        };
        let tag_end = *start_position + tag_end_relative;
        let tag = &content[*start_position..=tag_end];
        let Some(start_time) = parse_numeric_attribute_ms(tag, "start") else {
            continue;
        };
        let text = clean_markup_text(&content[tag_end + 1..next_position]);
        cues.push((start_time, text));
    }

    cues.sort_by_key(|(start_time, _)| *start_time);
    let mut segments = Vec::new();
    for index in 0..cues.len() {
        let (start_time, text) = &cues[index];
        let end_time = cues
            .get(index + 1)
            .map(|(next_start, _)| *next_start)
            .filter(|next_start| next_start > start_time)
            .unwrap_or(*start_time + DEFAULT_DUBBING_TEXT_DURATION_MS);
        push_subtitle_segment_raw(&mut segments, text.clone(), *start_time, end_time);
    }

    Ok(segments)
}

fn parse_ttml(content: &str) -> Result<Vec<TranscriptionSegment>, String> {
    let mut segments = Vec::new();
    let mut cursor = 0usize;

    while let Some(relative_start) = find_case_insensitive(&content[cursor..], "<p") {
        let start = cursor + relative_start;
        let Some(relative_tag_end) = content[start..].find('>') else {
            break;
        };
        let tag_end = start + relative_tag_end;
        let tag = &content[start..=tag_end];
        let Some(start_time) = attr_value_case_insensitive(tag, "begin")
            .and_then(|value| parse_ttml_time_value(&value))
        else {
            cursor = tag_end + 1;
            continue;
        };
        let end_time = attr_value_case_insensitive(tag, "end")
            .and_then(|value| parse_ttml_time_value(&value))
            .or_else(|| {
                attr_value_case_insensitive(tag, "dur")
                    .and_then(|value| parse_ttml_time_value(&value))
                    .map(|duration| start_time + duration)
            })
            .unwrap_or(start_time + DEFAULT_DUBBING_TEXT_DURATION_MS);
        let content_start = tag_end + 1;
        let Some(relative_end) = find_case_insensitive(&content[content_start..], "</p>") else {
            break;
        };
        let content_end = content_start + relative_end;
        let text = clean_markup_text(&content[content_start..content_end]);
        push_subtitle_segment_raw(&mut segments, text, start_time, end_time);
        cursor = content_end + 4;
    }

    Ok(segments)
}

fn parse_txt(content: &str) -> Result<Vec<TranscriptionSegment>, String> {
    let normalized = content
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut segments = Vec::new();
    let mut start_time = 0_u64;

    for line in normalized
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let end_time = start_time + DEFAULT_DUBBING_TEXT_DURATION_MS;
        push_subtitle_segment_raw(&mut segments, line.to_string(), start_time, end_time);
        start_time = end_time;
    }

    Ok(segments)
}

fn push_subtitle_segment_raw(
    segments: &mut Vec<TranscriptionSegment>,
    text: String,
    start_time: u64,
    end_time: u64,
) {
    let text = text.trim().to_string();
    if text.is_empty() {
        return;
    }

    segments.push(TranscriptionSegment {
        text,
        start_time,
        end_time,
        uid: String::new(),
        status: String::new(),
        words: Vec::new(),
    });
}

fn parse_time_range(line: &str) -> Result<(u64, u64), String> {
    let (start, end) = line
        .split_once("-->")
        .ok_or_else(|| format!("无效时间轴: {line}"))?;
    let end = end.split_whitespace().next().unwrap_or_default();

    Ok((
        parse_subtitle_time(start.trim())?,
        parse_subtitle_time(end.trim())?,
    ))
}

fn parse_subtitle_time(text: &str) -> Result<u64, String> {
    let normalized = text.trim().replace(',', ".");
    let parts = normalized.split(':').collect::<Vec<_>>();
    if parts.len() < 2 || parts.len() > 3 {
        return Err(format!("无效字幕时间: {text}"));
    }

    let (hours, minutes, seconds_text) = if parts.len() == 3 {
        (
            parts[0]
                .parse::<u64>()
                .map_err(|_| format!("无效字幕时间: {text}"))?,
            parts[1]
                .parse::<u64>()
                .map_err(|_| format!("无效字幕时间: {text}"))?,
            parts[2],
        )
    } else {
        (
            0,
            parts[0]
                .parse::<u64>()
                .map_err(|_| format!("无效字幕时间: {text}"))?,
            parts[1],
        )
    };

    let (seconds, millis) = parse_seconds_millis(seconds_text)?;
    Ok((((hours * 60 + minutes) * 60 + seconds) * 1000) + millis)
}

fn parse_ass_time(text: &str) -> Result<u64, String> {
    parse_subtitle_time(text)
}

fn parse_seconds_millis(text: &str) -> Result<(u64, u64), String> {
    let (seconds_text, fraction_text) = text.split_once('.').unwrap_or((text, ""));
    let seconds = seconds_text
        .parse::<u64>()
        .map_err(|_| format!("无效字幕秒数: {text}"))?;
    let millis = if fraction_text.is_empty() {
        0
    } else {
        let mut fraction = fraction_text.chars().take(3).collect::<String>();
        while fraction.len() < 3 {
            fraction.push('0');
        }
        fraction
            .parse::<u64>()
            .map_err(|_| format!("无效字幕毫秒: {text}"))?
    };

    Ok((seconds, millis))
}

fn parse_ttml_time_value(text: &str) -> Option<u64> {
    let trimmed = text.trim().trim_matches('"').trim_matches('\'').trim();
    if let Some(value) = trimmed.strip_suffix("ms") {
        return value
            .trim()
            .parse::<f64>()
            .ok()
            .map(|milliseconds| milliseconds.max(0.0).round() as u64);
    }
    if let Some(value) = trimmed.strip_suffix('s') {
        return value
            .trim()
            .parse::<f64>()
            .ok()
            .map(|seconds| (seconds.max(0.0) * 1000.0).round() as u64);
    }
    if trimmed.contains(':') {
        return parse_subtitle_time(trimmed).ok();
    }

    trimmed
        .parse::<f64>()
        .ok()
        .map(|seconds| (seconds.max(0.0) * 1000.0).round() as u64)
}

fn parse_numeric_attribute_ms(tag: &str, attribute: &str) -> Option<u64> {
    attr_value_case_insensitive(tag, attribute).and_then(|value| {
        value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .parse::<u64>()
            .ok()
    })
}

fn attr_value_case_insensitive(tag: &str, attribute: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let attribute = attribute.to_ascii_lowercase();
    let start = lower.find(&attribute)?;
    let mut index = start + attribute.len();
    let chars = tag[index..].char_indices().collect::<Vec<_>>();
    for (offset, character) in chars {
        if character.is_whitespace() {
            index = start + attribute.len() + offset + character.len_utf8();
            continue;
        }
        if character == '=' {
            index = start + attribute.len() + offset + character.len_utf8();
            break;
        }
        return None;
    }

    let value = &tag[index..];
    let value = value.trim_start();
    if value.is_empty() {
        return None;
    }

    let mut chars = value.chars();
    let first = chars.next()?;
    if first == '"' || first == '\'' {
        let rest = &value[first.len_utf8()..];
        let end = rest.find(first)?;
        return Some(rest[..end].to_string());
    }

    Some(
        value
            .chars()
            .take_while(|character| {
                !character.is_whitespace() && *character != '>' && *character != '/'
            })
            .collect(),
    )
}

fn clean_tts_subtitle_text(text: &str) -> String {
    let text = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace("\\N", "\n")
        .replace("\\n", "\n")
        .replace("\\h", " ");
    let text = strip_ass_overrides(&text);
    let text = strip_markup_tags(&text);
    let text = html_escape::decode_html_entities(&text).to_string();
    let text = strip_markup_tags(&text);
    let mut lines = text
        .lines()
        .map(clean_tts_text_line)
        .filter(|line| !line.is_empty() && !is_pure_bracket_content(line))
        .collect::<Vec<_>>();

    if lines.is_empty() {
        return String::new();
    }

    if let Some(first_chinese_line) = lines.iter().position(|line| contains_chinese(line)) {
        if first_chinese_line > 0 {
            lines = lines[first_chinese_line..].to_vec();
        }
    }

    let text = lines.join("\n");
    let text = oralize_trading_units(&text);
    ensure_sentence_punctuation(&collapse_repeated_punctuation(&text))
}

fn clean_markup_text(text: &str) -> String {
    let text = strip_markup_tags(text);
    let text = html_escape::decode_html_entities(&text).to_string();
    let text = strip_markup_tags(&text);
    text.replace('\u{00a0}', " ").trim().to_string()
}

fn clean_tts_text_line(line: &str) -> String {
    let mut cleaned = String::new();
    let mut previous_space = false;

    for character in line.chars() {
        if matches!(
            character,
            '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{feff}' | '\u{0000}'..='\u{0008}'
                | '\u{000b}' | '\u{000c}' | '\u{000e}'..='\u{001f}' | '\u{007f}'
        ) {
            continue;
        }

        let normalized = match character {
            '"' | '\'' | '‘' | '’' | '“' | '”' | '「' | '」' | '『' | '』' | '《' | '》' => {
                ' '
            }
            '—' | '–' => '，',
            _ => character,
        };

        if normalized.is_whitespace() {
            if !previous_space {
                cleaned.push(' ');
                previous_space = true;
            }
        } else {
            cleaned.push(normalized);
            previous_space = false;
        }
    }

    cleaned.trim().to_string()
}

fn strip_ass_overrides(text: &str) -> String {
    strip_between(text, '{', '}')
}

fn strip_markup_tags(text: &str) -> String {
    strip_between(text, '<', '>')
}

fn strip_between(text: &str, open: char, close: char) -> String {
    let mut output = String::new();
    let mut depth = 0_u32;

    for character in text.chars() {
        if character == open {
            depth = depth.saturating_add(1);
            continue;
        }
        if character == close && depth > 0 {
            depth -= 1;
            continue;
        }
        if depth == 0 {
            output.push(character);
        }
    }

    output
}

fn is_pure_bracket_content(text: &str) -> bool {
    let value = text.trim();
    if value.chars().count() < 2 {
        return false;
    }

    let pairs = [
        ('(', ')'),
        ('[', ']'),
        ('{', '}'),
        ('<', '>'),
        ('（', '）'),
        ('【', '】'),
        ('「', '」'),
        ('『', '』'),
    ];
    pairs.iter().any(|(open, close)| {
        value.starts_with(*open)
            && value.ends_with(*close)
            && value
                .trim_start_matches(*open)
                .trim_end_matches(*close)
                .trim()
                .chars()
                .any(|character| character.is_alphanumeric() || contains_chinese_char(character))
    })
}

fn contains_chinese(text: &str) -> bool {
    text.chars().any(contains_chinese_char)
}

fn contains_chinese_char(character: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&character)
}

fn oralize_trading_units(text: &str) -> String {
    let mut result = text.to_string();
    for unit in [
        "tick", "ticks", "Tick", "Ticks", "pip", "pips", "Pip", "Pips",
    ] {
        result = result.replace(&format!("二{unit}"), &format!("两{unit}"));
        result = result.replace(&format!("二 {unit}"), &format!("两 {unit}"));
    }
    result
}

fn ensure_sentence_punctuation(text: &str) -> String {
    let stripped = text.trim();
    if stripped.is_empty() {
        return String::new();
    }

    let closing_marks = "」』】）》〉］｝)}’”'\"";
    let core = stripped.trim_end_matches(|character| closing_marks.contains(character));
    let suffix = &stripped[core.len()..];
    if core.is_empty() || ends_with_sentence_punctuation(core) {
        return stripped.to_string();
    }

    let punctuation = if contains_chinese(core) { "。" } else { "." };
    format!("{core}{punctuation}{suffix}")
}

fn ends_with_sentence_punctuation(text: &str) -> bool {
    text.ends_with("……")
        || text.ends_with("?!")
        || text.ends_with("!?")
        || text
            .chars()
            .last()
            .is_some_and(|character| "。！？!?.,;:；：".contains(character))
}

fn collapse_repeated_punctuation(text: &str) -> String {
    let mut output = String::new();
    let mut previous = None;

    for character in text.chars() {
        if previous == Some(character) && "。！？!?.,;:；：".contains(character) {
            continue;
        }
        output.push(character);
        previous = Some(character);
    }

    while output.contains("………") {
        output = output.replace("………", "……");
    }

    output
}

fn deduplicate_warnings(warnings: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    warnings
        .into_iter()
        .filter(|warning| seen.insert(warning.clone()))
        .collect()
}

fn find_case_insensitive_positions(text: &str, pattern: &str) -> Vec<usize> {
    let lower_text = text.to_ascii_lowercase();
    let lower_pattern = pattern.to_ascii_lowercase();
    let mut positions = Vec::new();
    let mut cursor = 0usize;

    while let Some(relative_position) = lower_text[cursor..].find(&lower_pattern) {
        let position = cursor + relative_position;
        positions.push(position);
        cursor = position + lower_pattern.len();
    }

    positions
}

fn find_case_insensitive(text: &str, pattern: &str) -> Option<usize> {
    text.to_ascii_lowercase()
        .find(&pattern.to_ascii_lowercase())
}

fn engine_label(engine: &str) -> &'static str {
    match engine {
        EDGE_TTS_ENGINE => EDGE_TTS_ENGINE_LABEL,
        NANO_AI_TTS_ENGINE => NANO_AI_TTS_ENGINE_LABEL,
        INDEX_TTS2_ENGINE => INDEX_TTS2_ENGINE_LABEL,
        _ => "未知引擎",
    }
}

impl DubbingEngine for EdgeTtsEngine {
    fn list_voices(&self) -> Result<Vec<DubbingVoiceOption>, String> {
        let url = format!(
            "https://{EDGE_TTS_BASE_URL}/voices/list?trustedclienttoken={EDGE_TTS_TRUSTED_CLIENT_TOKEN}&Sec-MS-GEC={}&Sec-MS-GEC-Version={EDGE_TTS_SEC_MS_GEC_VERSION}",
            generate_sec_ms_gec()?
        );
        let voices = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|error| format!("无法创建 EDGE-TTS 客户端: {error}"))?
            .get(url)
            .headers(edge_voice_headers())
            .send()
            .map_err(|error| format!("无法获取 EDGE-TTS 语音列表: {error}"))?
            .error_for_status()
            .map_err(|error| format!("EDGE-TTS 语音列表请求失败: {error}"))?
            .json::<Vec<EdgeTtsVoice>>()
            .map_err(|error| format!("无法解析 EDGE-TTS 语音列表: {error}"))?;

        Ok(voices
            .into_iter()
            .map(|voice| DubbingVoiceOption {
                engine: EDGE_TTS_ENGINE.to_string(),
                engine_label: EDGE_TTS_ENGINE_LABEL.to_string(),
                model_key: voice.short_name,
                display_name: voice.friendly_name,
                locale: voice.locale,
                gender: voice.gender,
                metadata: json!({
                    "voiceTag": voice.voice_tag,
                }),
            })
            .collect())
    }

    fn synthesize_preview(
        &self,
        model_key: &str,
        locale: Option<&str>,
        _endpoint: Option<&str>,
    ) -> Result<Vec<u8>, String> {
        synthesize_edge_tts_audio(model_key, preview_text_for_voice(model_key, locale))
    }
}

impl DubbingEngine for NanoAiTtsEngine {
    fn list_voices(&self) -> Result<Vec<DubbingVoiceOption>, String> {
        let response = nano_ai_client()?
            .get(format!("{NANO_AI_TTS_BASE_URL}/api/robot/platform"))
            .headers(nano_ai_headers()?)
            .send()
            .map_err(|error| format!("无法获取纳米AI TTS 语音列表: {error}"))?
            .error_for_status()
            .map_err(|error| format!("纳米AI TTS 语音列表请求失败: {error}"))?
            .json::<NanoAiPlatformResponse>()
            .map_err(|error| format!("无法解析纳米AI TTS 语音列表: {error}"))?;

        let mut seen_model_keys = HashSet::new();
        let voices = response
            .data
            .list
            .into_iter()
            .filter_map(|robot| {
                let model_key = robot.tag.trim().to_string();
                if model_key.is_empty() || !seen_model_keys.insert(model_key.clone()) {
                    return None;
                }

                let title = robot.title.trim();
                Some(DubbingVoiceOption {
                    engine: NANO_AI_TTS_ENGINE.to_string(),
                    engine_label: NANO_AI_TTS_ENGINE_LABEL.to_string(),
                    model_key: model_key.clone(),
                    display_name: if title.is_empty() {
                        model_key
                    } else {
                        title.to_string()
                    },
                    locale: "zh-CN".to_string(),
                    gender: String::new(),
                    metadata: json!({
                        "iconUrl": robot.icon,
                    }),
                })
            })
            .collect::<Vec<_>>();

        if voices.is_empty() {
            return Err("纳米AI TTS 未返回语音列表".to_string());
        }

        Ok(voices)
    }

    fn synthesize_preview(
        &self,
        model_key: &str,
        locale: Option<&str>,
        _endpoint: Option<&str>,
    ) -> Result<Vec<u8>, String> {
        let preview_locale = locale
            .filter(|value| !value.trim().is_empty())
            .or(Some("zh-CN"));
        synthesize_nano_ai_tts_audio(model_key, preview_text_for_voice(model_key, preview_locale))
    }
}

impl DubbingEngine for IndexTts2Engine {
    fn list_voices(&self) -> Result<Vec<DubbingVoiceOption>, String> {
        Ok(index_tts2_templates()
            .iter()
            .map(|template| DubbingVoiceOption {
                engine: INDEX_TTS2_ENGINE.to_string(),
                engine_label: INDEX_TTS2_ENGINE_LABEL.to_string(),
                model_key: template.model_key.to_string(),
                display_name: template.display_name.to_string(),
                locale: template.locale.to_string(),
                gender: String::new(),
                metadata: index_tts2_metadata(INDEX_TTS2_ENDPOINT),
            })
            .collect())
    }

    fn synthesize_preview(
        &self,
        model_key: &str,
        locale: Option<&str>,
        endpoint: Option<&str>,
    ) -> Result<Vec<u8>, String> {
        let template = index_tts2_template(model_key)?;
        let endpoint = normalize_index_tts2_endpoint(endpoint)?;
        synthesize_index_tts2_audio(
            template,
            preview_text_for_voice(model_key, locale.or(Some(template.locale))),
            &endpoint,
        )
    }
}

fn index_tts2_templates() -> &'static [IndexTts2Template] {
    &[
        IndexTts2Template {
            model_key: "index-tts-2.0-zh",
            display_name: "Index-TTS 2.0 中文版",
            locale: "zh-CN",
            emo_control_method: "与音色参考音频相同",
        },
        IndexTts2Template {
            model_key: "index-tts-2.0-en",
            display_name: "Index-TTS 2.0 英文版",
            locale: "en-US",
            emo_control_method: "Same as the voice reference",
        },
    ]
}

fn index_tts2_template(model_key: &str) -> Result<&'static IndexTts2Template, String> {
    index_tts2_templates()
        .iter()
        .find(|template| template.model_key == model_key)
        .ok_or_else(|| "未找到该 Index-TTS 2.0 模型".to_string())
}

fn index_tts2_metadata(endpoint: &str) -> Value {
    json!({
        "endpoint": endpoint,
        "apiName": format!("/{INDEX_TTS2_API_NAME}"),
    })
}

fn normalize_index_tts2_endpoint(endpoint: Option<&str>) -> Result<String, String> {
    let value = endpoint
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(INDEX_TTS2_ENDPOINT);
    let value = if value.starts_with("http://") || value.starts_with("https://") {
        value.to_string()
    } else {
        format!("http://{value}")
    };
    let mut url = reqwest::Url::parse(&value)
        .map_err(|error| format!("Index-TTS 2.0 服务地址无效: {error}"))?;

    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("Index-TTS 2.0 服务地址必须是 HTTP/HTTPS 地址".to_string());
    }

    url.set_query(None);
    url.set_fragment(None);

    Ok(url.as_str().trim_end_matches('/').to_string())
}

fn synthesize_index_tts2_audio(
    template: &IndexTts2Template,
    text: &str,
    endpoint: &str,
) -> Result<Vec<u8>, String> {
    let client = index_tts2_client()?;
    let gradio = load_gradio_config(&client, endpoint)?;
    let uploaded_audio = upload_index_tts2_reference_audio(&client, &gradio.upload_url)?;
    let session_hash = connect_id();
    let output = submit_index_tts2_job(
        &client,
        &gradio,
        &session_hash,
        index_tts2_payload(template, text, uploaded_audio, &gradio.inputs),
    )?;
    let audio = download_gradio_audio(&client, &gradio, &output)?;

    if audio.is_empty() {
        return Err("Index-TTS 2.0 未返回试听音频".to_string());
    }

    Ok(audio.to_vec())
}

struct GradioRuntimeConfig {
    fn_index: usize,
    base_url: String,
    src_prefixed: String,
    sse_url: String,
    sse_data_url: String,
    upload_url: String,
    inputs: Vec<GradioInputComponent>,
}

struct GradioInputComponent {
    component_type: String,
    label: String,
    key: String,
    default_value: Value,
    choices: Value,
}

fn index_tts2_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|error| format!("无法创建 Index-TTS 2.0 客户端: {error}"))
}

fn load_gradio_config(client: &Client, endpoint: &str) -> Result<GradioRuntimeConfig, String> {
    let base_url = trailing_slash(endpoint);
    let config = client
        .get(format!("{base_url}config"))
        .send()
        .map_err(|error| format!("无法连接 Index-TTS 2.0 Gradio 服务: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Index-TTS 2.0 Gradio 配置请求失败: {error}"))?
        .json::<GradioConfig>()
        .map_err(|error| format!("无法解析 Index-TTS 2.0 Gradio 配置: {error}"))?;

    let protocol = if config.protocol.trim().is_empty() {
        "sse_v1"
    } else {
        config.protocol.trim()
    };
    if !protocol.starts_with("sse") {
        return Err(format!(
            "暂不支持该 Gradio 协议: {protocol}，请使用支持 SSE 的 Index-TTS 2.0 服务"
        ));
    }

    let (dependency_index, dependency) = config
        .dependencies
        .iter()
        .enumerate()
        .find(|(_, dependency)| gradio_api_name_matches(&dependency.api_name, INDEX_TTS2_API_NAME))
        .ok_or_else(|| "Index-TTS 2.0 Gradio 服务未暴露 /gen_single 接口".to_string())?;
    let fn_index = dependency.id.unwrap_or(dependency_index);
    let dependency_inputs = dependency.inputs.clone();
    let api_prefix = config.api_prefix.trim().trim_matches('/');
    let src_prefixed = if api_prefix.is_empty() {
        base_url.clone()
    } else {
        format!("{base_url}{api_prefix}/")
    };
    let components = config
        .components
        .into_iter()
        .map(|component| (component.id, component))
        .collect::<HashMap<_, _>>();
    let inputs = dependency_inputs
        .iter()
        .filter_map(|input_id| components.get(input_id))
        .map(|component| GradioInputComponent {
            component_type: component.component_type.clone(),
            label: component.props.label.clone().unwrap_or_default(),
            key: component_key_text(&component.props.key),
            default_value: component.props.value.clone(),
            choices: component.props.choices.clone(),
        })
        .collect::<Vec<_>>();

    Ok(GradioRuntimeConfig {
        fn_index,
        base_url,
        sse_url: format!("{src_prefixed}queue/data"),
        sse_data_url: format!("{src_prefixed}queue/join"),
        upload_url: format!("{src_prefixed}upload"),
        src_prefixed,
        inputs,
    })
}

fn upload_index_tts2_reference_audio(client: &Client, upload_url: &str) -> Result<Value, String> {
    let audio_part = Part::bytes(INDEX_TTS2_SAMPLE_AUDIO.to_vec())
        .file_name("audio_sample.mp3")
        .mime_str("audio/mpeg")
        .map_err(|error| format!("无法准备 Index-TTS 2.0 参考音频: {error}"))?;
    let uploaded_paths = client
        .post(upload_url)
        .multipart(Form::new().part("files", audio_part))
        .send()
        .map_err(|error| format!("无法上传 Index-TTS 2.0 参考音频: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Index-TTS 2.0 参考音频上传失败: {error}"))?
        .json::<Vec<String>>()
        .map_err(|error| format!("无法解析 Index-TTS 2.0 参考音频上传结果: {error}"))?;
    let uploaded_path = uploaded_paths
        .first()
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| "Index-TTS 2.0 未返回参考音频上传路径".to_string())?;

    Ok(json!({
        "path": uploaded_path,
        "orig_name": "audio_sample.mp3",
        "meta": { "_type": "gradio.FileData" },
    }))
}

fn index_tts2_payload(
    template: &IndexTts2Template,
    text: &str,
    reference_audio: Value,
    inputs: &[GradioInputComponent],
) -> Vec<Value> {
    if inputs.is_empty() {
        return vec![
            json!(template.emo_control_method),
            reference_audio.clone(),
            json!(text),
            reference_audio,
            json!(0.8),
            json!(0),
            json!(0),
            json!(0),
            json!(0),
            json!(0),
            json!(0),
            json!(0),
            json!(0),
            json!(""),
            json!(false),
            json!(120),
            json!(true),
            json!(0.8),
            json!(30),
            json!(0.8),
            json!(0),
            json!(3),
            json!(10),
            json!(1500),
        ];
    }

    inputs
        .iter()
        .map(|input| index_tts2_input_value(template, text, &reference_audio, input))
        .collect()
}

fn index_tts2_input_value(
    template: &IndexTts2Template,
    text: &str,
    reference_audio: &Value,
    input: &GradioInputComponent,
) -> Value {
    let identity = format!(
        "{} {} {}",
        input.key.to_lowercase(),
        input.label.to_lowercase(),
        input.component_type.to_lowercase()
    );
    let component_type = input.component_type.to_lowercase();

    if component_type == "audio"
        || identity_contains_any(
            &identity,
            &["prompt_audio", "reference_audio", "emo_ref_path"],
        )
    {
        return reference_audio.clone();
    }

    if identity_contains_any(
        &identity,
        &[
            "max_text_tokens_per_segment",
            "max_text_tokens",
            "分句最大token",
        ],
    ) {
        return json!(120);
    }

    if identity_contains_any(
        &identity,
        &[
            "input_text_single",
            "target_text",
            "input text",
            "目标文本",
            "文本",
        ],
    ) && !identity_contains_any(&identity, &["情感描述", "emotion text", "emo_text"])
    {
        return json!(text);
    }

    if component_type == "radio"
        && identity_contains_any(&identity, &["情感控制", "emotion control", "emo_control"])
    {
        return json!(index_tts2_emo_control_value(template, input));
    }

    if identity_contains_any(&identity, &["emo_weight", "情感权重", "emotion weight"]) {
        return json!(0.8);
    }

    if identity_contains_any(
        &identity,
        &[
            "vec1", "vec2", "vec3", "vec4", "vec5", "vec6", "vec7", "vec8", "喜", "怒", "哀", "惧",
            "厌恶", "低落", "惊喜", "平静",
        ],
    ) {
        return json!(0);
    }

    if identity_contains_any(&identity, &["emo_text", "情感描述", "emotion text"]) {
        return json!("");
    }

    if identity_contains_any(&identity, &["emo_random", "情感随机", "emotion random"]) {
        return json!(false);
    }

    if identity_contains_any(&identity, &["do_sample"]) {
        return json!(true);
    }

    if identity_contains_any(&identity, &["top_p"]) {
        return json!(0.8);
    }

    if identity_contains_any(&identity, &["top_k"]) {
        return json!(30);
    }

    if identity_contains_any(&identity, &["temperature"]) {
        return json!(0.8);
    }

    if identity_contains_any(&identity, &["length_penalty"]) {
        return json!(0);
    }

    if identity_contains_any(&identity, &["num_beams"]) {
        return json!(3);
    }

    if identity_contains_any(&identity, &["repetition_penalty"]) {
        return json!(10);
    }

    if identity_contains_any(&identity, &["max_mel_tokens"]) {
        return json!(1500);
    }

    if !input.default_value.is_null() {
        return input.default_value.clone();
    }

    match component_type.as_str() {
        "checkbox" => json!(false),
        "number" | "slider" => json!(0),
        "textbox" => json!(""),
        "radio" | "dropdown" => first_gradio_choice_value(&input.choices)
            .map(Value::String)
            .unwrap_or_else(|| json!("")),
        "audio" | "file" => reference_audio.clone(),
        _ => Value::Null,
    }
}

fn gradio_api_name_matches(value: &Value, expected: &str) -> bool {
    let expected_without_slash = expected.trim_start_matches('/');
    let expected_with_slash = format!("/{expected_without_slash}");

    match value {
        Value::String(value) => value == expected_without_slash || value == &expected_with_slash,
        Value::Array(values) => values
            .iter()
            .any(|value| gradio_api_name_matches(value, expected)),
        _ => false,
    }
}

fn index_tts2_emo_control_value(
    template: &IndexTts2Template,
    input: &GradioInputComponent,
) -> String {
    let preferred = template.emo_control_method;
    let alternatives = if preferred == "Same as the voice reference" {
        ["Same as the voice reference", "与音色参考音频相同"]
    } else {
        ["与音色参考音频相同", "Same as the voice reference"]
    };
    let choices = gradio_choice_values(&input.choices);

    for candidate in alternatives {
        if choices.iter().any(|choice| choice == candidate) {
            return candidate.to_string();
        }
    }

    if choices.iter().any(|choice| choice == preferred) {
        return preferred.to_string();
    }

    choices
        .into_iter()
        .next()
        .unwrap_or_else(|| preferred.to_string())
}

fn identity_contains_any(identity: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| identity.contains(needle))
}

fn component_key_text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Array(values) => values
            .iter()
            .map(component_key_text)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join("."),
        _ => String::new(),
    }
}

fn first_gradio_choice_value(value: &Value) -> Option<String> {
    gradio_choice_values(value).into_iter().next()
}

fn gradio_choice_values(value: &Value) -> Vec<String> {
    match value {
        Value::Array(values) => values
            .iter()
            .filter_map(|value| match value {
                Value::String(value) => Some(value.clone()),
                Value::Array(pair) => pair
                    .get(1)
                    .or_else(|| pair.first())
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn submit_index_tts2_job(
    client: &Client,
    gradio: &GradioRuntimeConfig,
    session_hash: &str,
    data: Vec<Value>,
) -> Result<Value, String> {
    let join_response = client
        .post(&gradio.sse_data_url)
        .json(&json!({
            "data": data,
            "fn_index": gradio.fn_index,
            "session_hash": session_hash,
        }))
        .send()
        .map_err(|error| format!("无法提交 Index-TTS 2.0 试听任务: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Index-TTS 2.0 试听任务提交失败: {error}"))?
        .json::<GradioQueueJoinResponse>()
        .map_err(|error| format!("无法解析 Index-TTS 2.0 任务提交结果: {error}"))?;

    read_gradio_sse_result(client, gradio, session_hash, &join_response.event_id)
}

fn read_gradio_sse_result(
    client: &Client,
    gradio: &GradioRuntimeConfig,
    session_hash: &str,
    event_id: &str,
) -> Result<Value, String> {
    let response = client
        .get(&gradio.sse_url)
        .query(&[("session_hash", session_hash)])
        .send()
        .map_err(|error| format!("无法读取 Index-TTS 2.0 试听任务状态: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Index-TTS 2.0 试听任务状态请求失败: {error}"))?;
    let reader = BufReader::new(response);

    for line in reader.lines() {
        let line = line.map_err(|error| format!("读取 Index-TTS 2.0 SSE 失败: {error}"))?;
        let Some(payload) = line.strip_prefix("data:") else {
            continue;
        };
        let message = serde_json::from_str::<Value>(payload.trim())
            .map_err(|error| format!("无法解析 Index-TTS 2.0 SSE 消息: {error}"))?;
        let message_type = message.get("msg").and_then(Value::as_str).unwrap_or("");

        if matches!(message_type, "heartbeat" | "estimation" | "process_starts") {
            continue;
        }

        if message
            .get("event_id")
            .and_then(Value::as_str)
            .is_some_and(|value| value != event_id)
        {
            continue;
        }

        if message_type == "process_completed" {
            if !message
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(true)
            {
                let error_text = message
                    .get("output")
                    .and_then(|output| output.get("error"))
                    .and_then(Value::as_str)
                    .unwrap_or("Index-TTS 2.0 试听任务失败");
                return Err(error_text.to_string());
            }

            return Ok(message
                .get("output")
                .cloned()
                .ok_or_else(|| "Index-TTS 2.0 试听任务未返回输出".to_string())?);
        }
    }

    Err("Index-TTS 2.0 试听任务连接已结束，但未收到完成结果".to_string())
}

fn download_gradio_audio(
    client: &Client,
    gradio: &GradioRuntimeConfig,
    output: &Value,
) -> Result<Vec<u8>, String> {
    if let Some(path) = find_gradio_file_path(output) {
        if let Some(audio) = read_local_gradio_file(&path)? {
            return Ok(audio);
        }
    }

    let candidates = gradio_audio_url_candidates(gradio, output)?;
    let mut last_error = String::new();

    for url in candidates {
        match client.get(&url).send() {
            Ok(response) => {
                let status = response.status();
                if !status.is_success() {
                    last_error = format!("HTTP {status} ({url})");
                    continue;
                }

                let audio = response
                    .bytes()
                    .map_err(|error| format!("无法读取 Index-TTS 2.0 试听音频: {error}"))?;
                if !audio.is_empty() {
                    return Ok(audio.to_vec());
                }

                last_error = format!("空音频 ({url})");
            }
            Err(error) => {
                last_error = format!("{error} ({url})");
            }
        }
    }

    if last_error.is_empty() {
        Err("未能解析 Index-TTS 2.0 返回的音频文件".to_string())
    } else {
        Err(format!("Index-TTS 2.0 音频下载失败: {last_error}"))
    }
}

fn find_gradio_file_url(value: &Value) -> Option<String> {
    match value {
        Value::Object(object) => object
            .get("url")
            .and_then(Value::as_str)
            .filter(|url| !url.trim().is_empty())
            .map(ToString::to_string)
            .or_else(|| object.values().find_map(find_gradio_file_url)),
        Value::Array(values) => values.iter().find_map(find_gradio_file_url),
        _ => None,
    }
}

fn find_gradio_file_path(value: &Value) -> Option<String> {
    match value {
        Value::Object(object) => object
            .get("path")
            .and_then(Value::as_str)
            .filter(|path| !path.trim().is_empty())
            .map(ToString::to_string)
            .or_else(|| object.values().find_map(find_gradio_file_path)),
        Value::Array(values) => values.iter().find_map(find_gradio_file_path),
        Value::String(value) if !value.trim().is_empty() => Some(value.to_string()),
        _ => None,
    }
}

fn read_local_gradio_file(path: &str) -> Result<Option<Vec<u8>>, String> {
    let path = Path::new(path);
    if !path.exists() || !path.is_file() {
        return Ok(None);
    }

    let audio = fs::read(path).map_err(|error| {
        format!(
            "无法读取 Index-TTS 2.0 本地试听音频 {}: {error}",
            path.display()
        )
    })?;

    if audio.is_empty() {
        Ok(None)
    } else {
        Ok(Some(audio))
    }
}

fn gradio_audio_url_candidates(
    gradio: &GradioRuntimeConfig,
    output: &Value,
) -> Result<Vec<String>, String> {
    let mut candidates = Vec::new();

    if let Some(url) = find_gradio_file_url(output) {
        push_gradio_url_candidate(&mut candidates, &gradio.base_url, &url)?;
        if let Some(relative_url) = absolute_gradio_file_relative_url(&url) {
            push_gradio_url_candidate(&mut candidates, &gradio.base_url, &relative_url)?;
        }
    }

    if let Some(path) = find_gradio_file_path(output) {
        push_gradio_url_candidate(
            &mut candidates,
            &gradio.src_prefixed,
            &format!("file={path}"),
        )?;
        push_gradio_url_candidate(
            &mut candidates,
            &gradio.src_prefixed,
            &format!("file={}", percent_encode_path_value(&path)),
        )?;
        push_gradio_url_candidate(&mut candidates, &gradio.base_url, &format!("file={path}"))?;
        push_gradio_url_candidate(
            &mut candidates,
            &gradio.base_url,
            &format!("file={}", percent_encode_path_value(&path)),
        )?;
    }

    candidates.dedup();
    Ok(candidates)
}

fn push_gradio_url_candidate(
    candidates: &mut Vec<String>,
    base_url: &str,
    file_url: &str,
) -> Result<(), String> {
    let url = normalize_gradio_file_url(base_url, file_url)?;
    if !candidates.iter().any(|candidate| candidate == &url) {
        candidates.push(url);
    }

    Ok(())
}

fn normalize_gradio_file_url(base_url: &str, file_url: &str) -> Result<String, String> {
    let base = reqwest::Url::parse(base_url)
        .map_err(|error| format!("Index-TTS 2.0 服务地址无效: {error}"))?;

    if file_url.starts_with("http://") || file_url.starts_with("https://") {
        return Ok(file_url.to_string());
    }

    base.join(file_url)
        .map(|url| url.to_string())
        .map_err(|error| format!("Index-TTS 2.0 音频地址无效: {error}"))
}

fn absolute_gradio_file_relative_url(value: &str) -> Option<String> {
    let url = reqwest::Url::parse(value).ok()?;
    let mut relative_url = url.path().to_string();
    if let Some(query) = url.query() {
        relative_url.push('?');
        relative_url.push_str(query);
    }

    Some(relative_url)
}

fn percent_encode_path_value(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn trailing_slash(value: &str) -> String {
    if value.ends_with('/') {
        value.to_string()
    } else {
        format!("{value}/")
    }
}

fn synthesize_nano_ai_tts_audio(role_id: &str, text: &str) -> Result<Vec<u8>, String> {
    let mut url = reqwest::Url::parse(&format!("{NANO_AI_TTS_BASE_URL}/api/tts/v1"))
        .map_err(|error| format!("无法创建纳米AI TTS 请求地址: {error}"))?;
    url.query_pairs_mut().append_pair("roleid", role_id);

    let response = nano_ai_client()?
        .post(url)
        .headers(nano_ai_headers()?)
        .form(&[("text", text), ("audio_type", "mp3"), ("format", "stream")])
        .send()
        .map_err(|error| format!("无法获取纳米AI TTS 试听音频: {error}"))?;
    let status = response.status();
    let audio = response
        .bytes()
        .map_err(|error| format!("无法读取纳米AI TTS 试听音频: {error}"))?;

    if !status.is_success() {
        return Err(format!("纳米AI TTS 请求失败: HTTP {status}"));
    }

    if audio.is_empty() {
        return Err("纳米AI TTS 未返回试听音频".to_string());
    }

    if looks_like_json_response(&audio) {
        let message = String::from_utf8_lossy(&audio);
        return Err(format!(
            "纳米AI TTS 返回异常: {}",
            truncate_response_text(&message)
        ));
    }

    Ok(audio.to_vec())
}

fn nano_ai_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent(NANO_AI_TTS_USER_AGENT)
        .build()
        .map_err(|error| format!("无法创建纳米AI TTS 客户端: {error}"))
}

fn nano_ai_headers() -> Result<HeaderMap, String> {
    let device = "Web";
    let version = "1.2";
    let timestamp = nano_ai_timestamp();
    let access_token = nano_ai_mid();
    let zm_ua = md5_hex(NANO_AI_TTS_USER_AGENT);
    let zm_token = md5_hex(&format!(
        "{device}{timestamp}{version}{access_token}{zm_ua}"
    ));

    let mut headers = HeaderMap::new();
    headers.insert("device-platform", HeaderValue::from_static(device));
    headers.insert("timestamp", header_value(&timestamp)?);
    headers.insert("access-token", header_value(&access_token)?);
    headers.insert("zm-token", header_value(&zm_token)?);
    headers.insert("zm-ver", HeaderValue::from_static(version));
    headers.insert("zm-ua", header_value(&zm_ua)?);
    headers.insert(USER_AGENT, HeaderValue::from_static(NANO_AI_TTS_USER_AGENT));

    Ok(headers)
}

fn header_value(value: &str) -> Result<HeaderValue, String> {
    HeaderValue::from_str(value).map_err(|error| format!("无法生成请求头: {error}"))
}

fn md5_hex(message: &str) -> String {
    format!("{:x}", md5::compute(message.as_bytes()))
}

fn nano_ai_timestamp() -> String {
    let offset = FixedOffset::east_opt(8 * 60 * 60).expect("valid fixed offset");
    Utc::now()
        .with_timezone(&offset)
        .format("%Y-%m-%dT%H:%M:%S+08:00")
        .to_string()
}

fn nano_ai_mid() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let timestamp = millis as f64 + random_fraction() + random_fraction();
    let raw = format!(
        "{}{}{}",
        nano_ai_hash(NANO_AI_TTS_BASE_URL),
        nano_ai_unique_hash(),
        format!("{timestamp:.6}")
    );

    raw.replace('.', "e").chars().take(32).collect()
}

fn nano_ai_unique_hash() -> u64 {
    let language = "zh-CN";
    let app_name = "chrome";
    let version = "1";
    let platform = "Win32";
    let width = 1920;
    let height = 1080;
    let color_depth = 24;
    let referrer = "https://bot.n.cn/chat";
    let mut value = format!(
        "{app_name}{version}.0{language}{platform}{NANO_AI_TTS_USER_AGENT}{width}x{height}{color_depth}{referrer}"
    );
    let mut length = value.chars().count() as u64;
    let mut index = 1_u64;

    while index != 0 {
        value.push_str(&(index ^ length).to_string());
        index -= 1;
        length += 1;
    }

    (random_u31() ^ nano_ai_hash(&value)) * 2_147_483_647
}

fn nano_ai_hash(value: &str) -> u64 {
    const HASH_MASK_1: u64 = 268_435_455;
    const HASH_MASK_2: u64 = 266_338_304;

    let mut result = 0_u64;
    for character in value.chars().rev() {
        let code = character as u64;
        result = ((result << 6) & HASH_MASK_1) + code + (code << 14);
        let overflow = result & HASH_MASK_2;
        if overflow != 0 {
            result ^= overflow >> 21;
        }
    }

    result
}

fn random_u31() -> u64 {
    (Uuid::new_v4().as_u128() % 2_147_483_648) as u64
}

fn random_fraction() -> f64 {
    (Uuid::new_v4().as_u128() % 1_000_000) as f64 / 1_000_000.0
}

fn looks_like_json_response(data: &[u8]) -> bool {
    data.iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
        .is_some_and(|byte| matches!(byte, b'{' | b'['))
}

fn truncate_response_text(text: &str) -> String {
    const MAX_LEN: usize = 160;
    let value = text.trim();
    if value.chars().count() <= MAX_LEN {
        return value.to_string();
    }

    format!("{}...", value.chars().take(MAX_LEN).collect::<String>())
}

fn synthesize_edge_tts_audio(model_key: &str, text: &str) -> Result<Vec<u8>, String> {
    let connection_id = connect_id();
    let request_url = format!(
        "wss://{EDGE_TTS_BASE_URL}/edge/v1?TrustedClientToken={EDGE_TTS_TRUSTED_CLIENT_TOKEN}&ConnectionId={connection_id}&Sec-MS-GEC={}&Sec-MS-GEC-Version={EDGE_TTS_SEC_MS_GEC_VERSION}",
        generate_sec_ms_gec()?
    );
    let mut request = request_url
        .into_client_request()
        .map_err(|error| format!("无法创建 EDGE-TTS WebSocket 请求: {error}"))?;

    {
        let headers = request.headers_mut();
        headers.insert("Pragma", "no-cache".parse().unwrap());
        headers.insert("Cache-Control", "no-cache".parse().unwrap());
        headers.insert(
            "Origin",
            "chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold"
                .parse()
                .unwrap(),
        );
        headers.insert("Sec-WebSocket-Version", "13".parse().unwrap());
        headers.insert("User-Agent", edge_user_agent().parse().unwrap());
        headers.insert(
            "Accept-Encoding",
            "gzip, deflate, br, zstd".parse().unwrap(),
        );
        headers.insert("Accept-Language", "en-US,en;q=0.9".parse().unwrap());
        headers.insert(
            "Cookie",
            format!("muid={};", generate_muid()).parse().unwrap(),
        );
    }

    let (mut socket, _) =
        connect(request).map_err(|error| format!("无法连接 EDGE-TTS 试听服务: {error}"))?;
    let timestamp = edge_date_string();

    socket
        .send(Message::Text(edge_speech_config_message().into()))
        .map_err(|error| format!("无法发送 EDGE-TTS 音频配置: {error}"))?;
    socket
        .send(Message::Text(
            edge_ssml_message(model_key, text, &timestamp).into(),
        ))
        .map_err(|error| format!("无法发送 EDGE-TTS 试听文本: {error}"))?;

    let mut audio = Vec::new();

    loop {
        let message = socket
            .read()
            .map_err(|error| format!("读取 EDGE-TTS 试听音频失败: {error}"))?;

        match message {
            Message::Binary(data) => {
                let chunk = parse_edge_audio_message(&data)?;
                audio.extend_from_slice(chunk);
            }
            Message::Text(text) => {
                if is_edge_turn_end(text.as_ref())? {
                    break;
                }
            }
            Message::Close(_) => break,
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
        }
    }

    if audio.is_empty() {
        return Err("EDGE-TTS 未返回试听音频".to_string());
    }

    Ok(audio)
}

fn edge_speech_config_message() -> String {
    format!(
        "X-Timestamp:{}\r\nContent-Type:application/json; charset=utf-8\r\nPath:speech.config\r\n\r\n{{\"context\":{{\"synthesis\":{{\"audio\":{{\"metadataoptions\":{{\"sentenceBoundaryEnabled\":\"true\",\"wordBoundaryEnabled\":\"false\"}},\"outputFormat\":\"audio-24khz-48kbitrate-mono-mp3\"}}}}}}}}\r\n",
        edge_date_string()
    )
}

fn edge_ssml_message(model_key: &str, text: &str, timestamp: &str) -> String {
    let cleaned_text = remove_incompatible_characters(text);
    let escaped_text = html_escape::encode_text(&cleaned_text);
    let ssml = format!(
        "<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='en-US'><voice name='{model_key}'><prosody pitch='+0Hz' rate='+0%' volume='+0%'>{escaped_text}</prosody></voice></speak>"
    );

    format!(
        "X-RequestId:{}\r\nContent-Type:application/ssml+xml\r\nX-Timestamp:{timestamp}Z\r\nPath:ssml\r\n\r\n{ssml}",
        connect_id()
    )
}

fn parse_edge_audio_message(data: &[u8]) -> Result<&[u8], String> {
    if data.len() < 2 {
        return Err("EDGE-TTS 音频响应缺少头部长度".to_string());
    }

    let header_length = u16::from_be_bytes([data[0], data[1]]) as usize;
    if header_length > data.len().saturating_sub(2) {
        return Err("EDGE-TTS 音频响应头部异常".to_string());
    }

    let header_start = 2;
    let header_end = header_start + header_length;
    let audio_start = header_end;
    let headers = parse_edge_headers(&data[header_start..header_end])?;

    if headers
        .iter()
        .any(|(key, value)| key == "Path" && value == "audio")
    {
        if audio_start > data.len() {
            return Ok(&[]);
        }

        return Ok(&data[audio_start..]);
    }

    Ok(&[])
}

fn is_edge_turn_end(data: &str) -> Result<bool, String> {
    let header_end = data
        .find("\r\n\r\n")
        .ok_or_else(|| "EDGE-TTS 文本响应格式异常".to_string())?;
    let headers = parse_edge_headers(data[..header_end].as_bytes())?;

    Ok(headers
        .iter()
        .any(|(key, value)| key == "Path" && value == "turn.end"))
}

fn parse_edge_headers(data: &[u8]) -> Result<Vec<(String, String)>, String> {
    let text = String::from_utf8_lossy(data);
    let mut headers = Vec::new();

    for line in text.split("\r\n").filter(|line| !line.is_empty()) {
        let Some((key, value)) = line.split_once(':') else {
            return Err("EDGE-TTS 响应头格式异常".to_string());
        };
        headers.push((key.to_string(), value.to_string()));
    }

    Ok(headers)
}

fn generate_sec_ms_gec() -> Result<String, String> {
    let unix_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("系统时间异常，无法生成 EDGE-TTS 请求签名: {error}"))?
        .as_secs();
    let windows_seconds = unix_seconds + WINDOWS_EPOCH_SECONDS;
    let rounded_seconds = windows_seconds - (windows_seconds % 300);
    let ticks = rounded_seconds * 10_000_000;
    let mut hasher = Sha256::new();
    hasher.update(format!("{ticks}{EDGE_TTS_TRUSTED_CLIENT_TOKEN}").as_bytes());

    Ok(format!("{:X}", hasher.finalize()))
}

fn edge_voice_headers() -> reqwest::header::HeaderMap {
    use reqwest::header::{HeaderMap, HeaderValue};

    let mut headers = HeaderMap::new();
    headers.insert(
        "Authority",
        HeaderValue::from_static("speech.platform.bing.com"),
    );
    headers.insert(
        "Sec-CH-UA",
        HeaderValue::from_str(&format!(
            "\" Not;A Brand\";v=\"99\", \"Microsoft Edge\";v=\"{EDGE_TTS_CHROMIUM_MAJOR_VERSION}\", \"Chromium\";v=\"{EDGE_TTS_CHROMIUM_MAJOR_VERSION}\""
        ))
        .unwrap(),
    );
    headers.insert("Sec-CH-UA-Mobile", HeaderValue::from_static("?0"));
    headers.insert("Accept", HeaderValue::from_static("*/*"));
    headers.insert("Sec-Fetch-Site", HeaderValue::from_static("none"));
    headers.insert("Sec-Fetch-Mode", HeaderValue::from_static("cors"));
    headers.insert("Sec-Fetch-Dest", HeaderValue::from_static("empty"));
    headers.insert(
        "User-Agent",
        HeaderValue::from_str(&edge_user_agent()).unwrap(),
    );
    headers.insert("Accept-Encoding", HeaderValue::from_static("identity"));
    headers.insert(
        "Accept-Language",
        HeaderValue::from_static("en-US,en;q=0.9"),
    );
    headers.insert(
        "Cookie",
        HeaderValue::from_str(&format!("muid={};", generate_muid())).unwrap(),
    );

    headers
}

fn edge_user_agent() -> String {
    format!(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{EDGE_TTS_CHROMIUM_MAJOR_VERSION}.0.0.0 Safari/537.36 Edg/{EDGE_TTS_CHROMIUM_MAJOR_VERSION}.0.0.0"
    )
}

fn edge_date_string() -> String {
    Utc::now()
        .format("%a %b %d %Y %H:%M:%S GMT+0000 (Coordinated Universal Time)")
        .to_string()
}

fn connect_id() -> String {
    Uuid::new_v4().simple().to_string()
}

fn generate_muid() -> String {
    Uuid::new_v4().simple().to_string().to_uppercase()
}

fn remove_incompatible_characters(text: &str) -> String {
    text.chars()
        .map(|character| {
            let code = character as u32;
            if (0..=8).contains(&code) || (11..=12).contains(&code) || (14..=31).contains(&code) {
                ' '
            } else {
                character
            }
        })
        .collect()
}

fn preview_text_for_voice(model_key: &str, locale: Option<&str>) -> &'static str {
    let language = locale
        .and_then(|value| value.split('-').next())
        .filter(|value| !value.is_empty())
        .or_else(|| model_key.split('-').next())
        .unwrap_or("en");

    match language {
        "ar" => "مرحبا، هذه معاينة للصوت.",
        "da" => "Hej, dette er en stemmeforhåndsvisning.",
        "de" => "Hallo, dies ist eine Stimmprobe.",
        "el" => "Γεια σας, αυτή είναι μια προεπισκόπηση φωνής.",
        "en" => "Hello, this is a voice preview.",
        "es" => "Hola, esta es una vista previa de voz.",
        "fi" => "Hei, tämä on äänen esikatselu.",
        "fr" => "Bonjour, ceci est un aperçu de la voix.",
        "he" => "שלום, זו תצוגה מקדימה של הקול.",
        "hi" => "नमस्ते, यह आवाज़ का पूर्वावलोकन है।",
        "id" => "Halo, ini adalah pratinjau suara.",
        "it" => "Ciao, questa è un'anteprima della voce.",
        "ja" => "こんにちは、これは音声プレビューです。",
        "ko" => "안녕하세요. 음성 미리 듣기입니다.",
        "nb" | "nn" | "no" => "Hei, dette er en forhåndsvisning av stemmen.",
        "nl" => "Hallo, dit is een stemvoorbeeld.",
        "pl" => "Cześć, to jest podgląd głosu.",
        "pt" => "Olá, esta é uma prévia de voz.",
        "ru" => "Здравствуйте, это предварительное прослушивание голоса.",
        "sv" => "Hej, det här är en röstförhandsvisning.",
        "th" => "สวัสดี นี่คือตัวอย่างเสียง",
        "tr" => "Merhaba, bu bir ses önizlemesidir.",
        "vi" => "Xin chào, đây là bản nghe thử giọng nói.",
        "zh" => "你好，这是一段配音试听。",
        _ => "1 2 3 4 5.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_srt_for_dubbing_preprocess() {
        let srt = "1\n00:00:01,000 --> 00:00:02,500\nHello world\n\n2\n00:00:03,000 --> 00:00:04,000\nNext\n";
        let segments = parse_srt(srt).unwrap();

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].start_time, 1000);
        assert_eq!(segments[0].end_time, 2500);
        assert_eq!(segments[0].text, "Hello world");
    }

    #[test]
    fn parses_ass_for_dubbing_preprocess() {
        let ass = "[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:02.50,Default,,0,0,0,,{\\i1}Hello\\Nworld\n";
        let segments = parse_ass(ass).unwrap();

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].start_time, 1000);
        assert_eq!(segments[0].end_time, 2500);
        assert_eq!(segments[0].text, "Hello\nworld");
    }

    #[test]
    fn repairs_invalid_subtitle_timing_for_dubbing_preprocess() {
        let segments = parse_srt("1\n00:00:02,000 --> 00:00:01,000\nBackwards\n").unwrap();
        let mut warnings = Vec::new();
        let normalized = normalize_dubbing_segments(segments, &mut warnings).unwrap();

        assert_eq!(normalized.len(), 1);
        assert_eq!(normalized[0].start_time, 2000);
        assert_eq!(normalized[0].end_time, 2100);
        assert_eq!(normalized[0].text, "Backwards.");
        assert!(warnings[0].contains("时间轴无效"));
    }

    #[test]
    fn pair_key_is_stable_for_same_paths() {
        let first = dubbing_pair_key(
            Path::new("C:/Videos/intro.mp4"),
            Path::new("C:/Videos/intro.srt"),
        );
        let second = dubbing_pair_key(
            Path::new("C:/Videos/intro.mp4"),
            Path::new("C:/Videos/intro.srt"),
        );
        let third = dubbing_pair_key(
            Path::new("C:/Videos/intro.mp4"),
            Path::new("C:/Videos/other.srt"),
        );

        assert_eq!(first, second);
        assert_ne!(first, third);
    }
}
