use crate::ai::AiService;
use crate::app_log::{AppLogger, LogSession};
use crate::app_paths;
use crate::content_copy::{
    generate_content_copy_record_with_checkpoint, GenerateContentCopyRequest,
};
use crate::dubbing::{
    prepare_dubbing_material_internal, start_dubbing_task_internal, DubbingTaskArtifact,
    DubbingTaskOptions, DubbingTaskSnapshot, PrepareDubbingMaterialRequest,
    StartDubbingTaskRequest, DUBBING_FINAL_DUBBED_VIDEO_ARTIFACT, DUBBING_FINAL_SUBTITLE_ARTIFACT,
};
use crate::home_tasks::{
    download_home_video_task_video_internal, read_home_video_task_by_id, HomeVideoDownload,
    HomeVideoSubtitle, HomeVideoTask, HomeVideoTaskRequest,
};
use crate::settings::{AppSettings, SettingsStore};
use crate::subtitle_ai::{
    correct_subtitles_with_downloaded_reference, SubtitleReferenceCorrectionReference,
};
use crate::subtitle_alignment::{match_downloaded_subtitle_references, reference_match_version};
use crate::subtitle_translation::{
    load_subtitle_segments, run_subtitle_translation_workflow_with_sink,
    SubtitleTranslationProgress, SubtitleTranslationProgressSink, SubtitleTranslationRequest,
    SubtitleTranslationResult,
};
use crate::transcription::{
    normalize_subtitle_format, run_transcription_workflow_with_checkpoint,
    serialize_segments_for_export, write_text_file, TranscriptionProgress,
    TranscriptionProgressSink, TranscriptionRequest,
};
use crate::workbench_checkpoint::{checkpoint_hash, WorkbenchCheckpointContext};
use chrono::Utc;
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

const HOME_WORKBENCH_PROGRESS_EVENT: &str = "home-workbench-progress";

const WORKBENCH_STATUS_IDLE: &str = "idle";
const WORKBENCH_STATUS_RUNNING: &str = "running";
const WORKBENCH_STATUS_DONE: &str = "done";
const WORKBENCH_STATUS_FAILED: &str = "failed";
const WORKBENCH_STATUS_INTERRUPTED: &str = "interrupted";

const STAGE_DOWNLOAD_VIDEO: &str = "download-video";
const STAGE_PREPARE_SUBTITLE: &str = "prepare-subtitle";
const STAGE_TRANSLATION: &str = "translation";
const STAGE_DUBBING: &str = "dubbing";
const STAGE_CONTENT_COPY: &str = "content-copy";
const STAGE_EXPORT: &str = "export";

const STAGE_STATUS_PENDING: &str = "pending";
const STAGE_STATUS_ACTIVE: &str = "active";
const STAGE_STATUS_DONE: &str = "done";
const STAGE_STATUS_SKIPPED: &str = "skipped";
const STAGE_STATUS_FAILED: &str = "failed";
const STAGE_STATUS_INTERRUPTED: &str = "interrupted";

const SUBTITLE_SOURCE_TRANSCRIBE: &str = "transcribe";
const SUBTITLE_SOURCE_DOWNLOADED: &str = "downloaded";

const ARTIFACT_TRANSCRIPTION_SUBTITLE: &str = "transcription-subtitle";
const ARTIFACT_SELECTED_SUBTITLE: &str = "selected-subtitle";
const ARTIFACT_ALIGNED_SELECTED_SUBTITLE: &str = "aligned-selected-subtitle";
const ARTIFACT_REFERENCE_CORRECTED_SUBTITLE: &str = "reference-corrected-subtitle";
const ARTIFACT_SOURCE_VIDEO: &str = "source-video";
const ARTIFACT_TRANSLATED_SUBTITLE: &str = "translated-subtitle";
const ARTIFACT_DUBBED_VIDEO: &str = "dubbed-video";
const ARTIFACT_DUBBED_SUBTITLE: &str = "dubbed-subtitle";
const ARTIFACT_EXPORTED_VIDEO: &str = "exported-video";
const ARTIFACT_EXPORTED_SUBTITLE: &str = "exported-subtitle";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HomeWorkbenchOptions {
    pub subtitle_source: String,
    #[serde(default)]
    pub subtitle_id: String,
    pub translation_enabled: bool,
    pub dubbing_enabled: bool,
    #[serde(default)]
    pub export_dir: String,
    pub transcription_model: String,
    pub source_language: String,
    pub transcription_format: String,
    pub is_smart_segmentation_enabled: bool,
    pub is_subtitle_correction_enabled: bool,
    pub translation_format: String,
    pub translation_service: String,
    pub needs_reflection_translation: bool,
    pub translation_batch_size: u32,
    pub translation_thread_count: u32,
    pub video_content_type: String,
    pub output_mode: String,
    pub is_subtitle_translation_enabled: bool,
    pub is_ai_subtitle_review_enabled: bool,
    pub ai_subtitle_review_mode: String,
    pub target_language: String,
    pub dubbing_tts_interval_ms: u32,
    pub dubbing_reference_audio_source: String,
    #[serde(default)]
    pub dubbing_custom_reference_audio_path: String,
    pub dubbing_is_background_music_enabled: bool,
    pub dubbing_background_music_volume: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeWorkbenchStage {
    pub key: String,
    pub label: String,
    pub progress: u8,
    pub status: String,
    pub message: String,
    #[serde(default)]
    pub snapshot: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeWorkbenchArtifact {
    pub kind: String,
    pub path: String,
    pub file_size: i64,
    pub metadata: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeWorkbenchSnapshot {
    pub task_id: String,
    pub status: String,
    pub current_stage: String,
    pub progress: u8,
    pub message: String,
    pub stages: Vec<HomeWorkbenchStage>,
    pub options: HomeWorkbenchOptions,
    pub artifacts: Vec<HomeWorkbenchArtifact>,
    pub warnings: Vec<String>,
    pub error_message: String,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeWorkbenchTaskRequest {
    pub task_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveHomeWorkbenchOptionsRequest {
    pub task_id: String,
    pub options: HomeWorkbenchOptions,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartHomeWorkbenchRequest {
    pub task_id: String,
    pub options: HomeWorkbenchOptions,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddHomeWorkbenchSubtitleInputRequest {
    pub task_id: String,
    pub subtitle_id: String,
}

struct ExportedFinalPaths {
    video_path: PathBuf,
    subtitle_path: PathBuf,
}

pub struct HomeWorkbenchRuntime {
    active_task_ids: Mutex<HashSet<String>>,
}

impl HomeWorkbenchRuntime {
    pub fn new() -> Self {
        Self {
            active_task_ids: Mutex::new(HashSet::new()),
        }
    }

    fn start(&self, task_id: &str) -> Result<HomeWorkbenchRunGuard<'_>, String> {
        let mut active = self
            .active_task_ids
            .lock()
            .map_err(|error| format!("工作台运行状态锁定失败: {error}"))?;
        if active.contains(task_id) {
            return Err("工作台任务正在执行中".to_string());
        }
        active.insert(task_id.to_string());
        Ok(HomeWorkbenchRunGuard {
            runtime: self,
            task_id: task_id.to_string(),
        })
    }

    fn is_active(&self, task_id: &str) -> Result<bool, String> {
        self.active_task_ids
            .lock()
            .map(|active| active.contains(task_id))
            .map_err(|error| format!("工作台运行状态锁定失败: {error}"))
    }
}

struct HomeWorkbenchRunGuard<'a> {
    runtime: &'a HomeWorkbenchRuntime,
    task_id: String,
}

impl Drop for HomeWorkbenchRunGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut active) = self.runtime.active_task_ids.lock() {
            active.remove(&self.task_id);
        }
    }
}

#[tauri::command]
pub fn get_home_workbench(
    store: tauri::State<'_, SettingsStore>,
    runtime: tauri::State<'_, HomeWorkbenchRuntime>,
    request: HomeWorkbenchTaskRequest,
) -> Result<HomeWorkbenchSnapshot, String> {
    let settings = store.load()?;
    let should_interrupt_stale_running = !runtime.is_active(&request.task_id)?;
    store.with_connection(|connection| {
        read_or_create_workbench_snapshot(
            connection,
            &request.task_id,
            &settings,
            should_interrupt_stale_running,
        )
    })
}

#[tauri::command]
pub fn save_home_workbench_options(
    store: tauri::State<'_, SettingsStore>,
    ai_service: tauri::State<'_, AiService>,
    request: SaveHomeWorkbenchOptionsRequest,
) -> Result<HomeWorkbenchSnapshot, String> {
    let mut settings = store.load()?;
    let options = normalize_options(request.options, &settings);
    store.with_connection(|connection| ensure_home_task_exists(connection, &request.task_id))?;
    apply_workbench_options_to_settings(&mut settings, &options);
    store.save(&settings)?;
    ai_service.update_thread_count(settings.translation_thread_count)?;

    store.with_connection(|connection| {
        let now = Utc::now().to_rfc3339();
        let existing = read_workbench_record(connection, &request.task_id)?.unwrap_or_else(|| {
            default_workbench_record(&request.task_id, default_workbench_options(&settings), &now)
        });
        upsert_workbench_record(
            connection,
            &request.task_id,
            &existing.status,
            &existing.current_stage,
            existing.progress,
            &existing.message,
            &existing.stages,
            &options,
            &existing.warnings,
            &existing.error_message,
            existing.revision + 1,
            existing.created_at.as_str(),
            &now,
        )?;
        read_or_create_workbench_snapshot(connection, &request.task_id, &settings, false)
    })
}

#[tauri::command]
pub fn add_home_workbench_video_input(
    app: AppHandle,
    request: HomeWorkbenchTaskRequest,
) -> Result<HomeWorkbenchSnapshot, String> {
    let store = app.state::<SettingsStore>();
    let settings = store.load()?;
    let task = store
        .with_connection(|connection| read_home_video_task_by_id(connection, &request.task_id))?;
    let video = task
        .downloaded_video
        .filter(|video| Path::new(&video.file_path).is_file())
        .ok_or_else(|| "请先下载视频文件".to_string())?;

    upsert_artifact_from_path(
        &store,
        &request.task_id,
        ARTIFACT_SOURCE_VIDEO,
        Path::new(&video.file_path),
        json!({
            "source": "downloaded-video",
            "videoId": &video.id,
            "format": &video.format,
            "fileName": &video.file_name,
        }),
    )?;
    set_stage_done_with_snapshot(
        &store,
        &app,
        &request.task_id,
        STAGE_DOWNLOAD_VIDEO,
        "视频已加入工作台",
        WORKBENCH_STATUS_IDLE,
        json!({
            "mode": "registered",
            "path": &video.file_path,
            "fileSize": video.file_size,
            "format": &video.format,
            "message": "视频已加入工作台",
        }),
    )?;

    store.with_connection(|connection| {
        read_or_create_workbench_snapshot(connection, &request.task_id, &settings, false)
    })
}

#[tauri::command]
pub fn remove_home_workbench_video_input(
    app: AppHandle,
    request: HomeWorkbenchTaskRequest,
) -> Result<HomeWorkbenchSnapshot, String> {
    let store = app.state::<SettingsStore>();
    let settings = store.load()?;
    let snapshot = store.with_connection(|connection| {
        ensure_home_task_exists(connection, &request.task_id)?;
        let now = Utc::now().to_rfc3339();
        let mut record =
            read_workbench_record(connection, &request.task_id)?.unwrap_or_else(|| {
                default_workbench_record(
                    &request.task_id,
                    default_workbench_options(&settings),
                    &now,
                )
            });
        if record.status == WORKBENCH_STATUS_RUNNING {
            return Err("工作台执行中，无法移除视频".to_string());
        }

        delete_workbench_artifact(connection, &request.task_id, ARTIFACT_SOURCE_VIDEO)?;
        delete_downstream_workbench_artifacts(connection, &request.task_id)?;
        reset_stage_to_pending(&mut record, STAGE_DOWNLOAD_VIDEO, "等待下载视频");
        reset_stage_to_pending(&mut record, STAGE_PREPARE_SUBTITLE, "等待准备字幕");
        record.status = WORKBENCH_STATUS_IDLE.to_string();
        record.current_stage.clear();
        record.progress = overall_progress(&record.stages);
        record.message = "已移除工作台视频".to_string();
        record.error_message.clear();
        upsert_workbench_record(
            connection,
            &request.task_id,
            &record.status,
            &record.current_stage,
            record.progress,
            &record.message,
            &record.stages,
            &record.options,
            &record.warnings,
            &record.error_message,
            record.revision + 1,
            &record.created_at,
            &now,
        )?;
        read_or_create_workbench_snapshot(connection, &request.task_id, &settings, false)
    })?;
    emit_workbench_progress(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn add_home_workbench_subtitle_input(
    app: AppHandle,
    request: AddHomeWorkbenchSubtitleInputRequest,
) -> Result<HomeWorkbenchSnapshot, String> {
    let store = app.state::<SettingsStore>();
    let settings = store.load()?;
    let subtitle = store.with_connection(|connection| {
        let task = read_home_video_task_by_id(connection, &request.task_id)?;
        task.downloaded_subtitles
            .into_iter()
            .find(|subtitle| {
                subtitle.id == request.subtitle_id && Path::new(&subtitle.file_path).is_file()
            })
            .ok_or_else(|| "请先下载字幕文件".to_string())
    })?;

    upsert_artifact_from_path(
        &store,
        &request.task_id,
        ARTIFACT_SELECTED_SUBTITLE,
        Path::new(&subtitle.file_path),
        json!({
            "subtitleId": &subtitle.id,
            "language": &subtitle.language,
            "languageName": &subtitle.language_name,
            "sourceKind": &subtitle.source_kind,
            "format": &subtitle.format,
        }),
    )?;

    store.with_connection(|connection| {
        let now = Utc::now().to_rfc3339();
        let mut record =
            read_workbench_record(connection, &request.task_id)?.unwrap_or_else(|| {
                default_workbench_record(
                    &request.task_id,
                    default_workbench_options(&settings),
                    &now,
                )
            });
        record.options.subtitle_source = SUBTITLE_SOURCE_DOWNLOADED.to_string();
        record.options.subtitle_id = subtitle.id.clone();
        delete_downstream_workbench_artifacts(connection, &request.task_id)?;
        reset_stage_to_pending(
            &mut record,
            STAGE_PREPARE_SUBTITLE,
            "已添加下载字幕，执行时将作为参考校正",
        );
        reset_stage_to_pending(&mut record, STAGE_TRANSLATION, "等待翻译");
        reset_stage_to_pending(&mut record, STAGE_DUBBING, "等待配音");
        reset_stage_to_pending(&mut record, STAGE_CONTENT_COPY, "等待生成文案");
        reset_stage_to_pending(&mut record, STAGE_EXPORT, "等待导出");
        if let Some(stage) = record
            .stages
            .iter_mut()
            .find(|stage| stage.key == STAGE_PREPARE_SUBTITLE)
        {
            stage.snapshot = json!({
                "mode": "downloaded",
                "path": &subtitle.file_path,
                "fileSize": subtitle.file_size,
                "format": &subtitle.format,
                "language": &subtitle.language,
                "languageName": &subtitle.language_name,
                "sourceKind": &subtitle.source_kind,
                "message": "已添加下载字幕，执行时将作为参考校正",
            });
        }
        record.progress = overall_progress(&record.stages);
        record.message = "已添加下载字幕，执行时将作为参考校正".to_string();
        record.current_stage = STAGE_PREPARE_SUBTITLE.to_string();
        upsert_workbench_record(
            connection,
            &request.task_id,
            &record.status,
            &record.current_stage,
            record.progress,
            &record.message,
            &record.stages,
            &record.options,
            &record.warnings,
            &record.error_message,
            record.revision + 1,
            &record.created_at,
            &now,
        )
    })?;

    let snapshot = store.with_connection(|connection| {
        read_or_create_workbench_snapshot(connection, &request.task_id, &settings, false)
    })?;
    emit_workbench_progress(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn remove_home_workbench_subtitle_input(
    app: AppHandle,
    request: AddHomeWorkbenchSubtitleInputRequest,
) -> Result<HomeWorkbenchSnapshot, String> {
    let store = app.state::<SettingsStore>();
    let settings = store.load()?;
    let snapshot = store.with_connection(|connection| {
        ensure_home_task_exists(connection, &request.task_id)?;
        let now = Utc::now().to_rfc3339();
        let mut record =
            read_workbench_record(connection, &request.task_id)?.unwrap_or_else(|| {
                default_workbench_record(
                    &request.task_id,
                    default_workbench_options(&settings),
                    &now,
                )
            });
        if record.status == WORKBENCH_STATUS_RUNNING {
            return Err("工作台执行中，无法移除字幕".to_string());
        }

        let selected_artifact =
            read_workbench_artifact(connection, &request.task_id, ARTIFACT_SELECTED_SUBTITLE)?;
        let should_remove = selected_artifact
            .as_ref()
            .map(|artifact| {
                let selected_id = artifact
                    .metadata
                    .get("subtitleId")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                selected_id.is_empty() || selected_id == request.subtitle_id
            })
            .unwrap_or(false);

        if should_remove {
            delete_workbench_artifact(connection, &request.task_id, ARTIFACT_SELECTED_SUBTITLE)?;
            delete_downstream_workbench_artifacts(connection, &request.task_id)?;
            reset_stage_to_pending(&mut record, STAGE_PREPARE_SUBTITLE, "等待准备字幕");
            if record.options.subtitle_source == SUBTITLE_SOURCE_DOWNLOADED
                && (record.options.subtitle_id.is_empty()
                    || record.options.subtitle_id == request.subtitle_id)
            {
                record.options.subtitle_source = SUBTITLE_SOURCE_TRANSCRIBE.to_string();
                record.options.subtitle_id.clear();
            }
            record.status = WORKBENCH_STATUS_IDLE.to_string();
            record.current_stage.clear();
            record.progress = overall_progress(&record.stages);
            record.message = "已移除工作台字幕".to_string();
            record.error_message.clear();
            upsert_workbench_record(
                connection,
                &request.task_id,
                &record.status,
                &record.current_stage,
                record.progress,
                &record.message,
                &record.stages,
                &record.options,
                &record.warnings,
                &record.error_message,
                record.revision + 1,
                &record.created_at,
                &now,
            )?;
        }

        read_or_create_workbench_snapshot(connection, &request.task_id, &settings, false)
    })?;
    emit_workbench_progress(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn start_home_workbench(
    app: AppHandle,
    request: StartHomeWorkbenchRequest,
) -> Result<HomeWorkbenchSnapshot, String> {
    run_home_workbench(app, request).await
}

async fn run_home_workbench(
    app: AppHandle,
    request: StartHomeWorkbenchRequest,
) -> Result<HomeWorkbenchSnapshot, String> {
    let store = app.state::<SettingsStore>();
    let ai_service = app.state::<AiService>();
    let app_logger = app.state::<AppLogger>();
    let runtime = app.state::<HomeWorkbenchRuntime>();
    let log_session = app_logger.start_session("home_workbench")?;
    let mut settings = store.load()?;
    let mut options = normalize_options(request.options, &settings);
    apply_workbench_options_to_settings(&mut settings, &options);
    store.save(&settings)?;
    ai_service.update_thread_count(settings.translation_thread_count)?;
    let task_id = request.task_id;
    let _run_guard = runtime.start(&task_id)?;
    log_session.info(
        "workbench_start",
        "开始执行首页工作台任务",
        json!({
            "taskId": &task_id,
            "subtitleSource": &options.subtitle_source,
            "translationEnabled": options.translation_enabled,
            "dubbingEnabled": options.dubbing_enabled,
        }),
    );
    initialize_run(&store, &task_id, &options)?;

    let run_result = run_home_workbench_inner(
        app.clone(),
        &store,
        &ai_service,
        &app_logger,
        &settings,
        &task_id,
        &mut options,
        &log_session,
    )
    .await;

    match run_result {
        Ok(snapshot) => {
            log_session.info(
                "workbench_completed",
                "首页工作台任务完成",
                json!({
                    "taskId": &task_id,
                    "status": &snapshot.status,
                    "logPath": log_session.path_string(),
                }),
            );
            Ok(snapshot)
        }
        Err(error) => {
            log_session.error(
                "workbench_failed",
                "首页工作台任务失败",
                json!({ "taskId": &task_id, "error": &error }),
            );
            let snapshot = mark_workbench_failed(&store, &task_id, &error)?;
            emit_workbench_progress(&app, &snapshot);
            Err(error)
        }
    }
}

async fn run_home_workbench_inner(
    app: AppHandle,
    store: &SettingsStore,
    ai_service: &AiService,
    app_logger: &AppLogger,
    settings: &AppSettings,
    task_id: &str,
    options: &mut HomeWorkbenchOptions,
    log_session: &LogSession,
) -> Result<HomeWorkbenchSnapshot, String> {
    let video = ensure_workbench_video(app.clone(), store, task_id)?;
    log_session.info(
        "video_ready",
        "工作台视频文件已就绪",
        json!({
            "taskId": task_id,
            "videoPath": &video.file_path,
            "fileSize": video.file_size,
        }),
    );

    set_stage_active(
        store,
        &app,
        task_id,
        STAGE_PREPARE_SUBTITLE,
        "准备字幕文件",
        2,
    )?;
    let subtitle_path = prepare_subtitle(
        app.clone(),
        store,
        ai_service,
        app_logger,
        settings,
        task_id,
        options,
        &video,
    )
    .await?;
    log_session.info(
        "subtitle_ready",
        "工作台字幕文件已就绪",
        json!({ "taskId": task_id, "subtitlePath": subtitle_path.to_string_lossy() }),
    );
    set_stage_done(
        store,
        &app,
        task_id,
        STAGE_PREPARE_SUBTITLE,
        "字幕文件已就绪",
    )?;

    let final_subtitle_path = if options.translation_enabled {
        log_session.info(
            "translation_start",
            "工作台开始翻译字幕",
            json!({ "taskId": task_id, "subtitlePath": subtitle_path.to_string_lossy() }),
        );
        let translated = translate_subtitle(
            app.clone(),
            store,
            ai_service,
            app_logger,
            settings,
            task_id,
            options,
            &subtitle_path,
        )
        .await?;
        set_stage_done(store, &app, task_id, STAGE_TRANSLATION, "翻译完成")?;
        log_session.info(
            "translation_completed",
            "工作台字幕翻译完成",
            json!({ "taskId": task_id, "subtitlePath": translated.to_string_lossy() }),
        );
        translated
    } else {
        set_stage_skipped(store, &app, task_id, STAGE_TRANSLATION, "已跳过翻译")?;
        log_session.info(
            "translation_skipped",
            "工作台已跳过字幕翻译",
            json!({ "taskId": task_id }),
        );
        subtitle_path
    };

    let final_video_path = if options.dubbing_enabled {
        log_session.info(
            "dubbing_start",
            "工作台开始配音",
            json!({ "taskId": task_id }),
        );
        let (dubbed_video, dubbed_subtitle) = run_dubbing(
            app.clone(),
            store,
            task_id,
            options,
            &video,
            &final_subtitle_path,
        )
        .await?;
        upsert_artifact_from_path(
            store,
            task_id,
            ARTIFACT_DUBBED_SUBTITLE,
            &dubbed_subtitle,
            json!({ "source": "dubbing" }),
        )?;
        set_stage_done(store, &app, task_id, STAGE_DUBBING, "配音完成")?;
        let _ = final_subtitle_path;
        log_session.info(
            "dubbing_completed",
            "工作台配音完成",
            json!({
                "taskId": task_id,
                "videoPath": dubbed_video.to_string_lossy(),
                "subtitlePath": dubbed_subtitle.to_string_lossy(),
            }),
        );
        (dubbed_video, dubbed_subtitle)
    } else {
        set_stage_skipped(store, &app, task_id, STAGE_DUBBING, "已跳过配音")?;
        log_session.info(
            "dubbing_skipped",
            "工作台已跳过配音",
            json!({ "taskId": task_id }),
        );
        (PathBuf::from(&video.file_path), final_subtitle_path)
    };

    log_session.info(
        "content_copy_stage_start",
        "工作台开始处理文案生成阶段",
        json!({ "taskId": task_id }),
    );
    generate_workbench_content_copy(
        app.clone(),
        store,
        ai_service,
        app_logger,
        settings,
        task_id,
        &final_video_path.1,
    )
    .await?;
    log_session.info(
        "content_copy_stage_completed",
        "工作台文案生成阶段完成",
        json!({ "taskId": task_id }),
    );

    let exported = if let Some(exported) = reusable_exported_artifacts(store, task_id)? {
        update_stage_snapshot_from_app(
            &app,
            task_id,
            STAGE_EXPORT,
            99,
            "复用已导出产物",
            json!({
                "mode": "export",
                "videoPath": exported.video_path.to_string_lossy(),
                "subtitlePath": exported.subtitle_path.to_string_lossy(),
                "message": "复用已导出产物",
            }),
        )?;
        log_session.info(
            "export_reused",
            "工作台复用已导出产物",
            json!({
                "taskId": task_id,
                "videoPath": exported.video_path.to_string_lossy(),
                "subtitlePath": exported.subtitle_path.to_string_lossy(),
            }),
        );
        exported
    } else {
        set_stage_active(store, &app, task_id, STAGE_EXPORT, "导出最终产物", 5)?;
        log_session.info(
            "export_start",
            "工作台开始导出最终产物",
            json!({ "taskId": task_id, "exportDir": &options.export_dir }),
        );
        let exported = export_final_artifacts(
            store,
            task_id,
            options,
            &final_video_path.0,
            &final_video_path.1,
        )?;
        upsert_artifact_from_path(
            store,
            task_id,
            ARTIFACT_EXPORTED_VIDEO,
            &exported.video_path,
            json!({ "scope": "final", "exportDir": &options.export_dir }),
        )?;
        upsert_artifact_from_path(
            store,
            task_id,
            ARTIFACT_EXPORTED_SUBTITLE,
            &exported.subtitle_path,
            json!({ "scope": "final", "exportDir": &options.export_dir }),
        )?;
        log_session.info(
            "export_completed",
            "工作台最终产物导出完成",
            json!({
                "taskId": task_id,
                "videoPath": exported.video_path.to_string_lossy(),
                "subtitlePath": exported.subtitle_path.to_string_lossy(),
            }),
        );
        exported
    };
    set_stage_done_with_snapshot(
        store,
        &app,
        task_id,
        STAGE_EXPORT,
        "导出完成",
        WORKBENCH_STATUS_RUNNING,
        json!({
            "mode": "export",
            "videoPath": exported.video_path.to_string_lossy(),
            "subtitlePath": exported.subtitle_path.to_string_lossy(),
            "message": "导出完成",
        }),
    )?;

    mark_workbench_done(store, &app, task_id)
}

fn ensure_video_downloaded(
    app: AppHandle,
    task: HomeVideoTask,
) -> Result<HomeVideoDownload, String> {
    if let Some(video) = task
        .downloaded_video
        .filter(|video| Path::new(&video.file_path).is_file())
    {
        // 视频已存在，发送进度完成事件以同步工作台UI
        let _ = app.emit(
            "home-video-download-progress",
            serde_json::json!({
                "taskId": task.id,
                "kind": "video",
                "key": "video",
                "progress": 100,
                "status": "done",
                "message": "视频文件已就绪",
                "downloadedBytes": serde_json::Value::Null,
                "totalBytes": serde_json::Value::Null,
                "language": serde_json::Value::Null,
                "sourceKind": serde_json::Value::Null,
            }),
        );
        return Ok(video);
    }

    let updated_task = download_home_video_task_video_internal(
        app,
        HomeVideoTaskRequest {
            task_id: task.id.clone(),
        },
    )?;
    updated_task
        .downloaded_video
        .filter(|video| Path::new(&video.file_path).is_file())
        .ok_or_else(|| "视频下载完成但未找到视频文件".to_string())
}

fn ensure_workbench_video(
    app: AppHandle,
    store: &SettingsStore,
    task_id: &str,
) -> Result<HomeVideoDownload, String> {
    set_stage_active(
        store,
        &app,
        task_id,
        STAGE_DOWNLOAD_VIDEO,
        "准备视频文件",
        2,
    )?;

    if let Some(artifact) = workbench_artifact_file(store, task_id, ARTIFACT_SOURCE_VIDEO)? {
        let video = artifact_video_download(task_id, &artifact);
        set_stage_done_with_snapshot(
            store,
            &app,
            task_id,
            STAGE_DOWNLOAD_VIDEO,
            "视频文件已就绪",
            WORKBENCH_STATUS_RUNNING,
            json!({
                "mode": "registered",
                "path": video.file_path,
                "fileSize": video.file_size,
                "format": video.format,
                "message": "复用已加入工作台的视频",
            }),
        )?;
        return Ok(video);
    }

    let task =
        store.with_connection(|connection| read_home_video_task_by_id(connection, task_id))?;
    let video = ensure_video_downloaded(app.clone(), task)?;
    upsert_artifact_from_path(
        store,
        task_id,
        ARTIFACT_SOURCE_VIDEO,
        Path::new(&video.file_path),
        json!({
            "source": "downloaded-video",
            "videoId": &video.id,
            "format": &video.format,
            "fileName": &video.file_name,
        }),
    )?;
    set_stage_done_with_snapshot(
        store,
        &app,
        task_id,
        STAGE_DOWNLOAD_VIDEO,
        "视频文件已就绪",
        WORKBENCH_STATUS_RUNNING,
        json!({
            "mode": "downloaded",
            "path": &video.file_path,
            "fileSize": video.file_size,
            "format": &video.format,
            "message": "视频文件已就绪",
        }),
    )?;
    Ok(video)
}

async fn prepare_subtitle(
    app: AppHandle,
    store: &SettingsStore,
    ai_service: &AiService,
    app_logger: &AppLogger,
    settings: &AppSettings,
    task_id: &str,
    options: &HomeWorkbenchOptions,
    video: &HomeVideoDownload,
) -> Result<PathBuf, String> {
    if let Some(artifact) = reusable_prepared_downloaded_subtitle(store, task_id, options, video)? {
        update_stage_snapshot_from_app(
            &app,
            task_id,
            STAGE_PREPARE_SUBTITLE,
            99,
            "复用已参考校正字幕",
            json!({
                "mode": "downloaded-reference",
                "path": artifact.path,
                "fileSize": artifact.file_size,
                "metadata": artifact.metadata,
                "referenceCorrection": artifact.metadata.get("referenceCorrection").cloned().unwrap_or_else(|| json!({})),
                "message": "复用已参考校正字幕",
            }),
        )?;
        return Ok(artifact_path_value(&artifact));
    }

    if options.subtitle_source == SUBTITLE_SOURCE_DOWNLOADED {
        if let Some(subtitle) = selected_downloaded_subtitle(store, task_id, &options.subtitle_id)?
        {
            return prepare_reference_corrected_downloaded_subtitle(
                app, store, ai_service, app_logger, settings, task_id, options, video, subtitle,
            )
            .await;
        }
    }

    if let Some(artifact) =
        workbench_artifact_file(store, task_id, ARTIFACT_TRANSCRIPTION_SUBTITLE)?
    {
        update_stage_snapshot_from_app(
            &app,
            task_id,
            STAGE_PREPARE_SUBTITLE,
            99,
            "复用已生成字幕",
            json!({
                "mode": "transcribe",
                "path": artifact.path,
                "fileSize": artifact.file_size,
                "metadata": artifact.metadata,
                "message": "复用已生成字幕",
            }),
        )?;
        return Ok(artifact_path_value(&artifact));
    }

    let mut run_settings = settings.clone();
    apply_transcription_options(&mut run_settings, options);
    let progress_sink = workbench_transcription_progress_sink(app.clone(), task_id.to_string());
    let checkpoint_context = WorkbenchCheckpointContext::new(
        task_id,
        "prepare-subtitle:transcribe",
        checkpoint_hash(&build_transcription_checkpoint_input(
            video,
            options,
            &run_settings,
        )),
    );
    let result = run_transcription_workflow_with_checkpoint(
        app,
        ai_service,
        app_logger,
        Some(store),
        TranscriptionRequest {
            file_path: video.file_path.clone(),
            model: options.transcription_model.clone(),
            source_language: options.source_language.clone(),
            output_format: options.transcription_format.clone(),
            client_run_id: task_id.to_string(),
            progress_source: "home-workbench".to_string(),
        },
        run_settings,
        Some(progress_sink),
        store,
        checkpoint_context,
    )
    .await?;
    save_workbench_subtitle(
        store,
        task_id,
        ARTIFACT_TRANSCRIPTION_SUBTITLE,
        &result.output_path,
        &result.subtitle_text,
        json!({
            "format": result.output_format,
            "segmentCount": result.segments.len(),
            "logPath": result.log_path,
            "warnings": result.warnings,
        }),
    )
}

#[allow(clippy::too_many_arguments)]
async fn prepare_reference_corrected_downloaded_subtitle(
    app: AppHandle,
    store: &SettingsStore,
    ai_service: &AiService,
    app_logger: &AppLogger,
    settings: &AppSettings,
    task_id: &str,
    options: &HomeWorkbenchOptions,
    video: &HomeVideoDownload,
    subtitle: HomeVideoSubtitle,
) -> Result<PathBuf, String> {
    let downloaded_segments = load_subtitle_segments(Path::new(&subtitle.file_path))?;
    let reference_correction_state = Arc::new(Mutex::new(json!({
        "status": "active",
        "message": "等待语音转录"
    })));
    update_stage_snapshot_from_app(
        &app,
        task_id,
        STAGE_PREPARE_SUBTITLE,
        5,
        "读取下载字幕",
        json!({
            "mode": "downloaded-reference",
            "message": "读取下载字幕",
            "referenceCorrection": {
                "status": "active",
                "message": "等待语音转录"
            },
        }),
    )?;

    let mut run_settings = settings.clone();
    apply_transcription_options(&mut run_settings, options);
    let progress_sink = workbench_transcription_progress_sink_with_mode(
        app.clone(),
        task_id.to_string(),
        "downloaded-reference",
        reference_correction_state.clone(),
    );
    let checkpoint_context = WorkbenchCheckpointContext::new(
        task_id,
        "prepare-subtitle:downloaded-reference",
        checkpoint_hash(&build_downloaded_reference_transcription_checkpoint_input(
            &subtitle,
            video,
            options,
            &run_settings,
        )),
    );
    let mut result = run_transcription_workflow_with_checkpoint(
        app.clone(),
        ai_service,
        app_logger,
        Some(store),
        TranscriptionRequest {
            file_path: video.file_path.clone(),
            model: options.transcription_model.clone(),
            source_language: options.source_language.clone(),
            output_format: options.transcription_format.clone(),
            client_run_id: task_id.to_string(),
            progress_source: "home-workbench".to_string(),
        },
        run_settings,
        Some(progress_sink),
        store,
        checkpoint_context.clone(),
    )
    .await?;

    update_reference_correction_state(
        &reference_correction_state,
        json!({
            "status": "active",
            "message": "匹配参考字幕",
        }),
    );
    update_stage_snapshot_from_app(
        &app,
        task_id,
        STAGE_PREPARE_SUBTITLE,
        96,
        "匹配参考字幕",
        json!({
            "mode": "downloaded-reference",
            "message": "匹配参考字幕",
            "stageProgress": serde_json::Value::Null,
            "segments": result.segments,
            "warnings": result.warnings,
            "referenceCorrection": current_reference_correction_state(&reference_correction_state),
        }),
    )?;

    let mut warnings = result.warnings.clone();
    let mut reference_correction = json!({
        "status": "active",
        "message": "AI 参考校正中",
        "matchVersion": reference_match_version(),
    });
    let reference_match =
        match match_downloaded_subtitle_references(&result.segments, &downloaded_segments) {
            Ok(reference_match) => {
                let mut match_value =
                    serde_json::to_value(&reference_match.report).unwrap_or_else(|_| json!({}));
                if let Some(object) = match_value.as_object_mut() {
                    object.insert("status".to_string(), json!("matched"));
                }
                if let Some(object) = reference_correction.as_object_mut() {
                    object.insert("match".to_string(), match_value);
                    object.insert(
                        "referenceCount".to_string(),
                        json!(reference_match.matches.len()),
                    );
                }
                warnings.extend(reference_match.report.warnings.clone());
                Some(reference_match)
            }
            Err(error) => {
                let warning = format!("下载字幕参考匹配失败，已保留转录字幕：{error}");
                warnings.push(warning);
                reference_correction = json!({
                    "status": "failed",
                    "message": "参考匹配失败，已保留转录字幕",
                    "matchVersion": reference_match_version(),
                    "error": error,
                });
                None
            }
        };

    update_reference_correction_state(&reference_correction_state, reference_correction.clone());
    if let Some(reference_match) = reference_match {
        let references = reference_match
            .matches
            .iter()
            .map(|item| SubtitleReferenceCorrectionReference {
                asr_index: item.asr_index,
                reference_text: item.reference_text.clone(),
                confidence: item.confidence,
            })
            .collect::<Vec<_>>();
        let previous_warnings = warnings.clone();
        let reference_log_session =
            app_logger.start_session("home_workbench_reference_correction")?;
        let mut report_reference_correction =
            |progress: u8,
             message: &str,
             snapshot_segments: &[crate::transcription::TranscriptionSegment],
             snapshot_warnings: &[String]| {
                let mut combined_warnings = previous_warnings.clone();
                combined_warnings.extend(snapshot_warnings.iter().cloned());
                let mut state = reference_correction.clone();
                if let Some(object) = state.as_object_mut() {
                    object.insert(
                        "status".to_string(),
                        json!(if progress >= 100 { "done" } else { "active" }),
                    );
                    object.insert("progress".to_string(), json!(progress));
                    object.insert("message".to_string(), json!(message));
                }
                update_reference_correction_state(&reference_correction_state, state.clone());
                let stage_progress = 96u8.saturating_add(progress.min(100) / 34).min(99);
                let _ = update_stage_snapshot_from_app(
                    &app,
                    task_id,
                    STAGE_PREPARE_SUBTITLE,
                    stage_progress,
                    message,
                    json!({
                        "mode": "downloaded-reference",
                        "message": message,
                        "stageProgress": serde_json::Value::Null,
                        "segments": snapshot_segments,
                        "warnings": combined_warnings,
                        "referenceCorrection": state,
                    }),
                );
            };
        let correction_result = correct_subtitles_with_downloaded_reference(
            settings,
            ai_service,
            &reference_log_session,
            result.segments,
            &references,
            &mut report_reference_correction,
            Some((store, &checkpoint_context.child("reference-correction"))),
        )
        .await;
        result.segments = correction_result.segments;
        warnings.extend(correction_result.warnings);
        let mut done_state = current_reference_correction_state(&reference_correction_state);
        if let Some(object) = done_state.as_object_mut() {
            object.insert("status".to_string(), json!("done"));
            object.insert("progress".to_string(), json!(100));
            object.insert("message".to_string(), json!("AI 参考校正完成"));
        }
        reference_correction = done_state;
    }

    result.warnings = warnings;
    let output_format = normalize_subtitle_format(&result.output_format);
    result.subtitle_text =
        serialize_segments_for_export(&result.segments, output_format, Some(store), settings)?;

    let mut metadata = json!({
        "mode": "downloaded-reference",
        "referenceCorrection": reference_correction,
        "subtitleId": &subtitle.id,
        "language": &subtitle.language,
        "languageName": &subtitle.language_name,
        "sourceKind": &subtitle.source_kind,
        "format": &subtitle.format,
    });
    metadata["format"] = json!(result.output_format);
    metadata["segmentCount"] = json!(result.segments.len());
    metadata["logPath"] = json!(result.log_path);
    metadata["warnings"] = json!(result.warnings);
    metadata["input"] = json!(build_downloaded_subtitle_cache_key(
        &subtitle, video, options
    ));

    let stem = "reference-corrected";
    let output_path = workbench_file_path(task_id, stem, &result.output_format)?;
    let output_path_string = output_path.to_string_lossy().to_string();
    write_text_file(&output_path_string, result.subtitle_text.clone())?;
    upsert_artifact_from_path(
        store,
        task_id,
        ARTIFACT_REFERENCE_CORRECTED_SUBTITLE,
        &output_path,
        metadata.clone(),
    )?;
    update_stage_snapshot_from_app(
        &app,
        task_id,
        STAGE_PREPARE_SUBTITLE,
        99,
        "AI 参考校正完成",
        json!({
            "mode": "downloaded-reference",
            "path": output_path.to_string_lossy(),
            "metadata": metadata,
            "stageProgress": serde_json::Value::Null,
            "segments": result.segments,
            "warnings": result.warnings,
            "referenceCorrection": current_reference_correction_state(&reference_correction_state),
            "message": "AI 参考校正完成",
        }),
    )?;
    Ok(output_path)
}

fn update_reference_correction_state(state: &Arc<Mutex<Value>>, value: Value) {
    if let Ok(mut current) = state.lock() {
        *current = value;
    }
}

fn current_reference_correction_state(state: &Arc<Mutex<Value>>) -> Value {
    state
        .lock()
        .map(|value| value.clone())
        .unwrap_or_else(|_| json!({}))
}

async fn translate_subtitle(
    app: AppHandle,
    store: &SettingsStore,
    ai_service: &AiService,
    app_logger: &AppLogger,
    settings: &AppSettings,
    task_id: &str,
    options: &HomeWorkbenchOptions,
    subtitle_path: &Path,
) -> Result<PathBuf, String> {
    if let Some(artifact) = workbench_artifact_file(store, task_id, ARTIFACT_TRANSLATED_SUBTITLE)? {
        update_stage_snapshot_from_app(
            &app,
            task_id,
            STAGE_TRANSLATION,
            99,
            "复用已翻译字幕",
            json!({
                "mode": "translation",
                "path": artifact.path,
                "fileSize": artifact.file_size,
                "metadata": artifact.metadata,
                "message": "复用已翻译字幕",
            }),
        )?;
        return Ok(artifact_path_value(&artifact));
    }

    set_stage_active(store, &app, task_id, STAGE_TRANSLATION, "翻译字幕", 2)?;
    let mut run_settings = settings.clone();
    apply_translation_options(&mut run_settings, options);
    let progress_sink = workbench_translation_progress_sink(app.clone(), task_id.to_string());
    let checkpoint_context = WorkbenchCheckpointContext::new(
        task_id,
        "translation",
        checkpoint_hash(&build_translation_checkpoint_input(
            subtitle_path,
            options,
            &run_settings,
        )),
    );
    let result = run_subtitle_translation_workflow_with_sink(
        app,
        ai_service,
        app_logger,
        Some(store),
        SubtitleTranslationRequest {
            file_path: subtitle_path.to_string_lossy().to_string(),
            client_run_id: task_id.to_string(),
            progress_source: "home-workbench".to_string(),
        },
        run_settings,
        Some(progress_sink),
        Some((store, checkpoint_context)),
    )
    .await?;
    save_translation_result(store, task_id, &result)
}

async fn run_dubbing(
    app: AppHandle,
    store: &SettingsStore,
    task_id: &str,
    options: &HomeWorkbenchOptions,
    video: &HomeVideoDownload,
    subtitle_path: &Path,
) -> Result<(PathBuf, PathBuf), String> {
    if let (Some(video_artifact), Some(subtitle_artifact)) = (
        workbench_artifact_file(store, task_id, ARTIFACT_DUBBED_VIDEO)?,
        workbench_artifact_file(store, task_id, ARTIFACT_DUBBED_SUBTITLE)?,
    ) {
        update_stage_snapshot_from_app(
            &app,
            task_id,
            STAGE_DUBBING,
            99,
            "复用已完成配音",
            json!({
                "mode": "dubbing",
                "videoPath": &video_artifact.path,
                "subtitlePath": &subtitle_artifact.path,
                "message": "复用已完成配音",
            }),
        )?;
        return Ok((
            artifact_path_value(&video_artifact),
            artifact_path_value(&subtitle_artifact),
        ));
    }

    set_stage_active(store, &app, task_id, STAGE_DUBBING, "配音流程执行中", 2)?;
    let prepared = prepare_dubbing_material_internal(
        app.clone(),
        store,
        PrepareDubbingMaterialRequest {
            video_path: video.file_path.clone(),
            subtitle_path: subtitle_path.to_string_lossy().to_string(),
        },
    )?;
    update_stage_snapshot_from_app(
        &app,
        task_id,
        STAGE_DUBBING,
        5,
        "配音素材已准备",
        json!({
            "mode": "dubbing",
            "dubbingTaskId": &prepared.id,
            "dubbingSnapshot": &prepared,
            "message": "配音素材已准备",
        }),
    )?;
    let snapshot = start_dubbing_task_internal(
        app.clone(),
        StartDubbingTaskRequest {
            task_id: prepared.id,
            options: DubbingTaskOptions {
                tts_interval_ms: options.dubbing_tts_interval_ms,
                reference_audio_source: options.dubbing_reference_audio_source.clone(),
                custom_reference_audio_path: options.dubbing_custom_reference_audio_path.clone(),
                is_background_music_enabled: options.dubbing_is_background_music_enabled,
                background_music_volume: options.dubbing_background_music_volume,
            },
            client_run_id: task_id.to_string(),
            progress_source: "home-workbench".to_string(),
        },
    )
    .await?;
    let video_path = artifact_path(&snapshot, DUBBING_FINAL_DUBBED_VIDEO_ARTIFACT)
        .ok_or_else(|| "配音完成但未找到最终视频".to_string())?;
    let subtitle_path = artifact_path(&snapshot, DUBBING_FINAL_SUBTITLE_ARTIFACT)
        .ok_or_else(|| "配音完成但未找到最终字幕".to_string())?;
    upsert_artifact_from_path(
        store,
        task_id,
        ARTIFACT_DUBBED_VIDEO,
        &video_path,
        json!({ "dubbingTaskId": snapshot.id }),
    )?;
    update_stage_snapshot_from_app(
        &app,
        task_id,
        STAGE_DUBBING,
        99,
        "配音完成",
        json!({
            "mode": "dubbing",
            "dubbingTaskId": &snapshot.id,
            "dubbingSnapshot": &snapshot,
            "message": "配音完成",
        }),
    )?;
    Ok((video_path, subtitle_path))
}

async fn generate_workbench_content_copy(
    app: AppHandle,
    store: &SettingsStore,
    ai_service: &AiService,
    app_logger: &AppLogger,
    settings: &AppSettings,
    task_id: &str,
    subtitle_path: &Path,
) -> Result<(), String> {
    if let Some(mut snapshot) = reusable_content_copy_snapshot(store, task_id)? {
        if let Some(snapshot_object) = snapshot.as_object_mut() {
            snapshot_object.insert("message".to_string(), json!("复用已生成文案"));
        }
        set_stage_done_with_snapshot(
            store,
            &app,
            task_id,
            STAGE_CONTENT_COPY,
            "复用已生成文案",
            WORKBENCH_STATUS_RUNNING,
            snapshot,
        )?;
        return Ok(());
    }

    set_stage_active(store, &app, task_id, STAGE_CONTENT_COPY, "准备生成文案", 5)?;
    let subtitle_path_text = subtitle_path.to_string_lossy().to_string();
    update_stage_snapshot_from_app(
        &app,
        task_id,
        STAGE_CONTENT_COPY,
        12,
        "正在生成文案",
        json!({
            "mode": "content-copy",
            "subtitlePath": &subtitle_path_text,
            "message": "正在生成文案",
        }),
    )?;

    let task =
        store.with_connection(|connection| read_home_video_task_by_id(connection, task_id))?;
    let extra_context = build_workbench_content_copy_context(&task);
    let checkpoint_context = WorkbenchCheckpointContext::new(
        task_id,
        "content-copy",
        checkpoint_hash(&build_content_copy_checkpoint_input(
            subtitle_path,
            &extra_context,
            settings,
        )),
    );
    let record = generate_content_copy_record_with_checkpoint(
        store,
        ai_service,
        app_logger,
        GenerateContentCopyRequest {
            subtitle_path: subtitle_path_text.clone(),
            extra_context,
            platform: None,
            source: Some("workbench".to_string()),
        },
        checkpoint_context,
    )
    .await?;

    set_stage_done_with_snapshot(
        store,
        &app,
        task_id,
        STAGE_CONTENT_COPY,
        "文案已生成",
        WORKBENCH_STATUS_RUNNING,
        json!({
            "mode": "content-copy",
            "subtitlePath": &subtitle_path_text,
            "recordId": &record.id,
            "record": &record,
            "message": "文案已生成",
        }),
    )?;

    Ok(())
}

fn reusable_content_copy_snapshot(
    store: &SettingsStore,
    task_id: &str,
) -> Result<Option<Value>, String> {
    store.with_connection(|connection| {
        Ok(
            read_workbench_record(connection, task_id)?.and_then(|record| {
                record
                    .stages
                    .into_iter()
                    .find(|stage| stage.key == STAGE_CONTENT_COPY)
                    .and_then(|stage| {
                        let record_id = stage
                            .snapshot
                            .get("recordId")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        if stage.status == STAGE_STATUS_DONE && !record_id.trim().is_empty() {
                            Some(stage.snapshot)
                        } else {
                            None
                        }
                    })
            }),
        )
    })
}

fn build_workbench_content_copy_context(task: &HomeVideoTask) -> String {
    let mut lines = Vec::new();
    push_context_line(&mut lines, "视频标题", &task.title);
    push_context_line(&mut lines, "频道", &task.channel_title);
    push_context_line(&mut lines, "视频地址", &task.webpage_url);
    if task.webpage_url.trim() != task.url.trim() {
        push_context_line(&mut lines, "原始地址", &task.url);
    }
    let description = task
        .description
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if !description.trim().is_empty() {
        lines.push(format!(
            "视频简介：{}",
            truncate_context_text(&description, 1200)
        ));
    }
    if lines.is_empty() {
        "来源：主页任务工作台".to_string()
    } else {
        lines.join("\n")
    }
}

fn push_context_line(lines: &mut Vec<String>, label: &str, value: &str) {
    let value = value.trim();
    if !value.is_empty() {
        lines.push(format!("{label}：{value}"));
    }
}

fn truncate_context_text(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

fn artifact_path(snapshot: &DubbingTaskSnapshot, kind: &str) -> Option<PathBuf> {
    snapshot
        .artifacts
        .iter()
        .find(|artifact: &&DubbingTaskArtifact| artifact.kind == kind && !artifact.path.is_empty())
        .map(|artifact| PathBuf::from(&artifact.path))
        .filter(|path| path.is_file())
}

fn workbench_transcription_progress_sink(
    app: AppHandle,
    task_id: String,
) -> TranscriptionProgressSink {
    workbench_transcription_progress_sink_with_mode(
        app,
        task_id,
        "transcribe",
        Arc::new(Mutex::new(json!({}))),
    )
}

fn workbench_transcription_progress_sink_with_mode(
    app: AppHandle,
    task_id: String,
    mode: &'static str,
    reference_correction_state: Arc<Mutex<Value>>,
) -> TranscriptionProgressSink {
    Arc::new(move |progress: TranscriptionProgress| {
        let reference_correction = reference_correction_state
            .lock()
            .map(|value| value.clone())
            .unwrap_or_else(|_| json!({}));
        let snapshot = json!({
            "mode": mode,
            "message": &progress.message,
            "stageProgress": &progress.stage_progress,
            "segments": &progress.segments,
            "warnings": &progress.warnings,
            "referenceCorrection": reference_correction,
            "revision": progress.revision,
        });
        let _ = update_stage_snapshot_from_app(
            &app,
            &task_id,
            STAGE_PREPARE_SUBTITLE,
            progress.progress,
            &progress.message,
            snapshot,
        );
    })
}

fn workbench_translation_progress_sink(
    app: AppHandle,
    task_id: String,
) -> SubtitleTranslationProgressSink {
    Arc::new(move |progress: SubtitleTranslationProgress| {
        let snapshot = json!({
            "mode": "translation",
            "message": &progress.message,
            "stageProgress": &progress.stage_progress,
            "sourceSegments": &progress.source_segments,
            "translatedSegments": &progress.translated_segments,
            "warnings": &progress.warnings,
            "revision": progress.revision,
        });
        let _ = update_stage_snapshot_from_app(
            &app,
            &task_id,
            STAGE_TRANSLATION,
            progress.progress,
            &progress.message,
            snapshot,
        );
    })
}

fn save_workbench_subtitle(
    store: &SettingsStore,
    task_id: &str,
    kind: &str,
    suggested_path: &str,
    content: &str,
    metadata: Value,
) -> Result<PathBuf, String> {
    let output_path = workbench_subtitle_path(store, task_id, suggested_path)?;
    let output_path_string = output_path.to_string_lossy().to_string();
    write_text_file(&output_path_string, content.to_string())?;
    upsert_artifact_from_path(store, task_id, kind, &output_path, metadata)?;
    Ok(output_path)
}

fn save_translation_result(
    store: &SettingsStore,
    task_id: &str,
    result: &SubtitleTranslationResult,
) -> Result<PathBuf, String> {
    let path = workbench_file_path(task_id, "translated", &result.output_format)?;
    let path_string = path.to_string_lossy().to_string();
    write_text_file(&path_string, result.subtitle_text.clone())?;
    upsert_artifact_from_path(
        store,
        task_id,
        ARTIFACT_TRANSLATED_SUBTITLE,
        &path,
        json!({
            "format": result.output_format,
            "mode": result.output_mode,
            "sourceSegmentCount": result.source_segments.len(),
            "translatedSegmentCount": result.translated_segments.len(),
            "logPath": result.log_path,
            "warnings": result.warnings,
        }),
    )?;
    Ok(path)
}

fn workbench_subtitle_path(
    _store: &SettingsStore,
    task_id: &str,
    suggested_path: &str,
) -> Result<PathBuf, String> {
    let extension = Path::new(suggested_path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("srt");
    workbench_file_path(task_id, "transcription", extension)
}

fn workbench_file_path(task_id: &str, stem: &str, extension: &str) -> Result<PathBuf, String> {
    let dir = app_paths::youtube_task_dir(task_id)?.join("workbench");
    fs::create_dir_all(&dir).map_err(|error| format!("无法创建工作台目录: {error}"))?;
    Ok(dir.join(format!(
        "{}.{}",
        stem,
        normalize_extension(extension, "srt")
    )))
}

fn export_final_artifacts(
    store: &SettingsStore,
    task_id: &str,
    options: &HomeWorkbenchOptions,
    video_path: &Path,
    subtitle_path: &Path,
) -> Result<ExportedFinalPaths, String> {
    let export_dir = resolved_export_dir(&options.export_dir)?;
    let title = store.with_connection(|connection| {
        read_home_video_task_by_id(connection, task_id).map(|task| {
            if task.title.trim().is_empty() {
                task_id.to_string()
            } else {
                task.title
            }
        })
    })?;
    let base_name = sanitize_file_segment(&title);
    let video_extension = path_extension(video_path).unwrap_or_else(|| "mp4".to_string());
    let subtitle_extension = path_extension(subtitle_path).unwrap_or_else(|| "srt".to_string());
    let video_destination = unique_export_path(&export_dir, &base_name, &video_extension)?;
    let subtitle_destination = unique_export_path(&export_dir, &base_name, &subtitle_extension)?;
    fs::copy(video_path, &video_destination)
        .map_err(|error| format!("无法导出视频文件: {error}"))?;
    fs::copy(subtitle_path, &subtitle_destination)
        .map_err(|error| format!("无法导出字幕文件: {error}"))?;
    Ok(ExportedFinalPaths {
        video_path: video_destination,
        subtitle_path: subtitle_destination,
    })
}

fn reusable_exported_artifacts(
    store: &SettingsStore,
    task_id: &str,
) -> Result<Option<ExportedFinalPaths>, String> {
    let video = workbench_artifact_file(store, task_id, ARTIFACT_EXPORTED_VIDEO)?;
    let subtitle = workbench_artifact_file(store, task_id, ARTIFACT_EXPORTED_SUBTITLE)?;
    Ok(match (video, subtitle) {
        (Some(video), Some(subtitle)) => Some(ExportedFinalPaths {
            video_path: artifact_path_value(&video),
            subtitle_path: artifact_path_value(&subtitle),
        }),
        _ => None,
    })
}

fn selected_downloaded_subtitle(
    store: &SettingsStore,
    task_id: &str,
    subtitle_id: &str,
) -> Result<Option<HomeVideoSubtitle>, String> {
    store.with_connection(|connection| {
        let task = read_home_video_task_by_id(connection, task_id)?;
        if !subtitle_id.trim().is_empty() {
            if let Some(subtitle) = task.downloaded_subtitles.into_iter().find(|subtitle| {
                subtitle.id == subtitle_id && Path::new(&subtitle.file_path).is_file()
            }) {
                return Ok(Some(subtitle));
            }
            return Ok(None);
        }

        Ok(task
            .downloaded_subtitles
            .into_iter()
            .find(|subtitle| Path::new(&subtitle.file_path).is_file()))
    })
}

fn reusable_prepared_downloaded_subtitle(
    store: &SettingsStore,
    task_id: &str,
    options: &HomeWorkbenchOptions,
    video: &HomeVideoDownload,
) -> Result<Option<HomeWorkbenchArtifact>, String> {
    if options.subtitle_source != SUBTITLE_SOURCE_DOWNLOADED
        || options.subtitle_id.trim().is_empty()
    {
        return Ok(None);
    }

    let Some(subtitle) = selected_downloaded_subtitle(store, task_id, &options.subtitle_id)? else {
        return Ok(None);
    };
    let expected_key = build_downloaded_subtitle_cache_key(&subtitle, video, options);
    let Some(artifact) =
        workbench_artifact_file(store, task_id, ARTIFACT_REFERENCE_CORRECTED_SUBTITLE)?
    else {
        return Ok(None);
    };
    let artifact_key = artifact
        .metadata
        .get("input")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if artifact_key == expected_key {
        return Ok(Some(artifact));
    }

    Ok(None)
}

fn build_downloaded_subtitle_cache_key(
    subtitle: &HomeVideoSubtitle,
    video: &HomeVideoDownload,
    options: &HomeWorkbenchOptions,
) -> Value {
    json!({
        "referenceMatchVersion": reference_match_version(),
        "subtitleId": &subtitle.id,
        "subtitlePath": &subtitle.file_path,
        "subtitleFileSize": subtitle.file_size,
        "subtitleUpdatedAt": &subtitle.updated_at,
        "videoPath": &video.file_path,
        "videoFileSize": video.file_size,
        "videoUpdatedAt": &video.updated_at,
        "transcriptionModel": &options.transcription_model,
        "sourceLanguage": &options.source_language,
        "transcriptionFormat": &options.transcription_format,
        "smartSegmentationEnabled": options.is_smart_segmentation_enabled,
        "subtitleCorrectionEnabled": options.is_subtitle_correction_enabled,
        "aiSubtitleReviewEnabled": options.is_ai_subtitle_review_enabled,
        "aiSubtitleReviewMode": &options.ai_subtitle_review_mode,
        "videoContentType": &options.video_content_type,
        "translationBatchSize": options.translation_batch_size,
        "translationThreadCount": options.translation_thread_count,
    })
}

fn build_transcription_checkpoint_input(
    video: &HomeVideoDownload,
    options: &HomeWorkbenchOptions,
    settings: &AppSettings,
) -> Value {
    json!({
        "videoPath": &video.file_path,
        "videoFileSize": video.file_size,
        "videoUpdatedAt": &video.updated_at,
        "transcriptionModel": &options.transcription_model,
        "sourceLanguage": &options.source_language,
        "transcriptionFormat": &options.transcription_format,
        "smartSegmentationEnabled": options.is_smart_segmentation_enabled,
        "subtitleCorrectionEnabled": options.is_subtitle_correction_enabled,
        "aiSubtitleReviewEnabled": options.is_ai_subtitle_review_enabled,
        "aiSubtitleReviewMode": &options.ai_subtitle_review_mode,
        "videoContentType": &options.video_content_type,
        "translationBatchSize": options.translation_batch_size,
        "translationThreadCount": options.translation_thread_count,
        "selectedLlmService": &settings.selected_llm_service,
        "llmConfig": settings.llm_configs.get(&settings.selected_llm_service).map(|config| {
            json!({
                "baseUrl": &config.base_url,
                "model": &config.model,
                "reasoningEffort": &config.reasoning_effort,
                "streaming": config.is_streaming,
            })
        }),
    })
}

fn build_downloaded_reference_transcription_checkpoint_input(
    subtitle: &HomeVideoSubtitle,
    video: &HomeVideoDownload,
    options: &HomeWorkbenchOptions,
    settings: &AppSettings,
) -> Value {
    let mut value = build_transcription_checkpoint_input(video, options, settings);
    if let Some(object) = value.as_object_mut() {
        object.insert("subtitleId".to_string(), json!(&subtitle.id));
        object.insert("subtitlePath".to_string(), json!(&subtitle.file_path));
        object.insert("subtitleFileSize".to_string(), json!(subtitle.file_size));
        object.insert("subtitleUpdatedAt".to_string(), json!(&subtitle.updated_at));
        object.insert(
            "referenceMatchVersion".to_string(),
            json!(reference_match_version()),
        );
    }
    value
}

fn build_translation_checkpoint_input(
    subtitle_path: &Path,
    options: &HomeWorkbenchOptions,
    settings: &AppSettings,
) -> Value {
    json!({
        "subtitle": subtitle_file_checkpoint_metadata(subtitle_path),
        "translationFormat": &options.translation_format,
        "translationService": &options.translation_service,
        "needsReflectionTranslation": options.needs_reflection_translation,
        "translationBatchSize": options.translation_batch_size,
        "translationThreadCount": options.translation_thread_count,
        "videoContentType": &options.video_content_type,
        "outputMode": &options.output_mode,
        "subtitleTranslationEnabled": options.is_subtitle_translation_enabled,
        "aiSubtitleReviewEnabled": options.is_ai_subtitle_review_enabled,
        "aiSubtitleReviewMode": &options.ai_subtitle_review_mode,
        "targetLanguage": &options.target_language,
        "selectedLlmService": &settings.selected_llm_service,
        "llmConfig": settings.llm_configs.get(&settings.selected_llm_service).map(|config| {
            json!({
                "baseUrl": &config.base_url,
                "model": &config.model,
                "reasoningEffort": &config.reasoning_effort,
                "streaming": config.is_streaming,
            })
        }),
    })
}

fn build_content_copy_checkpoint_input(
    subtitle_path: &Path,
    extra_context: &str,
    settings: &AppSettings,
) -> Value {
    json!({
        "subtitle": subtitle_file_checkpoint_metadata(subtitle_path),
        "extraContext": extra_context,
        "platform": "bilibili",
        "titleCount": 6,
        "coverTextCount": 4,
        "selectedLlmService": &settings.selected_llm_service,
        "llmConfig": settings.llm_configs.get(&settings.selected_llm_service).map(|config| {
            json!({
                "baseUrl": &config.base_url,
                "model": &config.model,
                "reasoningEffort": &config.reasoning_effort,
                "streaming": config.is_streaming,
            })
        }),
    })
}

fn subtitle_file_checkpoint_metadata(path: &Path) -> Value {
    let metadata = fs::metadata(path).ok();
    json!({
        "path": path.to_string_lossy(),
        "fileSize": metadata.as_ref().map(|metadata| metadata.len()),
        "modified": metadata
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs()),
    })
}

fn initialize_run(
    store: &SettingsStore,
    task_id: &str,
    options: &HomeWorkbenchOptions,
) -> Result<(), String> {
    store.with_connection(|connection| {
        ensure_home_task_exists(connection, task_id)?;
        let now = Utc::now().to_rfc3339();
        let mut existing = read_workbench_record(connection, task_id)?
            .unwrap_or_else(|| default_workbench_record(task_id, options.clone(), &now));
        merge_workbench_stages(&mut existing.stages, options);
        for stage in &mut existing.stages {
            if matches!(
                stage.status.as_str(),
                STAGE_STATUS_ACTIVE | STAGE_STATUS_FAILED | STAGE_STATUS_INTERRUPTED
            ) {
                stage.status = STAGE_STATUS_PENDING.to_string();
                stage.progress = stage.progress.min(99);
            }
        }
        upsert_workbench_record(
            connection,
            task_id,
            WORKBENCH_STATUS_RUNNING,
            STAGE_DOWNLOAD_VIDEO,
            existing.progress,
            "工作台继续执行",
            &existing.stages,
            options,
            &existing.warnings,
            "",
            existing.revision + 1,
            &existing.created_at,
            &now,
        )
    })
}

fn mark_workbench_done(
    store: &SettingsStore,
    app: &AppHandle,
    task_id: &str,
) -> Result<HomeWorkbenchSnapshot, String> {
    let settings = store.load()?;
    let snapshot = store.with_connection(|connection| {
        let mut record = read_workbench_record(connection, task_id)?
            .ok_or_else(|| "未找到工作台任务".to_string())?;
        record.status = WORKBENCH_STATUS_DONE.to_string();
        record.current_stage = STAGE_EXPORT.to_string();
        record.progress = 100;
        record.message = "工作台已完成".to_string();
        record.error_message.clear();
        let now = Utc::now().to_rfc3339();
        upsert_workbench_record(
            connection,
            task_id,
            &record.status,
            &record.current_stage,
            record.progress,
            &record.message,
            &record.stages,
            &record.options,
            &record.warnings,
            &record.error_message,
            record.revision + 1,
            &record.created_at,
            &now,
        )?;
        read_or_create_workbench_snapshot(connection, task_id, &settings, false)
    })?;
    emit_workbench_progress(app, &snapshot);
    Ok(snapshot)
}

fn mark_workbench_failed(
    store: &SettingsStore,
    task_id: &str,
    error: &str,
) -> Result<HomeWorkbenchSnapshot, String> {
    let settings = store.load()?;
    store.with_connection(|connection| {
        let mut record = read_workbench_record(connection, task_id)?.unwrap_or_else(|| {
            default_workbench_record(
                task_id,
                default_workbench_options(&settings),
                &Utc::now().to_rfc3339(),
            )
        });
        record.status = WORKBENCH_STATUS_FAILED.to_string();
        record.progress = overall_progress(&record.stages);
        record.message = "工作台执行失败".to_string();
        record.error_message = compact_error(error);
        if let Some(stage) = record
            .stages
            .iter_mut()
            .find(|stage| stage.status == STAGE_STATUS_ACTIVE)
        {
            stage.status = STAGE_STATUS_FAILED.to_string();
            stage.message = "执行失败，查看下方错误详情".to_string();
        }
        let now = Utc::now().to_rfc3339();
        upsert_workbench_record(
            connection,
            task_id,
            &record.status,
            &record.current_stage,
            record.progress,
            &record.message,
            &record.stages,
            &record.options,
            &record.warnings,
            &record.error_message,
            record.revision + 1,
            &record.created_at,
            &now,
        )?;
        read_or_create_workbench_snapshot(connection, task_id, &settings, false)
    })
}

fn set_stage_active(
    store: &SettingsStore,
    app: &AppHandle,
    task_id: &str,
    stage_key: &str,
    message: &str,
    progress: u8,
) -> Result<HomeWorkbenchSnapshot, String> {
    update_stage(
        store,
        app,
        task_id,
        stage_key,
        progress,
        STAGE_STATUS_ACTIVE,
        message,
        WORKBENCH_STATUS_RUNNING,
        None,
    )
}

fn set_stage_done(
    store: &SettingsStore,
    app: &AppHandle,
    task_id: &str,
    stage_key: &str,
    message: &str,
) -> Result<HomeWorkbenchSnapshot, String> {
    update_stage(
        store,
        app,
        task_id,
        stage_key,
        100,
        STAGE_STATUS_DONE,
        message,
        WORKBENCH_STATUS_RUNNING,
        None,
    )
}

fn set_stage_done_with_snapshot(
    store: &SettingsStore,
    app: &AppHandle,
    task_id: &str,
    stage_key: &str,
    message: &str,
    workbench_status: &str,
    snapshot: Value,
) -> Result<HomeWorkbenchSnapshot, String> {
    update_stage(
        store,
        app,
        task_id,
        stage_key,
        100,
        STAGE_STATUS_DONE,
        message,
        workbench_status,
        Some(snapshot),
    )
}

fn set_stage_skipped(
    store: &SettingsStore,
    app: &AppHandle,
    task_id: &str,
    stage_key: &str,
    message: &str,
) -> Result<HomeWorkbenchSnapshot, String> {
    update_stage(
        store,
        app,
        task_id,
        stage_key,
        100,
        STAGE_STATUS_SKIPPED,
        message,
        WORKBENCH_STATUS_RUNNING,
        None,
    )
}

fn update_stage(
    store: &SettingsStore,
    app: &AppHandle,
    task_id: &str,
    stage_key: &str,
    progress: u8,
    status: &str,
    message: &str,
    workbench_status: &str,
    snapshot: Option<Value>,
) -> Result<HomeWorkbenchSnapshot, String> {
    let settings = store.load()?;
    let snapshot = store.with_connection(|connection| {
        let mut record = read_workbench_record(connection, task_id)?
            .ok_or_else(|| "未找到工作台任务".to_string())?;
        for stage in &mut record.stages {
            if stage.key == stage_key {
                stage.progress = progress.min(100);
                stage.status = status.to_string();
                stage.message = message.to_string();
                if let Some(snapshot) = snapshot.clone() {
                    stage.snapshot = snapshot;
                }
            }
        }
        record.status = workbench_status.to_string();
        record.current_stage = stage_key.to_string();
        record.progress = overall_progress(&record.stages);
        record.message = message.to_string();
        record.error_message.clear();
        let now = Utc::now().to_rfc3339();
        upsert_workbench_record(
            connection,
            task_id,
            &record.status,
            &record.current_stage,
            record.progress,
            &record.message,
            &record.stages,
            &record.options,
            &record.warnings,
            &record.error_message,
            record.revision + 1,
            &record.created_at,
            &now,
        )?;
        read_or_create_workbench_snapshot(connection, task_id, &settings, false)
    })?;
    emit_workbench_progress(app, &snapshot);
    Ok(snapshot)
}

fn update_stage_snapshot_from_app(
    app: &AppHandle,
    task_id: &str,
    stage_key: &str,
    progress: u8,
    message: &str,
    snapshot: Value,
) -> Result<HomeWorkbenchSnapshot, String> {
    let store = app.state::<SettingsStore>();
    update_stage(
        &store,
        app,
        task_id,
        stage_key,
        progress.min(99),
        STAGE_STATUS_ACTIVE,
        message,
        WORKBENCH_STATUS_RUNNING,
        Some(snapshot),
    )
}

fn read_or_create_workbench_snapshot(
    connection: &rusqlite::Connection,
    task_id: &str,
    settings: &AppSettings,
    should_interrupt_stale_running: bool,
) -> Result<HomeWorkbenchSnapshot, String> {
    ensure_home_task_exists(connection, task_id)?;
    let now = Utc::now().to_rfc3339();
    if read_workbench_record(connection, task_id)?.is_none() {
        let options = default_workbench_options(settings);
        let stages = initial_stages(&options);
        upsert_workbench_record(
            connection,
            task_id,
            WORKBENCH_STATUS_IDLE,
            "",
            0,
            "等待开始",
            &stages,
            &options,
            &[],
            "",
            1,
            &now,
            &now,
        )?;
    }
    let mut record = read_workbench_record(connection, task_id)?
        .ok_or_else(|| "无法创建工作台任务".to_string())?;
    let mut record_changed = false;
    let before_stage_count = record.stages.len();
    merge_workbench_stages(&mut record.stages, &record.options);
    if record.stages.len() != before_stage_count {
        record_changed = true;
    }
    if should_interrupt_stale_running && record.status == WORKBENCH_STATUS_RUNNING {
        mark_record_interrupted(&mut record);
        record_changed = true;
    }
    if record.status != WORKBENCH_STATUS_RUNNING {
        let cascaded_options =
            cascade_settings_into_workbench_options(record.options.clone(), settings);
        if record.options != cascaded_options {
            record.options = cascaded_options;
            record_changed = true;
        }
        if record.status == WORKBENCH_STATUS_IDLE
            && sync_option_dependent_stage_messages(&mut record)
        {
            record_changed = true;
        }
    }
    if sync_idle_workbench_stages(connection, task_id, &mut record)? {
        record_changed = true;
    }
    if record_changed {
        let now = Utc::now().to_rfc3339();
        upsert_workbench_record(
            connection,
            task_id,
            &record.status,
            &record.current_stage,
            record.progress,
            &record.message,
            &record.stages,
            &record.options,
            &record.warnings,
            &record.error_message,
            record.revision + 1,
            &record.created_at,
            &now,
        )?;
        record.updated_at = now;
        record.revision += 1;
    }
    let artifacts = read_workbench_artifacts(connection, task_id)?;

    Ok(HomeWorkbenchSnapshot {
        task_id: task_id.to_string(),
        status: record.status,
        current_stage: record.current_stage,
        progress: record.progress,
        message: record.message,
        stages: record.stages,
        options: normalize_options(record.options, settings),
        artifacts,
        warnings: record.warnings,
        error_message: record.error_message,
        revision: record.revision,
        created_at: record.created_at,
        updated_at: record.updated_at,
    })
}

fn mark_record_interrupted(record: &mut HomeWorkbenchRecord) {
    record.status = WORKBENCH_STATUS_INTERRUPTED.to_string();
    record.progress = overall_progress(&record.stages);
    record.message = "上次执行已中断，点击继续执行".to_string();
    record.error_message.clear();
    for stage in &mut record.stages {
        if stage.status == STAGE_STATUS_ACTIVE {
            stage.status = STAGE_STATUS_INTERRUPTED.to_string();
            stage.progress = stage.progress.min(99);
            stage.message = "已中断，点击继续执行".to_string();
            mark_snapshot_interrupted(&mut stage.snapshot);
        }
    }
}

fn mark_snapshot_interrupted(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let status_is_processing = object
                .get("status")
                .and_then(Value::as_str)
                .is_some_and(is_processing_snapshot_status);
            if status_is_processing {
                object.insert("status".to_string(), json!(STAGE_STATUS_INTERRUPTED));
                object.insert("message".to_string(), json!("已中断，点击继续执行"));
            }
            for child in object.values_mut() {
                mark_snapshot_interrupted(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                mark_snapshot_interrupted(item);
            }
        }
        _ => {}
    }
}

fn is_processing_snapshot_status(status: &str) -> bool {
    matches!(
        status,
        STAGE_STATUS_ACTIVE
            | "segmenting"
            | "correcting"
            | "translating"
            | "reviewing"
            | "optimizing"
            | "synthesizing"
            | "processing"
    )
}

fn sync_idle_workbench_stages(
    connection: &rusqlite::Connection,
    task_id: &str,
    record: &mut HomeWorkbenchRecord,
) -> Result<bool, String> {
    if record.status != WORKBENCH_STATUS_IDLE {
        return Ok(false);
    }

    let is_video_ready = matches!(
        read_workbench_artifact(connection, task_id, ARTIFACT_SOURCE_VIDEO)?,
        Some(artifact) if Path::new(&artifact.path).is_file()
    );
    if !is_video_ready {
        return Ok(false);
    }

    let mut changed = false;
    for stage in &mut record.stages {
        if stage.key == STAGE_DOWNLOAD_VIDEO && stage.status != STAGE_STATUS_DONE {
            stage.progress = 100;
            stage.status = STAGE_STATUS_DONE.to_string();
            stage.message = "视频文件已就绪".to_string();
            changed = true;
        }
    }

    if changed {
        record.progress = overall_progress(&record.stages);
    }

    Ok(changed)
}

fn default_workbench_options(settings: &AppSettings) -> HomeWorkbenchOptions {
    HomeWorkbenchOptions {
        subtitle_source: SUBTITLE_SOURCE_TRANSCRIBE.to_string(),
        subtitle_id: String::new(),
        translation_enabled: settings.home_workbench_translation_enabled,
        dubbing_enabled: settings.home_workbench_dubbing_enabled
            && settings.home_workbench_translation_enabled,
        export_dir: settings.home_workbench_export_dir.clone(),
        transcription_model: settings.transcription_model.clone(),
        source_language: settings.source_language.clone(),
        transcription_format: settings.transcription_format.clone(),
        is_smart_segmentation_enabled: settings.is_smart_segmentation_enabled,
        is_subtitle_correction_enabled: settings.is_subtitle_correction_enabled,
        translation_format: settings.translation_format.clone(),
        translation_service: settings.translation_service.clone(),
        needs_reflection_translation: settings.needs_reflection_translation,
        translation_batch_size: settings.translation_batch_size,
        translation_thread_count: settings.translation_thread_count,
        video_content_type: settings.video_content_type.clone(),
        output_mode: settings.output_mode.clone(),
        is_subtitle_translation_enabled: settings.is_subtitle_translation_enabled,
        is_ai_subtitle_review_enabled: settings.is_ai_subtitle_review_enabled,
        ai_subtitle_review_mode: settings.ai_subtitle_review_mode.clone(),
        target_language: settings.target_language.clone(),
        dubbing_tts_interval_ms: settings.dubbing_tts_interval_ms,
        dubbing_reference_audio_source: settings.dubbing_reference_audio_source.clone(),
        dubbing_custom_reference_audio_path: settings.dubbing_custom_reference_audio_path.clone(),
        dubbing_is_background_music_enabled: settings.dubbing_is_background_music_enabled,
        dubbing_background_music_volume: settings.dubbing_background_music_volume,
    }
}

fn normalize_options(
    mut options: HomeWorkbenchOptions,
    settings: &AppSettings,
) -> HomeWorkbenchOptions {
    if options.subtitle_source != SUBTITLE_SOURCE_DOWNLOADED {
        options.subtitle_source = SUBTITLE_SOURCE_TRANSCRIBE.to_string();
        options.subtitle_id.clear();
    }
    if options.dubbing_enabled {
        options.translation_enabled = true;
    }
    if !options.translation_enabled {
        options.dubbing_enabled = false;
    }
    if options.transcription_model.trim().is_empty() {
        options.transcription_model = settings.transcription_model.clone();
    }
    if options.source_language.trim().is_empty() {
        options.source_language = settings.source_language.clone();
    }
    options.transcription_format = normalize_extension(&options.transcription_format, "srt");
    options.translation_format = normalize_extension(&options.translation_format, "ass");
    if options.translation_service.trim().is_empty() {
        options.translation_service = settings.translation_service.clone();
    }
    if options.translation_batch_size == 0 {
        options.translation_batch_size = settings.translation_batch_size;
    }
    if options.translation_thread_count == 0 {
        options.translation_thread_count = settings.translation_thread_count;
    }
    if options.video_content_type.trim().is_empty() {
        options.video_content_type = settings.video_content_type.clone();
    }
    if options.output_mode.trim().is_empty() {
        options.output_mode = settings.output_mode.clone();
    }
    if options.target_language.trim().is_empty() {
        options.target_language = settings.target_language.clone();
    }
    if options.dubbing_tts_interval_ms > 1000 {
        options.dubbing_tts_interval_ms = 1000;
    }
    if options.dubbing_reference_audio_source.trim().is_empty() {
        options.dubbing_reference_audio_source = settings.dubbing_reference_audio_source.clone();
    }
    options.dubbing_background_music_volume =
        options.dubbing_background_music_volume.clamp(0.0, 1.0);
    options
}

fn cascade_settings_into_workbench_options(
    options: HomeWorkbenchOptions,
    settings: &AppSettings,
) -> HomeWorkbenchOptions {
    let subtitle_source = if options.subtitle_source == SUBTITLE_SOURCE_DOWNLOADED {
        SUBTITLE_SOURCE_DOWNLOADED.to_string()
    } else {
        SUBTITLE_SOURCE_TRANSCRIBE.to_string()
    };
    let subtitle_id = if subtitle_source == SUBTITLE_SOURCE_DOWNLOADED {
        options.subtitle_id
    } else {
        String::new()
    };
    let mut next = default_workbench_options(settings);
    next.subtitle_source = subtitle_source;
    next.subtitle_id = subtitle_id;
    normalize_options(next, settings)
}

fn apply_workbench_options_to_settings(settings: &mut AppSettings, options: &HomeWorkbenchOptions) {
    settings.home_workbench_translation_enabled = options.translation_enabled;
    settings.home_workbench_dubbing_enabled =
        options.dubbing_enabled && options.translation_enabled;
    settings.home_workbench_export_dir = options.export_dir.clone();
    apply_transcription_options(settings, options);
    apply_translation_options(settings, options);
    apply_dubbing_options(settings, options);
}

fn apply_transcription_options(settings: &mut AppSettings, options: &HomeWorkbenchOptions) {
    settings.transcription_model = options.transcription_model.clone();
    settings.source_language = options.source_language.clone();
    settings.transcription_format = options.transcription_format.clone();
    settings.is_smart_segmentation_enabled = options.is_smart_segmentation_enabled;
    settings.is_subtitle_correction_enabled = options.is_subtitle_correction_enabled;
    settings.video_content_type = options.video_content_type.clone();
    settings.translation_batch_size = options.translation_batch_size;
    settings.translation_thread_count = options.translation_thread_count;
}

fn apply_translation_options(settings: &mut AppSettings, options: &HomeWorkbenchOptions) {
    settings.translation_format = options.translation_format.clone();
    settings.translation_service = options.translation_service.clone();
    settings.needs_reflection_translation = options.needs_reflection_translation;
    settings.translation_batch_size = options.translation_batch_size;
    settings.translation_thread_count = options.translation_thread_count;
    settings.video_content_type = options.video_content_type.clone();
    settings.output_mode = options.output_mode.clone();
    settings.is_subtitle_translation_enabled = options.is_subtitle_translation_enabled;
    settings.is_ai_subtitle_review_enabled = options.is_ai_subtitle_review_enabled;
    settings.ai_subtitle_review_mode = options.ai_subtitle_review_mode.clone();
    settings.target_language = options.target_language.clone();
}

fn apply_dubbing_options(settings: &mut AppSettings, options: &HomeWorkbenchOptions) {
    settings.dubbing_tts_interval_ms = options.dubbing_tts_interval_ms;
    settings.dubbing_reference_audio_source = options.dubbing_reference_audio_source.clone();
    settings.dubbing_custom_reference_audio_path =
        options.dubbing_custom_reference_audio_path.clone();
    settings.dubbing_is_background_music_enabled = options.dubbing_is_background_music_enabled;
    settings.dubbing_background_music_volume = options.dubbing_background_music_volume;
}

fn initial_stages(options: &HomeWorkbenchOptions) -> Vec<HomeWorkbenchStage> {
    [
        (STAGE_DOWNLOAD_VIDEO, "下载视频", "等待下载视频"),
        (STAGE_PREPARE_SUBTITLE, "准备字幕", "等待准备字幕"),
        (
            STAGE_TRANSLATION,
            "翻译",
            if options.translation_enabled {
                "等待翻译"
            } else {
                "已关闭"
            },
        ),
        (
            STAGE_DUBBING,
            "配音",
            if options.dubbing_enabled {
                "等待配音"
            } else {
                "已关闭"
            },
        ),
        (STAGE_CONTENT_COPY, "文案", "等待生成文案"),
        (STAGE_EXPORT, "导出", "等待导出"),
    ]
    .into_iter()
    .map(|(key, label, message)| HomeWorkbenchStage {
        key: key.to_string(),
        label: label.to_string(),
        progress: 0,
        status: STAGE_STATUS_PENDING.to_string(),
        message: message.to_string(),
        snapshot: json!({}),
    })
    .collect()
}

fn merge_workbench_stages(stages: &mut Vec<HomeWorkbenchStage>, options: &HomeWorkbenchOptions) {
    let defaults = initial_stages(options);
    let default_order = defaults
        .iter()
        .map(|stage| stage.key.clone())
        .collect::<Vec<_>>();
    if stages.is_empty() {
        *stages = defaults;
        return;
    }

    for default_stage in defaults {
        if let Some(stage) = stages
            .iter_mut()
            .find(|stage| stage.key == default_stage.key)
        {
            stage.label = default_stage.label;
            if stage.message.trim().is_empty() {
                stage.message = default_stage.message;
            }
            if stage.snapshot.is_null() {
                stage.snapshot = json!({});
            }
        } else {
            stages.push(default_stage);
        }
    }
    stages.sort_by_key(|stage| {
        default_order
            .iter()
            .position(|key| key == &stage.key)
            .unwrap_or(default_order.len())
    });
}

fn sync_option_dependent_stage_messages(record: &mut HomeWorkbenchRecord) -> bool {
    let mut changed = false;
    for stage in &mut record.stages {
        if stage.key == STAGE_TRANSLATION && stage.label != "翻译" {
            stage.label = "翻译".to_string();
            changed = true;
        }

        if stage.status != STAGE_STATUS_PENDING {
            continue;
        }

        let next_message = match stage.key.as_str() {
            STAGE_TRANSLATION => {
                if record.options.translation_enabled {
                    Some("等待翻译")
                } else {
                    Some("已关闭")
                }
            }
            STAGE_DUBBING => {
                if record.options.dubbing_enabled {
                    Some("等待配音")
                } else {
                    Some("已关闭")
                }
            }
            _ => None,
        };

        if let Some(next_message) = next_message {
            if stage.message != next_message {
                stage.message = next_message.to_string();
                changed = true;
            }
        }
    }
    changed
}

struct HomeWorkbenchRecord {
    status: String,
    current_stage: String,
    progress: u8,
    message: String,
    stages: Vec<HomeWorkbenchStage>,
    options: HomeWorkbenchOptions,
    warnings: Vec<String>,
    error_message: String,
    revision: u64,
    created_at: String,
    updated_at: String,
}

fn default_workbench_record(
    task_id: &str,
    options: HomeWorkbenchOptions,
    now: &str,
) -> HomeWorkbenchRecord {
    let _ = task_id;
    HomeWorkbenchRecord {
        status: WORKBENCH_STATUS_IDLE.to_string(),
        current_stage: String::new(),
        progress: 0,
        message: "等待开始".to_string(),
        stages: initial_stages(&options),
        options,
        warnings: Vec::new(),
        error_message: String::new(),
        revision: 0,
        created_at: now.to_string(),
        updated_at: now.to_string(),
    }
}

fn read_workbench_record(
    connection: &rusqlite::Connection,
    task_id: &str,
) -> Result<Option<HomeWorkbenchRecord>, String> {
    connection
        .query_row(
            "
            SELECT status, current_stage, progress, message, stages, options,
                   warnings, error_message, revision, created_at, updated_at
            FROM home_workbench_tasks
            WHERE task_id = ?1
            ",
            params![task_id],
            map_workbench_record,
        )
        .optional()
        .map_err(|error| format!("无法读取工作台任务: {error}"))
}

fn map_workbench_record(row: &Row<'_>) -> rusqlite::Result<HomeWorkbenchRecord> {
    let stages_text: String = row.get(4)?;
    let options_text: String = row.get(5)?;
    let warnings_text: String = row.get(6)?;
    let stages = serde_json::from_str::<Vec<HomeWorkbenchStage>>(&stages_text).unwrap_or_default();
    let options = serde_json::from_str::<HomeWorkbenchOptions>(&options_text)
        .unwrap_or_else(|_| default_workbench_options_from_empty());
    let warnings = serde_json::from_str::<Vec<String>>(&warnings_text).unwrap_or_default();
    let progress: i64 = row.get(2)?;
    let revision: i64 = row.get(8)?;
    Ok(HomeWorkbenchRecord {
        status: row.get(0)?,
        current_stage: row.get(1)?,
        progress: progress.clamp(0, 100) as u8,
        message: row.get(3)?,
        stages,
        options,
        warnings,
        error_message: row.get(7)?,
        revision: revision.max(0) as u64,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn default_workbench_options_from_empty() -> HomeWorkbenchOptions {
    HomeWorkbenchOptions {
        subtitle_source: SUBTITLE_SOURCE_TRANSCRIBE.to_string(),
        subtitle_id: String::new(),
        translation_enabled: true,
        dubbing_enabled: false,
        export_dir: String::new(),
        transcription_model: "bilibili".to_string(),
        source_language: "auto".to_string(),
        transcription_format: "srt".to_string(),
        is_smart_segmentation_enabled: true,
        is_subtitle_correction_enabled: true,
        translation_format: "ass".to_string(),
        translation_service: "llm".to_string(),
        needs_reflection_translation: true,
        translation_batch_size: 30,
        translation_thread_count: 10,
        video_content_type: "general".to_string(),
        output_mode: "bilingual".to_string(),
        is_subtitle_translation_enabled: true,
        is_ai_subtitle_review_enabled: true,
        ai_subtitle_review_mode: "expert".to_string(),
        target_language: "zh-Hans".to_string(),
        dubbing_tts_interval_ms: 150,
        dubbing_reference_audio_source: "existing-dubbing".to_string(),
        dubbing_custom_reference_audio_path: String::new(),
        dubbing_is_background_music_enabled: true,
        dubbing_background_music_volume: 0.5,
    }
}

#[allow(clippy::too_many_arguments)]
fn upsert_workbench_record(
    connection: &rusqlite::Connection,
    task_id: &str,
    status: &str,
    current_stage: &str,
    progress: u8,
    message: &str,
    stages: &[HomeWorkbenchStage],
    options: &HomeWorkbenchOptions,
    warnings: &[String],
    error_message: &str,
    revision: u64,
    created_at: &str,
    updated_at: &str,
) -> Result<(), String> {
    let stages_text =
        serde_json::to_string(stages).map_err(|error| format!("无法保存工作台阶段: {error}"))?;
    let options_text =
        serde_json::to_string(options).map_err(|error| format!("无法保存工作台参数: {error}"))?;
    let warnings_text =
        serde_json::to_string(warnings).map_err(|error| format!("无法保存工作台警告: {error}"))?;
    connection
        .execute(
            "
            INSERT INTO home_workbench_tasks (
                task_id, status, current_stage, progress, message, stages,
                options, warnings, error_message, revision, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ON CONFLICT(task_id) DO UPDATE SET
                status = excluded.status,
                current_stage = excluded.current_stage,
                progress = excluded.progress,
                message = excluded.message,
                stages = excluded.stages,
                options = excluded.options,
                warnings = excluded.warnings,
                error_message = excluded.error_message,
                revision = excluded.revision,
                updated_at = excluded.updated_at
            ",
            params![
                task_id,
                status,
                current_stage,
                progress,
                message,
                stages_text,
                options_text,
                warnings_text,
                error_message,
                revision.min(i64::MAX as u64) as i64,
                created_at,
                updated_at,
            ],
        )
        .map(|_| ())
        .map_err(|error| format!("无法保存工作台任务: {error}"))
}

fn read_workbench_artifacts(
    connection: &rusqlite::Connection,
    task_id: &str,
) -> Result<Vec<HomeWorkbenchArtifact>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT kind, path, file_size, metadata, created_at, updated_at
            FROM home_workbench_artifacts
            WHERE task_id = ?1
            ORDER BY datetime(updated_at) DESC
            ",
        )
        .map_err(|error| format!("无法读取工作台产物: {error}"))?;
    let rows = statement
        .query_map(params![task_id], |row| {
            let metadata_text: String = row.get(3)?;
            Ok(HomeWorkbenchArtifact {
                kind: row.get(0)?,
                path: row.get(1)?,
                file_size: row.get(2)?,
                metadata: serde_json::from_str(&metadata_text).unwrap_or_else(|_| json!({})),
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })
        .map_err(|error| format!("无法读取工作台产物: {error}"))?;
    let mut artifacts = Vec::new();
    for row in rows {
        artifacts.push(row.map_err(|error| format!("无法解析工作台产物: {error}"))?);
    }
    Ok(artifacts)
}

fn read_workbench_artifact(
    connection: &rusqlite::Connection,
    task_id: &str,
    kind: &str,
) -> Result<Option<HomeWorkbenchArtifact>, String> {
    connection
        .query_row(
            "
            SELECT kind, path, file_size, metadata, created_at, updated_at
            FROM home_workbench_artifacts
            WHERE task_id = ?1 AND kind = ?2
            LIMIT 1
            ",
            params![task_id, kind],
            |row| {
                let metadata_text: String = row.get(3)?;
                Ok(HomeWorkbenchArtifact {
                    kind: row.get(0)?,
                    path: row.get(1)?,
                    file_size: row.get(2)?,
                    metadata: serde_json::from_str(&metadata_text).unwrap_or_else(|_| json!({})),
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(|error| format!("无法读取工作台产物: {error}"))
}

fn workbench_artifact_file(
    store: &SettingsStore,
    task_id: &str,
    kind: &str,
) -> Result<Option<HomeWorkbenchArtifact>, String> {
    store.with_connection(|connection| {
        Ok(read_workbench_artifact(connection, task_id, kind)?
            .filter(|artifact| Path::new(&artifact.path).is_file()))
    })
}

fn artifact_video_download(task_id: &str, artifact: &HomeWorkbenchArtifact) -> HomeVideoDownload {
    let format = artifact
        .metadata
        .get("format")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| path_extension(Path::new(&artifact.path)).unwrap_or_default());
    HomeVideoDownload {
        id: artifact
            .metadata
            .get("videoId")
            .and_then(Value::as_str)
            .unwrap_or("workbench-video")
            .to_string(),
        task_id: task_id.to_string(),
        format,
        file_path: artifact.path.clone(),
        file_name: artifact
            .metadata
            .get("fileName")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                Path::new(&artifact.path)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .map(str::to_string)
            })
            .unwrap_or_default(),
        file_size: artifact.file_size,
        created_at: artifact.created_at.clone(),
        updated_at: artifact.updated_at.clone(),
    }
}

fn artifact_path_value(artifact: &HomeWorkbenchArtifact) -> PathBuf {
    PathBuf::from(&artifact.path)
}

fn upsert_artifact_from_path(
    store: &SettingsStore,
    task_id: &str,
    kind: &str,
    path: &Path,
    metadata: Value,
) -> Result<(), String> {
    let file_size = fs::metadata(path)
        .map(|metadata| metadata.len().min(i64::MAX as u64) as i64)
        .unwrap_or_default();
    let metadata_text = serde_json::to_string(&metadata).unwrap_or_else(|_| "{}".to_string());
    let path_text = path.to_string_lossy().to_string();
    let now = Utc::now().to_rfc3339();
    store.with_connection(|connection| {
        let existing_created_at = connection
            .query_row(
                "
                SELECT created_at
                FROM home_workbench_artifacts
                WHERE task_id = ?1 AND kind = ?2
                LIMIT 1
                ",
                params![task_id, kind],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("无法检查工作台产物: {error}"))?
            .unwrap_or_else(|| now.clone());
        connection
            .execute(
                "
                INSERT INTO home_workbench_artifacts (
                    id, task_id, kind, path, file_size, metadata, created_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ON CONFLICT(task_id, kind) DO UPDATE SET
                    path = excluded.path,
                    file_size = excluded.file_size,
                    metadata = excluded.metadata,
                    updated_at = excluded.updated_at
                ",
                params![
                    Uuid::new_v4().to_string(),
                    task_id,
                    kind,
                    path_text,
                    file_size,
                    metadata_text,
                    existing_created_at,
                    now,
                ],
            )
            .map(|_| ())
            .map_err(|error| format!("无法保存工作台产物: {error}"))
    })
}

fn delete_workbench_artifact(
    connection: &rusqlite::Connection,
    task_id: &str,
    kind: &str,
) -> Result<(), String> {
    connection
        .execute(
            "
            DELETE FROM home_workbench_artifacts
            WHERE task_id = ?1 AND kind = ?2
            ",
            params![task_id, kind],
        )
        .map(|_| ())
        .map_err(|error| format!("无法移除工作台产物: {error}"))
}

fn delete_downstream_workbench_artifacts(
    connection: &rusqlite::Connection,
    task_id: &str,
) -> Result<(), String> {
    for kind in [
        ARTIFACT_TRANSCRIPTION_SUBTITLE,
        ARTIFACT_ALIGNED_SELECTED_SUBTITLE,
        ARTIFACT_REFERENCE_CORRECTED_SUBTITLE,
        ARTIFACT_TRANSLATED_SUBTITLE,
        ARTIFACT_DUBBED_VIDEO,
        ARTIFACT_DUBBED_SUBTITLE,
        ARTIFACT_EXPORTED_VIDEO,
        ARTIFACT_EXPORTED_SUBTITLE,
    ] {
        delete_workbench_artifact(connection, task_id, kind)?;
    }
    Ok(())
}

fn reset_stage_to_pending(record: &mut HomeWorkbenchRecord, stage_key: &str, message: &str) {
    for stage in &mut record.stages {
        if stage.key == stage_key {
            stage.progress = 0;
            stage.status = STAGE_STATUS_PENDING.to_string();
            stage.message = message.to_string();
            stage.snapshot = json!({});
        }
    }
}

fn ensure_home_task_exists(connection: &rusqlite::Connection, task_id: &str) -> Result<(), String> {
    read_home_video_task_by_id(connection, task_id).map(|_| ())
}

fn overall_progress(stages: &[HomeWorkbenchStage]) -> u8 {
    if stages.is_empty() {
        return 0;
    }
    let total = stages
        .iter()
        .map(|stage| stage.progress as u32)
        .sum::<u32>();
    (total / stages.len() as u32).min(100) as u8
}

fn resolved_export_dir(value: &str) -> Result<PathBuf, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return app_paths::exports_dir();
    }
    let path = PathBuf::from(trimmed);
    fs::create_dir_all(&path).map_err(|error| format!("无法创建导出目录: {error}"))?;
    Ok(path)
}

fn unique_export_path(dir: &Path, base_name: &str, extension: &str) -> Result<PathBuf, String> {
    let extension = normalize_extension(extension, "bin");
    for index in 0..1000 {
        let name = if index == 0 {
            format!("{base_name}.{extension}")
        } else {
            format!("{base_name}-{index}.{extension}")
        };
        let candidate = dir.join(name);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err("无法生成唯一导出文件名".to_string())
}

fn path_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| normalize_extension(value, ""))
        .filter(|value| !value.is_empty())
}

fn normalize_extension(value: &str, fallback: &str) -> String {
    let value = value
        .trim()
        .trim_start_matches('.')
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

fn sanitize_file_segment(value: &str) -> String {
    let mut sanitized = value
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            ch if ch.is_control() => '_',
            ch => ch,
        })
        .collect::<String>();
    sanitized = sanitized.trim_matches(['.', ' ']).trim().to_string();
    if sanitized.is_empty() {
        "untitled".to_string()
    } else {
        sanitized.chars().take(96).collect()
    }
}

fn compact_error(error: &str) -> String {
    error
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("工作台执行失败")
        .chars()
        .take(240)
        .collect()
}

fn emit_workbench_progress(app: &AppHandle, snapshot: &HomeWorkbenchSnapshot) {
    let _ = app.emit(HOME_WORKBENCH_PROGRESS_EVENT, snapshot);
}
