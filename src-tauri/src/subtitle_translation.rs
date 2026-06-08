use crate::ai::AiService;
use crate::app_log::{AppLogger, LogSession};
use crate::settings::{AppSettings, SettingsStore};
use crate::subtitle_ai::SubtitleProcessingResult;
use crate::transcription::{normalize_subtitle_format, serialize_subtitle, TranscriptionSegment};
use futures::stream::{FuturesUnordered, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

const PROGRESS_EVENT: &str = "subtitle-translation-progress";
const MAX_TRANSLATION_ATTEMPTS: usize = 3;
const MAX_POST_OPTIMIZATION_ATTEMPTS: usize = 3;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleTranslationRequest {
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitlePreviewResult {
    pub segments: Vec<TranscriptionSegment>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleTranslationProgress {
    pub progress: u8,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage_progress: Option<SubtitleTranslationStageProgress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_segments: Option<Vec<TranscriptionSegment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translated_segments: Option<Vec<TranscriptionSegment>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleTranslationProgressStage {
    pub progress: u8,
    pub message: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleTranslationStageProgress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle_translation: Option<SubtitleTranslationProgressStage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_translation_optimization: Option<SubtitleTranslationProgressStage>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleTranslationResult {
    pub source_segments: Vec<TranscriptionSegment>,
    pub translated_segments: Vec<TranscriptionSegment>,
    pub output_segments: Vec<TranscriptionSegment>,
    pub subtitle_text: String,
    pub source_subtitle_text: String,
    pub target_subtitle_text: String,
    pub output_format: String,
    pub output_mode: String,
    pub log_path: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
enum TranslationProgressStage {
    SubtitleTranslation,
    PostTranslationOptimization,
}

#[derive(Clone)]
struct TranslationWorkflowProgress {
    stages: std::sync::Arc<std::sync::Mutex<SubtitleTranslationStageProgress>>,
}

struct TranslationSnapshotEmitter {
    app: AppHandle,
    revision: u64,
    workflow_progress: TranslationWorkflowProgress,
}

#[derive(Debug, Clone)]
struct TextChunk {
    start_index: usize,
    end_index: usize,
    entries: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
struct TranslationChunk {
    start_index: usize,
    end_index: usize,
    entries: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
struct TextChunkResult {
    chunk: TextChunk,
    entries: Vec<(usize, String)>,
}

#[derive(Debug, Clone)]
struct TranslationChunkResult {
    chunk: TranslationChunk,
    entries: Vec<(usize, String)>,
}

#[tauri::command]
pub fn load_subtitle_preview(file_path: String) -> Result<SubtitlePreviewResult, String> {
    let path = PathBuf::from(file_path);
    let segments = load_subtitle_segments(&path)?;

    Ok(SubtitlePreviewResult { segments })
}

#[tauri::command]
pub async fn start_subtitle_translation(
    app: AppHandle,
    settings_store: tauri::State<'_, SettingsStore>,
    ai_service: tauri::State<'_, AiService>,
    app_logger: tauri::State<'_, AppLogger>,
    request: SubtitleTranslationRequest,
) -> Result<SubtitleTranslationResult, String> {
    let log_session = app_logger.start_session("subtitle_translation")?;
    log_session.info(
        "request_received",
        "收到字幕翻译与优化请求",
        json!({ "filePath": &request.file_path }),
    );

    let settings = settings_store.load()?;
    log_translation_settings(&log_session, &settings);

    let input_path = PathBuf::from(&request.file_path);
    let output_format = normalize_subtitle_format(&settings.translation_format);
    let workflow_progress = TranslationWorkflowProgress::new();
    initialize_workflow_progress(&workflow_progress, &settings);
    emit_progress_event(
        &app,
        0,
        "准备处理字幕",
        Some(workflow_progress.snapshot()),
        None,
        None,
        None,
        &[],
    );

    let mut source_segments = load_subtitle_segments(&input_path)?;
    if source_segments.is_empty() {
        return Err("字幕内容为空".to_string());
    }

    assign_segment_metadata(&mut source_segments, "src", "raw");
    let mut translated_segments = build_empty_translated_segments(&source_segments);
    let mut warnings = Vec::new();
    let mut emitter = TranslationSnapshotEmitter::new(app.clone(), workflow_progress.clone());
    emitter.emit(
        "字幕已导入",
        &source_segments,
        &translated_segments,
        &warnings,
    );
    log_session.info(
        "subtitle_loaded",
        "字幕文件已读取",
        json!({
            "segmentCount": source_segments.len(),
            "inputFormat": input_path.extension().and_then(|value| value.to_str()).unwrap_or(""),
        }),
    );

    if settings.is_subtitle_translation_enabled {
        if settings.translation_service != "llm" {
            return Err(format!(
                "当前后端暂不支持该翻译服务: {}",
                settings.translation_service
            ));
        }

        workflow_progress.set_stage(
            TranslationProgressStage::SubtitleTranslation,
            0,
            "AI 字幕翻译中",
            "active",
        );
        emitter.emit(
            "AI 字幕翻译中",
            &source_segments,
            &translated_segments,
            &warnings,
        );

        let translation_result = translate_subtitles(
            &settings,
            &ai_service,
            &log_session,
            &source_segments,
            translated_segments,
            |progress, message, snapshot, snapshot_warnings| {
                let mut combined_warnings = warnings.clone();
                combined_warnings.extend(snapshot_warnings.iter().cloned());
                let status = if progress >= 100 { "done" } else { "active" };
                workflow_progress.set_stage(
                    TranslationProgressStage::SubtitleTranslation,
                    progress,
                    message,
                    status,
                );
                emitter.emit(message, &source_segments, snapshot, &combined_warnings);
            },
        )
        .await?;
        translated_segments = translation_result.segments;
        warnings.extend(translation_result.warnings);
        workflow_progress.set_stage(
            TranslationProgressStage::SubtitleTranslation,
            100,
            "AI 字幕翻译完成",
            "done",
        );
        emitter.emit(
            "AI 字幕翻译完成",
            &source_segments,
            &translated_segments,
            &warnings,
        );

        if settings.is_post_translation_optimization_enabled {
            workflow_progress.set_stage(
                TranslationProgressStage::PostTranslationOptimization,
                0,
                "AI 译后优化中",
                "active",
            );
            emitter.emit(
                "AI 译后优化中",
                &source_segments,
                &translated_segments,
                &warnings,
            );

            let optimization_result = optimize_translated_subtitles(
                &settings,
                &ai_service,
                &log_session,
                &source_segments,
                translated_segments,
                |progress, message, snapshot, snapshot_warnings| {
                    let mut combined_warnings = warnings.clone();
                    combined_warnings.extend(snapshot_warnings.iter().cloned());
                    let status = if progress >= 100 { "done" } else { "active" };
                    workflow_progress.set_stage(
                        TranslationProgressStage::PostTranslationOptimization,
                        progress,
                        message,
                        status,
                    );
                    emitter.emit(message, &source_segments, snapshot, &combined_warnings);
                },
            )
            .await;
            translated_segments = optimization_result.segments;
            warnings.extend(optimization_result.warnings);
            workflow_progress.set_stage(
                TranslationProgressStage::PostTranslationOptimization,
                100,
                "AI 译后优化完成",
                "done",
            );
            emitter.emit(
                "AI 译后优化完成",
                &source_segments,
                &translated_segments,
                &warnings,
            );
        }
    } else {
        translated_segments = source_segments.clone();
        assign_segment_metadata(&mut translated_segments, "target", "done");
    }

    mark_segments_status(&mut source_segments, "done");
    mark_segments_status(&mut translated_segments, "done");
    let output_segments = build_output_segments(
        &source_segments,
        &translated_segments,
        &settings.output_mode,
    );
    let subtitle_text = serialize_subtitle(&output_segments, output_format);
    let source_subtitle_text = serialize_subtitle(&source_segments, output_format);
    let target_subtitle_text = serialize_subtitle(&translated_segments, output_format);

    emitter.emit(
        "翻译与优化完成",
        &source_segments,
        &translated_segments,
        &warnings,
    );
    log_session.info(
        "subtitle_translation_completed",
        "字幕翻译与优化流程完成",
        json!({
            "sourceSegmentCount": source_segments.len(),
            "translatedSegmentCount": translated_segments.len(),
            "outputFormat": output_format.to_string(),
            "outputMode": &settings.output_mode,
            "warningCount": warnings.len(),
            "logPath": log_session.path_string(),
        }),
    );

    Ok(SubtitleTranslationResult {
        source_segments,
        translated_segments,
        output_segments,
        subtitle_text,
        source_subtitle_text,
        target_subtitle_text,
        output_format: output_format.to_string(),
        output_mode: settings.output_mode,
        log_path: log_session.path_string(),
        warnings,
    })
}

impl TranslationWorkflowProgress {
    fn new() -> Self {
        Self {
            stages: std::sync::Arc::new(std::sync::Mutex::new(
                SubtitleTranslationStageProgress::default(),
            )),
        }
    }

    fn set_stage(
        &self,
        stage: TranslationProgressStage,
        progress: u8,
        message: &str,
        status: &str,
    ) {
        if let Ok(mut stages) = self.stages.lock() {
            let stage_progress = Some(SubtitleTranslationProgressStage {
                progress: progress.min(100),
                message: message.to_string(),
                status: status.to_string(),
            });

            match stage {
                TranslationProgressStage::SubtitleTranslation => {
                    stages.subtitle_translation = stage_progress
                }
                TranslationProgressStage::PostTranslationOptimization => {
                    stages.post_translation_optimization = stage_progress
                }
            }
        }
    }

    fn snapshot(&self) -> SubtitleTranslationStageProgress {
        self.stages
            .lock()
            .map(|stages| stages.clone())
            .unwrap_or_default()
    }
}

impl TranslationSnapshotEmitter {
    fn new(app: AppHandle, workflow_progress: TranslationWorkflowProgress) -> Self {
        Self {
            app,
            revision: 0,
            workflow_progress,
        }
    }

    fn emit(
        &mut self,
        message: &str,
        source_segments: &[TranscriptionSegment],
        translated_segments: &[TranscriptionSegment],
        warnings: &[String],
    ) {
        self.revision += 1;
        let stage_progress = self.workflow_progress.snapshot();
        emit_progress_event(
            &self.app,
            overall_progress(&stage_progress),
            message,
            Some(stage_progress),
            Some(self.revision),
            Some(source_segments.to_vec()),
            Some(translated_segments.to_vec()),
            warnings,
        );
    }
}

fn initialize_workflow_progress(
    workflow_progress: &TranslationWorkflowProgress,
    settings: &AppSettings,
) {
    if settings.is_subtitle_translation_enabled {
        workflow_progress.set_stage(
            TranslationProgressStage::SubtitleTranslation,
            0,
            "等待开始翻译",
            "pending",
        );

        if settings.is_post_translation_optimization_enabled {
            workflow_progress.set_stage(
                TranslationProgressStage::PostTranslationOptimization,
                0,
                "等待翻译完成",
                "pending",
            );
        }
    }
}

async fn translate_subtitles<F>(
    settings: &AppSettings,
    ai_service: &AiService,
    log_session: &LogSession,
    source_segments: &[TranscriptionSegment],
    mut translated_segments: Vec<TranscriptionSegment>,
    mut report: F,
) -> Result<SubtitleProcessingResult, String>
where
    F: FnMut(u8, &str, &[TranscriptionSegment], &[String]),
{
    let chunks = build_translation_chunks(
        source_segments,
        settings.translation_batch_size.max(1) as usize,
    );
    if chunks.is_empty() {
        return Ok(SubtitleProcessingResult {
            segments: translated_segments,
            warnings: Vec::new(),
        });
    }

    log_session.info(
        "subtitle_translation_stage_prepared",
        "AI 字幕翻译批次已准备",
        json!({
            "inputSegmentCount": source_segments.len(),
            "chunkCount": chunks.len(),
            "batchSize": settings.translation_batch_size.max(1),
            "targetLanguage": &settings.target_language,
            "reflectionEnabled": settings.needs_reflection_translation,
            "videoContentType": &settings.video_content_type,
            "llmMode": "configured_llm_settings_json_response",
        }),
    );

    let total = chunks.len().max(1);
    let max_active = active_ai_work_count(settings);
    let mut futures = FuturesUnordered::new();
    let mut next_chunk_index = 0usize;
    let mut failed_chunks = 0usize;
    let mut warnings = Vec::new();

    while next_chunk_index < chunks.len() && futures.len() < max_active {
        let chunk = chunks[next_chunk_index].clone();
        mark_range_status(
            &mut translated_segments,
            chunk.start_index,
            chunk.end_index,
            "translating",
        );
        futures.push(run_translation_chunk(
            settings,
            ai_service,
            chunk,
            log_session.clone(),
        ));
        next_chunk_index += 1;
    }
    report(0, "AI 字幕翻译中", &translated_segments, &warnings);

    let mut completed = 0usize;
    while let Some(result) = futures.next().await {
        completed += 1;

        match result {
            Ok(result) => {
                for (index, text) in result.entries {
                    if let Some(segment) = translated_segments.get_mut(index) {
                        segment.text = text;
                    }
                }
                mark_range_status(
                    &mut translated_segments,
                    result.chunk.start_index,
                    result.chunk.end_index,
                    "translated",
                );
            }
            Err((chunk, error)) => {
                copy_source_range_to_target(source_segments, &mut translated_segments, &chunk);
                mark_range_status(
                    &mut translated_segments,
                    chunk.start_index,
                    chunk.end_index,
                    "kept",
                );
                failed_chunks += 1;
                log_session.warn(
                    "subtitle_translation_chunk_failed",
                    "字幕翻译批次失败，已保留原文",
                    json!({
                        "startIndex": chunk.start_index + 1,
                        "endIndex": chunk.end_index + 1,
                        "entryCount": chunk.entries.len(),
                        "error": &error,
                    }),
                );
            }
        }

        while next_chunk_index < chunks.len() && futures.len() < max_active {
            let chunk = chunks[next_chunk_index].clone();
            mark_range_status(
                &mut translated_segments,
                chunk.start_index,
                chunk.end_index,
                "translating",
            );
            futures.push(run_translation_chunk(
                settings,
                ai_service,
                chunk,
                log_session.clone(),
            ));
            next_chunk_index += 1;
        }

        warnings = build_processing_warnings("字幕翻译", failed_chunks, "翻译批次");
        let progress = stage_progress(0, 100, completed, total);
        let message = if completed == total {
            "字幕翻译完成"
        } else {
            "字幕翻译中"
        };
        report(progress, message, &translated_segments, &warnings);
    }

    if failed_chunks == total {
        return Err("字幕翻译全部失败，请检查 LLM 配置、网络或模型响应格式".to_string());
    }

    if failed_chunks > 0 {
        log_session.warn(
            "subtitle_translation_stage_partial",
            "AI 字幕翻译部分批次失败，已保留原文",
            json!({
                "failedChunkCount": failed_chunks,
                "chunkCount": total,
            }),
        );
    }

    Ok(SubtitleProcessingResult {
        segments: translated_segments,
        warnings,
    })
}

async fn optimize_translated_subtitles<F>(
    settings: &AppSettings,
    ai_service: &AiService,
    log_session: &LogSession,
    source_segments: &[TranscriptionSegment],
    mut translated_segments: Vec<TranscriptionSegment>,
    mut report: F,
) -> SubtitleProcessingResult
where
    F: FnMut(u8, &str, &[TranscriptionSegment], &[String]),
{
    let chunks = build_text_chunks(
        &translated_segments,
        settings.translation_batch_size.max(1) as usize,
    );
    if chunks.is_empty() {
        return SubtitleProcessingResult {
            segments: translated_segments,
            warnings: Vec::new(),
        };
    }

    log_session.info(
        "post_translation_optimization_stage_prepared",
        "AI 译后优化批次已准备",
        json!({
            "inputSegmentCount": translated_segments.len(),
            "chunkCount": chunks.len(),
            "batchSize": settings.translation_batch_size.max(1),
            "targetLanguage": &settings.target_language,
            "videoContentType": &settings.video_content_type,
            "llmMode": "configured_llm_settings_json_response",
        }),
    );

    let total = chunks.len().max(1);
    let max_active = active_ai_work_count(settings);
    let source_chunks = build_text_chunks(
        source_segments,
        settings.translation_batch_size.max(1) as usize,
    );
    let mut futures = FuturesUnordered::new();
    let mut next_chunk_index = 0usize;
    let mut failed_chunks = 0usize;
    let mut warnings = Vec::new();

    while next_chunk_index < chunks.len() && futures.len() < max_active {
        let chunk = chunks[next_chunk_index].clone();
        let source_entries = source_chunks
            .get(next_chunk_index)
            .map(|chunk| chunk.entries.clone())
            .unwrap_or_default();
        mark_range_status(
            &mut translated_segments,
            chunk.start_index,
            chunk.end_index,
            "optimizing",
        );
        futures.push(run_post_optimization_chunk(
            settings,
            ai_service,
            source_entries,
            chunk,
            log_session.clone(),
        ));
        next_chunk_index += 1;
    }
    report(0, "AI 译后优化中", &translated_segments, &warnings);

    let mut completed = 0usize;
    while let Some(result) = futures.next().await {
        completed += 1;

        match result {
            Ok(result) => {
                for (index, text) in result.entries {
                    if let Some(segment) = translated_segments.get_mut(index) {
                        segment.text = text;
                    }
                }
                mark_range_status(
                    &mut translated_segments,
                    result.chunk.start_index,
                    result.chunk.end_index,
                    "optimized",
                );
            }
            Err((chunk, error)) => {
                mark_range_status(
                    &mut translated_segments,
                    chunk.start_index,
                    chunk.end_index,
                    "translated",
                );
                failed_chunks += 1;
                log_session.warn(
                    "post_translation_optimization_chunk_failed",
                    "译后优化批次失败，已保留译文",
                    json!({
                        "startIndex": chunk.start_index + 1,
                        "endIndex": chunk.end_index + 1,
                        "entryCount": chunk.entries.len(),
                        "error": &error,
                    }),
                );
            }
        }

        while next_chunk_index < chunks.len() && futures.len() < max_active {
            let chunk = chunks[next_chunk_index].clone();
            let source_entries = source_chunks
                .get(next_chunk_index)
                .map(|chunk| chunk.entries.clone())
                .unwrap_or_default();
            mark_range_status(
                &mut translated_segments,
                chunk.start_index,
                chunk.end_index,
                "optimizing",
            );
            futures.push(run_post_optimization_chunk(
                settings,
                ai_service,
                source_entries,
                chunk,
                log_session.clone(),
            ));
            next_chunk_index += 1;
        }

        warnings = build_processing_warnings("译后优化", failed_chunks, "优化批次");
        let progress = stage_progress(0, 100, completed, total);
        let message = if completed == total {
            "译后优化完成"
        } else {
            "译后优化中"
        };
        report(progress, message, &translated_segments, &warnings);
    }

    if failed_chunks > 0 {
        log_session.warn(
            "post_translation_optimization_stage_partial",
            "AI 译后优化部分批次失败，已保留译文",
            json!({
                "failedChunkCount": failed_chunks,
                "chunkCount": total,
            }),
        );
    }

    SubtitleProcessingResult {
        segments: translated_segments,
        warnings,
    }
}

async fn run_translation_chunk(
    settings: &AppSettings,
    ai_service: &AiService,
    chunk: TranslationChunk,
    log_session: LogSession,
) -> Result<TranslationChunkResult, (TranslationChunk, String)> {
    translate_chunk_by_llm(settings, ai_service, chunk, log_session).await
}

async fn run_post_optimization_chunk(
    settings: &AppSettings,
    ai_service: &AiService,
    source_entries: BTreeMap<String, String>,
    chunk: TextChunk,
    log_session: LogSession,
) -> Result<TextChunkResult, (TextChunk, String)> {
    optimize_translation_chunk_by_llm(settings, ai_service, source_entries, chunk, log_session)
        .await
}

async fn translate_chunk_by_llm(
    settings: &AppSettings,
    ai_service: &AiService,
    chunk: TranslationChunk,
    log_session: LogSession,
) -> Result<TranslationChunkResult, (TranslationChunk, String)> {
    let system_prompt = build_translation_system_prompt(settings);
    let source_text = chunk
        .entries
        .values()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    let max_output_tokens = estimate_max_output_tokens(&source_text);
    let mut feedback = String::new();

    for attempt in 1..=MAX_TRANSLATION_ATTEMPTS {
        let user_prompt = build_translation_user_prompt(
            &chunk.entries,
            settings.needs_reflection_translation,
            &feedback,
        );
        let response = match ai_service
            .chat_for_json_output(
                settings,
                system_prompt.clone(),
                user_prompt,
                max_output_tokens,
            )
            .await
        {
            Ok(response) => response,
            Err(error) => {
                log_session.warn(
                    "subtitle_translation_llm_request_failed",
                    "字幕翻译 LLM 请求失败",
                    json!({
                        "attempt": attempt,
                        "startIndex": chunk.start_index + 1,
                        "endIndex": chunk.end_index + 1,
                        "error": &error,
                    }),
                );
                return Err((chunk, error));
            }
        };

        let parsed =
            match parse_translation_response(&response, settings.needs_reflection_translation) {
                Ok(parsed) => parsed,
                Err(error) => {
                    feedback = build_translation_json_feedback(
                        &chunk.entries,
                        settings.needs_reflection_translation,
                        &error,
                    );
                    continue;
                }
            };

        match validate_or_remap_relative_keys(&chunk.entries, parsed) {
            Ok(parsed) => {
                let entries = parsed
                    .into_iter()
                    .filter_map(|(key, text)| {
                        key.parse::<usize>().ok().map(|index| (index - 1, text))
                    })
                    .collect();
                return Ok(TranslationChunkResult { chunk, entries });
            }
            Err(error) => {
                feedback = build_translation_key_feedback(
                    &chunk.entries,
                    settings.needs_reflection_translation,
                    &error,
                );
            }
        }
    }

    Err((chunk, "LLM 翻译结果多次校验失败".to_string()))
}

async fn optimize_translation_chunk_by_llm(
    settings: &AppSettings,
    ai_service: &AiService,
    source_entries: BTreeMap<String, String>,
    chunk: TextChunk,
    log_session: LogSession,
) -> Result<TextChunkResult, (TextChunk, String)> {
    let system_prompt = build_post_optimization_system_prompt(settings);
    let current_text = chunk
        .entries
        .values()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    let max_output_tokens = estimate_max_output_tokens(&current_text);
    let mut feedback = String::new();

    for attempt in 1..=MAX_POST_OPTIMIZATION_ATTEMPTS {
        let user_prompt =
            build_post_optimization_user_prompt(&source_entries, &chunk.entries, &feedback);
        let response = match ai_service
            .chat_for_json_output(
                settings,
                system_prompt.clone(),
                user_prompt,
                max_output_tokens,
            )
            .await
        {
            Ok(response) => response,
            Err(error) => {
                log_session.warn(
                    "post_translation_optimization_llm_request_failed",
                    "译后优化 LLM 请求失败",
                    json!({
                        "attempt": attempt,
                        "startIndex": chunk.start_index + 1,
                        "endIndex": chunk.end_index + 1,
                        "error": &error,
                    }),
                );
                return Err((chunk, error));
            }
        };

        let parsed = match parse_json_text_map(&response) {
            Ok(parsed) => parsed,
            Err(error) => {
                feedback = build_json_parse_feedback(&chunk.entries, &error);
                log_post_optimization_validation_failure(
                    &log_session,
                    attempt,
                    &chunk,
                    "json_parse",
                    &error,
                );
                continue;
            }
        };

        match validate_or_remap_relative_keys(&chunk.entries, parsed) {
            Ok(parsed) => {
                let entries = parsed
                    .into_iter()
                    .filter_map(|(key, text)| {
                        key.parse::<usize>().ok().map(|index| (index - 1, text))
                    })
                    .collect();
                return Ok(TextChunkResult { chunk, entries });
            }
            Err(error) => {
                feedback = build_key_mismatch_feedback(&chunk.entries, &error);
                log_post_optimization_validation_failure(
                    &log_session,
                    attempt,
                    &chunk,
                    "key_mismatch",
                    &error,
                );
            }
        }
    }

    Err((chunk, "LLM 译后优化结果多次校验失败".to_string()))
}

fn log_post_optimization_validation_failure(
    log_session: &LogSession,
    attempt: usize,
    chunk: &TextChunk,
    validation_type: &str,
    error: &str,
) {
    log_session.warn(
        "post_translation_optimization_validation_failed",
        "译后优化 LLM 结果校验失败，准备带反馈重试",
        json!({
            "attempt": attempt,
            "startIndex": chunk.start_index + 1,
            "endIndex": chunk.end_index + 1,
            "entryCount": chunk.entries.len(),
            "validationType": validation_type,
            "error": error,
        }),
    );
}

fn build_translation_system_prompt(settings: &AppSettings) -> String {
    let target_language = language_label(&settings.target_language);
    let custom_prompt = translation_reference(settings);

    if settings.needs_reflection_translation {
        return format!(
            r#"你是一位专业字幕翻译专家，专精于{target_language}。你的目标是输出自然、流畅、符合母语习惯的字幕译文，而不是机器翻译腔。

<context>
机器翻译常常逐词直译、忽略上下文和字幕之间的连贯性。你需要先理解整批字幕，再反思初译是否生硬，最后重写为{target_language}母语者自然会说的表达。
</context>

<terminology_and_requirements>
{custom_prompt}
</terminology_and_requirements>

<instructions>
1. 保持字幕编号一一对应，不合并、不拆分、不新增、不删除。
2. 完整保留原意、数字、专有名词、术语和语气。
3. 如果一句话跨多条字幕延续，译文要让相邻字幕读起来顺畅。
4. 输出必须是纯 JSON 对象，不要 Markdown、解释或额外文本。
</instructions>

<output_format>
{{
  "1": {{
    "initial_translation": "初译",
    "reflection": "指出不自然之处和改写理由",
    "native_translation": "最终自然译文"
  }}
}}
</output_format>"#
        );
    }

    format!(
        r#"你是一位专业字幕翻译专家，专精于{target_language}。请输出自然、流畅、易懂、符合{target_language}表达习惯的字幕译文。

<guidelines>
1. 保持字幕编号一一对应，不合并、不拆分、不新增、不删除。
2. 翻译应适合字幕阅读，简洁自然，不要逐词直译。
3. 专有名词或技术术语按上下文保留原文、音译或采用通行译法。
4. 如果最后一句不完整，不要擅自补省略号，后续字幕会继续。
5. 输出必须是纯 JSON 对象，不要 Markdown、解释或额外文本。
</guidelines>

<terminology_and_requirements>
{custom_prompt}
</terminology_and_requirements>

<output_format>
{{
  "1": "译文字幕 1",
  "2": "译文字幕 2"
}}
</output_format>"#
    )
}

fn build_post_optimization_system_prompt(settings: &AppSettings) -> String {
    let target_language = language_label(&settings.target_language);
    let custom_prompt = translation_reference(settings);

    format!(
        r#"你是一位专业字幕译后优化专家，专精于{target_language}字幕润色。请把同一批字幕行连成句子或段落理解，找出不通顺、生硬、前后衔接差或机器翻译腔的译文行，并在不改变时间轴和行数的前提下优化。

<rules>
1. 保持输入 JSON 的所有真实 key，不新增、不删除、不合并、不拆分条目，也不要把本批重新编号为 1..N。
2. 只优化译文表达、语序、衔接和术语一致性，不改变原意、数字、专有名词、交易方向或风险提示。
3. 相邻字幕跨句衔接时，允许让单行译文更自然地承接前后文，但不要把一行内容搬到另一行。
4. 如果某行已经通顺，保留该行。
5. 你可以在内部分析哪些行不通顺，但不要输出分析、理由、评分、建议或嵌套对象。
6. 输出只能是单个 JSON object，第一字符必须是 {{，最后字符必须是 }}。
7. 外层只能是字幕 key，key 和 value 都必须使用英文双引号；禁止输出数组、列表、key: value 文本、Markdown、XML 标签、代码块、解释或额外文本。
</rules>

<terminology_and_requirements>
{custom_prompt}
</terminology_and_requirements>

<output_format>
{{
  "<current_translation 中的真实 key>": "优化后的译文"
}}
</output_format>"#
    )
}

fn build_translation_user_prompt(
    entries: &BTreeMap<String, String>,
    is_reflection: bool,
    feedback: &str,
) -> String {
    let input_json = serde_json::to_string(entries).unwrap_or_else(|_| "{}".to_string());
    let output_template = build_translation_output_template(entries, is_reflection);
    let mut prompt = format!(
        "请翻译以下字幕 JSON。最终必须输出 JSON 对象，key 必须与输入完全一致。\n\
         <input_subtitle>{input_json}</input_subtitle>\n\
         <output_template>{output_template}</output_template>\n\
         <template_rule>最终答案必须复制 output_template 的完整 JSON object 外层结构和全部 key，只改 value 内容。</template_rule>\n\
         <final_answer_rule>最终答案第一字符必须是 {{，最后字符必须是 }}，且必须能被 JSON.parse 直接解析。</final_answer_rule>"
    );

    if !feedback.is_empty() {
        prompt.push_str("\n<feedback>");
        prompt.push_str(feedback);
        prompt.push_str("</feedback>");
    }

    prompt
}

fn build_translation_output_template(
    entries: &BTreeMap<String, String>,
    is_reflection: bool,
) -> String {
    if is_reflection {
        let template = entries
            .keys()
            .map(|key| {
                (
                    key.clone(),
                    json!({
                        "initial_translation": "初译",
                        "reflection": "指出不自然之处和改写理由",
                        "native_translation": "最终自然译文"
                    }),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        return Value::Object(template).to_string();
    }

    let template = entries
        .keys()
        .map(|key| (key.clone(), "译文".to_string()))
        .collect::<BTreeMap<_, _>>();

    serde_json::to_string(&template).unwrap_or_else(|_| "{}".to_string())
}

fn build_translation_json_feedback(
    entries: &BTreeMap<String, String>,
    is_reflection: bool,
    error: &str,
) -> String {
    let output_template = build_translation_output_template(entries, is_reflection);

    format!(
        "上一次结果不是有效 JSON: {error}\n请只输出完整 JSON 对象，第一字符必须是 {{，最后字符必须是 }}。请复制这个 JSON object 的外层结构和全部 key，只改 value: {output_template}"
    )
}

fn build_translation_key_feedback(
    entries: &BTreeMap<String, String>,
    is_reflection: bool,
    error: &str,
) -> String {
    let output_template = build_translation_output_template(entries, is_reflection);

    format!(
        "上一次结果 key 不匹配: {error}\n请输出完整 JSON，必须包含原始所有 key。请复制这个 JSON object 的外层结构和全部 key，只改 value: {output_template}"
    )
}

fn build_post_optimization_user_prompt(
    source_entries: &BTreeMap<String, String>,
    translated_entries: &BTreeMap<String, String>,
    feedback: &str,
) -> String {
    let source_json = serde_json::to_string(source_entries).unwrap_or_else(|_| "{}".to_string());
    let translated_json =
        serde_json::to_string(translated_entries).unwrap_or_else(|_| "{}".to_string());
    let required_keys = sorted_subtitle_keys(translated_entries);
    let required_keys_json =
        serde_json::to_string(&required_keys).unwrap_or_else(|_| "[]".to_string());
    let paragraph = translated_entries
        .values()
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    let mut prompt = format!(
        "请对以下同批字幕译文做译后优化。先把译文行组成句子或段落理解，在内部判断哪些行不通顺，然后只输出 key 完全一致、value 全部为字符串的 JSON 对象。\n\
         <required_keys>{required_keys_json}</required_keys>\n\
         <output_contract>required_keys 只用于核对，不要原样输出 required_keys 数组。最终输出必须是单个 JSON object；必须完整使用 required_keys 中的真实字幕编号；key 和 value 都必须使用英文双引号；禁止遗漏、增加、重命名 key；禁止从 1 重新编号；禁止输出数组、列表、key: value 文本、Markdown、说明文字、思考过程或 XML 标签。</output_contract>\n\
         <source_subtitle>{source_json}</source_subtitle>\n\
         <current_translation>{translated_json}</current_translation>\n\
         <output_template>{translated_json}</output_template>\n\
         <template_rule>最终答案必须复制 output_template 的完整 JSON object 外层结构和全部 key，只根据上下文改写 value；不需要优化的 value 原样保留。</template_rule>\n\
         <translation_paragraph>{paragraph}</translation_paragraph>\n\
         <final_answer_rule>最终答案第一字符必须是 {{，最后字符必须是 }}，且必须能被 JSON.parse 直接解析。</final_answer_rule>"
    );

    if !feedback.is_empty() {
        prompt.push_str("\n<feedback>");
        prompt.push_str(feedback);
        prompt.push_str("</feedback>");
    }

    prompt
}

fn build_json_parse_feedback(entries: &BTreeMap<String, String>, error: &str) -> String {
    let required_keys_json =
        serde_json::to_string(&sorted_subtitle_keys(entries)).unwrap_or_else(|_| "[]".to_string());
    let output_template = serde_json::to_string(entries).unwrap_or_else(|_| "{}".to_string());

    format!(
        "上一次结果不是有效 JSON: {error}\n请只输出一个完整 JSON 对象，第一字符必须是 {{，最后字符必须是 }}。key 和 value 都必须使用英文双引号。不要输出数组、列表、key: value 文本、Markdown、说明、思考过程或 XML 标签。必须包含这些真实 key: {required_keys_json}。请复制这个 JSON object 的外层结构，只改 value: {output_template}"
    )
}

fn build_key_mismatch_feedback(entries: &BTreeMap<String, String>, error: &str) -> String {
    let required_keys = sorted_subtitle_keys(entries);
    let required_keys_json =
        serde_json::to_string(&required_keys).unwrap_or_else(|_| "[]".to_string());
    let output_template = serde_json::to_string(entries).unwrap_or_else(|_| "{}".to_string());
    let key_hint = match (required_keys.first(), required_keys.last()) {
        (Some(first), Some(last)) if first != last => {
            format!("本批 key 从 {first} 到 {last}，不能改成 1..N。")
        }
        (Some(only), _) => format!("本批唯一 key 是 {only}，不能改成 1。"),
        _ => String::new(),
    };

    format!(
        "上一次结果 key 不匹配: {error}\n{key_hint}请输出完整 JSON，必须且只能包含这些真实 key: {required_keys_json}。请复制这个 JSON object 的外层结构，只改 value: {output_template}"
    )
}

fn translation_reference(settings: &AppSettings) -> &'static str {
    match settings.video_content_type.as_str() {
        "trading" => {
            "交易视频内容。严格保护价格、比例、方向、周期、币种、ticker、技术指标、交易术语和风险提示。long/short、support/resistance、bar、setup、price action 等可按语境保留或使用通行译法，不改变交易判断。"
        }
        _ => "通用视频内容。优先保证准确、自然、口语可读，术语按上下文保持一致。",
    }
}

fn language_label(language_code: &str) -> String {
    match language_code {
        "zh-Hans" => "简体中文".to_string(),
        "zh-Hant" => "繁体中文".to_string(),
        "en" => "英语".to_string(),
        "ja" => "日语".to_string(),
        "ko" => "韩语".to_string(),
        "fr" => "法语".to_string(),
        "de" => "德语".to_string(),
        "es" => "西班牙语".to_string(),
        "ru" => "俄语".to_string(),
        "pt" => "葡萄牙语".to_string(),
        "it" => "意大利语".to_string(),
        "ar" => "阿拉伯语".to_string(),
        "vi" => "越南语".to_string(),
        "th" => "泰语".to_string(),
        "tr" => "土耳其语".to_string(),
        other => other.to_string(),
    }
}

fn build_empty_translated_segments(
    source_segments: &[TranscriptionSegment],
) -> Vec<TranscriptionSegment> {
    source_segments
        .iter()
        .enumerate()
        .map(|(index, segment)| TranscriptionSegment {
            text: String::new(),
            start_time: segment.start_time,
            end_time: segment.end_time,
            uid: format!("target-{index}"),
            status: String::new(),
            words: Vec::new(),
        })
        .collect()
}

fn build_output_segments(
    source_segments: &[TranscriptionSegment],
    translated_segments: &[TranscriptionSegment],
    output_mode: &str,
) -> Vec<TranscriptionSegment> {
    match output_mode {
        "bilingual" => source_segments
            .iter()
            .zip(translated_segments.iter())
            .enumerate()
            .map(|(index, (source, translated))| {
                let translated_text = translated.text.trim();
                let source_text = source.text.trim();
                let text = if translated_text.is_empty() {
                    source_text.to_string()
                } else if source_text.is_empty() {
                    translated_text.to_string()
                } else {
                    format!("{source_text}\n{translated_text}")
                };

                TranscriptionSegment {
                    text,
                    start_time: source.start_time,
                    end_time: source.end_time,
                    uid: format!("out-{index}"),
                    status: "done".to_string(),
                    words: Vec::new(),
                }
            })
            .collect(),
        _ => translated_segments.to_vec(),
    }
}

fn load_subtitle_segments(path: &Path) -> Result<Vec<TranscriptionSegment>, String> {
    if !path.is_file() {
        return Err("字幕文件不存在".to_string());
    }

    let content = fs::read_to_string(path).map_err(|error| format!("无法读取字幕文件: {error}"))?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut segments = match extension.as_str() {
        "vtt" => parse_vtt(&content)?,
        "ass" => parse_ass(&content)?,
        _ => parse_srt(&content)?,
    };

    assign_segment_metadata(&mut segments, "src", "raw");
    Ok(segments)
}

fn parse_srt(content: &str) -> Result<Vec<TranscriptionSegment>, String> {
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
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

        let time_line_index = lines
            .iter()
            .position(|line| line.contains("-->"))
            .ok_or_else(|| "SRT 字幕缺少时间轴".to_string())?;
        let (start_time, end_time) = parse_time_range(lines[time_line_index])?;
        let text = lines[time_line_index + 1..].join("\n").trim().to_string();
        if text.is_empty() {
            continue;
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

    Ok(segments)
}

fn parse_vtt(content: &str) -> Result<Vec<TranscriptionSegment>, String> {
    let normalized = content
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut segments = Vec::new();
    let mut block_lines = Vec::new();

    for line in normalized.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            push_vtt_block(&mut segments, &block_lines)?;
            block_lines.clear();
        } else if !trimmed.starts_with("WEBVTT") && !trimmed.starts_with("NOTE") {
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
    if text.is_empty() {
        return Ok(());
    }

    segments.push(TranscriptionSegment {
        text,
        start_time,
        end_time,
        uid: String::new(),
        status: String::new(),
        words: Vec::new(),
    });
    Ok(())
}

fn parse_ass(content: &str) -> Result<Vec<TranscriptionSegment>, String> {
    let mut segments = Vec::new();
    let mut in_events = false;
    let mut start_index = 1usize;
    let mut end_index = 2usize;
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

        let parsed_text = clean_ass_text(text);
        if parsed_text.trim().is_empty() {
            continue;
        }

        segments.push(TranscriptionSegment {
            text: parsed_text,
            start_time: parse_ass_time(start_text)?,
            end_time: parse_ass_time(end_text)?,
            uid: String::new(),
            status: String::new(),
            words: Vec::new(),
        });
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

fn build_text_chunks(segments: &[TranscriptionSegment], batch_size: usize) -> Vec<TextChunk> {
    segments
        .chunks(batch_size.max(1))
        .enumerate()
        .map(|(chunk_index, chunk)| {
            let start_index = chunk_index * batch_size.max(1);
            let entries = chunk
                .iter()
                .enumerate()
                .map(|(offset, segment)| {
                    ((start_index + offset + 1).to_string(), segment.text.clone())
                })
                .collect::<BTreeMap<_, _>>();

            TextChunk {
                start_index,
                end_index: start_index + chunk.len().saturating_sub(1),
                entries,
            }
        })
        .collect()
}

fn build_translation_chunks(
    source_segments: &[TranscriptionSegment],
    batch_size: usize,
) -> Vec<TranslationChunk> {
    source_segments
        .chunks(batch_size.max(1))
        .enumerate()
        .map(|(chunk_index, chunk)| {
            let start_index = chunk_index * batch_size.max(1);
            let entries = chunk
                .iter()
                .enumerate()
                .map(|(offset, segment)| {
                    ((start_index + offset + 1).to_string(), segment.text.clone())
                })
                .collect::<BTreeMap<_, _>>();
            TranslationChunk {
                start_index,
                end_index: start_index + chunk.len().saturating_sub(1),
                entries,
            }
        })
        .collect()
}

fn copy_source_range_to_target(
    source_segments: &[TranscriptionSegment],
    translated_segments: &mut [TranscriptionSegment],
    chunk: &TranslationChunk,
) {
    for index in chunk.start_index..=chunk.end_index {
        if let (Some(source), Some(target)) = (
            source_segments.get(index),
            translated_segments.get_mut(index),
        ) {
            target.text = source.text.clone();
        }
    }
}

fn parse_translation_response(
    text: &str,
    is_reflection: bool,
) -> Result<BTreeMap<String, String>, String> {
    if !is_reflection {
        return parse_json_text_map(text);
    }

    let candidates = extract_json_object_candidates(text);
    if candidates.is_empty() {
        return Err("未找到 JSON 对象开始符".to_string());
    }

    let mut last_error = String::new();
    for json_text in candidates.iter().rev() {
        match parse_reflection_translation_candidate(json_text) {
            Ok(result) => return Ok(result),
            Err(error) => last_error = error,
        }
    }

    Err(last_error)
}

fn parse_reflection_translation_candidate(
    json_text: &str,
) -> Result<BTreeMap<String, String>, String> {
    let value = serde_json::from_str::<Value>(json_text)
        .map_err(|error| format!("JSON 解析失败: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "LLM 返回内容不是 JSON 对象".to_string())?;
    let mut result = BTreeMap::new();

    for (key, value) in object {
        if let Some(text) = value.as_str() {
            result.insert(key.clone(), text.to_string());
            continue;
        }

        let text = value
            .get("native_translation")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("key {key} 缺少 native_translation 字符串字段"))?;
        result.insert(key.clone(), text.to_string());
    }

    Ok(result)
}

fn parse_json_text_map(text: &str) -> Result<BTreeMap<String, String>, String> {
    let candidates = extract_json_object_candidates(text);
    if candidates.is_empty() {
        return Err("未找到 JSON 对象开始符".to_string());
    }

    let mut last_error = String::new();
    for json_text in candidates.iter().rev() {
        match parse_json_text_map_candidate(json_text) {
            Ok(result) => return Ok(result),
            Err(error) => last_error = error,
        }
    }

    Err(last_error)
}

fn parse_json_text_map_candidate(json_text: &str) -> Result<BTreeMap<String, String>, String> {
    let value = serde_json::from_str::<Value>(json_text)
        .map_err(|error| format!("JSON 解析失败: {error}"))?;
    parse_json_text_map_value(&value)
}

fn parse_json_text_map_value(value: &Value) -> Result<BTreeMap<String, String>, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "LLM 返回内容不是 JSON 对象".to_string())?;

    if let Ok(result) = parse_text_map_object(object) {
        return Ok(result);
    }

    let wrapper_fields = [
        "translations",
        "translated_subtitles",
        "optimized_translations",
        "optimized_subtitles",
        "optimized",
        "results",
        "result",
        "subtitles",
        "captions",
        "items",
        "data",
        "output",
    ];

    let mut last_error = "未找到字幕编号 key".to_string();
    for field in wrapper_fields {
        let Some(nested) = object.get(field) else {
            continue;
        };

        match nested {
            Value::Object(nested_object) => match parse_text_map_object(nested_object) {
                Ok(result) => return Ok(result),
                Err(error) => last_error = format!("字段 {field} 解析失败: {error}"),
            },
            Value::Array(items) => match parse_text_map_array(items) {
                Ok(result) => return Ok(result),
                Err(error) => last_error = format!("字段 {field} 解析失败: {error}"),
            },
            _ => {
                last_error = format!("字段 {field} 不是 JSON 对象或数组");
            }
        }
    }

    for (field, nested) in object {
        match nested {
            Value::Object(nested_object) => {
                if !looks_like_subtitle_map(nested_object) {
                    continue;
                }

                match parse_text_map_object(nested_object) {
                    Ok(result) => return Ok(result),
                    Err(error) => last_error = format!("字段 {field} 解析失败: {error}"),
                }
            }
            Value::Array(items) => {
                if !looks_like_subtitle_array(items) {
                    continue;
                }

                match parse_text_map_array(items) {
                    Ok(result) => return Ok(result),
                    Err(error) => last_error = format!("字段 {field} 解析失败: {error}"),
                }
            }
            _ => {}
        }
    }

    Err(last_error)
}

fn parse_text_map_object(
    object: &serde_json::Map<String, Value>,
) -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::new();
    let mut last_error = String::new();

    for (key, value) in object {
        let Some(normalized_key) = normalize_subtitle_key(key) else {
            continue;
        };

        match extract_subtitle_text_value(value) {
            Ok(text) => {
                result.insert(normalized_key, text);
            }
            Err(error) => {
                last_error = format!("key {key} {error}");
            }
        }
    }

    if !result.is_empty() {
        return Ok(result);
    }

    if last_error.is_empty() {
        Err("未找到字幕编号 key".to_string())
    } else {
        Err(last_error)
    }
}

fn parse_text_map_array(items: &[Value]) -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::new();
    let mut last_error = String::new();

    for item in items {
        let Some(object) = item.as_object() else {
            last_error = "数组项不是 JSON 对象".to_string();
            continue;
        };
        let Some(key) = extract_subtitle_key_from_object(object) else {
            last_error = "数组项缺少字幕编号字段".to_string();
            continue;
        };

        match extract_subtitle_text_value(item) {
            Ok(text) => {
                result.insert(key, text);
            }
            Err(error) => {
                last_error = error;
            }
        }
    }

    if !result.is_empty() {
        return Ok(result);
    }

    if last_error.is_empty() {
        Err("未找到字幕编号 key".to_string())
    } else {
        Err(last_error)
    }
}

fn extract_subtitle_text_value(value: &Value) -> Result<String, String> {
    if let Some(text) = value.as_str() {
        return Ok(text.to_string());
    }

    let object = value
        .as_object()
        .ok_or_else(|| "的值不是字符串或对象".to_string())?;
    let text_fields = [
        "native_translation",
        "optimized_translation",
        "optimized_text",
        "optimized",
        "translation",
        "translated_text",
        "text",
        "target_text",
        "target",
        "final_translation",
        "final",
        "revised_translation",
        "polished_translation",
        "result",
        "output",
        "value",
        "content",
        "subtitle",
        "caption",
        "译文",
        "优化译文",
        "优化后的译文",
    ];

    for field in text_fields {
        if let Some(text) = object.get(field).and_then(Value::as_str) {
            return Ok(text.to_string());
        }
    }

    let string_values = object
        .iter()
        .filter(|(key, value)| {
            !matches!(
                key.as_str(),
                "id" | "index"
                    | "key"
                    | "line"
                    | "line_number"
                    | "number"
                    | "analysis"
                    | "reflection"
                    | "reason"
                    | "reasoning"
                    | "comment"
                    | "comments"
                    | "explanation"
                    | "score"
            ) && value.as_str().is_some()
        })
        .map(|(_, value)| value.as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>();

    if string_values.len() == 1 {
        return Ok(string_values[0].clone());
    }

    Err("缺少译文字段字符串".to_string())
}

fn extract_subtitle_key_from_object(object: &serde_json::Map<String, Value>) -> Option<String> {
    ["id", "index", "key", "line", "line_number", "number"]
        .iter()
        .find_map(|field| {
            object.get(*field).and_then(|value| {
                value
                    .as_str()
                    .and_then(normalize_subtitle_key)
                    .or_else(|| value.as_u64().map(|number| number.to_string()))
            })
        })
}

fn looks_like_subtitle_map(object: &serde_json::Map<String, Value>) -> bool {
    object
        .keys()
        .any(|key| normalize_subtitle_key(key).is_some())
}

fn looks_like_subtitle_array(items: &[Value]) -> bool {
    items.iter().any(|item| {
        item.as_object()
            .and_then(extract_subtitle_key_from_object)
            .is_some()
    })
}

fn normalize_subtitle_key(key: &str) -> Option<String> {
    let trimmed = key.trim().trim_start_matches('#');
    let numeric = trimmed.parse::<usize>().ok()?;

    if numeric == 0 {
        None
    } else {
        Some(numeric.to_string())
    }
}

fn sorted_subtitle_keys(entries: &BTreeMap<String, String>) -> Vec<String> {
    let mut keys = entries
        .keys()
        .filter_map(|key| {
            key.parse::<usize>()
                .ok()
                .map(|number| (number, key.clone()))
        })
        .collect::<Vec<_>>();
    keys.sort_by_key(|(number, _)| *number);
    keys.into_iter().map(|(_, key)| key).collect()
}

fn extract_json_value_candidates(text: &str) -> Vec<&str> {
    let mut candidates = Vec::new();
    let mut start = None;
    let mut expected_closers = Vec::new();
    let mut in_string = false;
    let mut escaped = false;

    for (index, character) in text.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }

            match character {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match character {
            '"' => in_string = true,
            '{' => {
                if expected_closers.is_empty() {
                    start = Some(index);
                }
                expected_closers.push('}');
            }
            '[' => {
                if expected_closers.is_empty() {
                    start = Some(index);
                }
                expected_closers.push(']');
            }
            '}' | ']' => {
                let Some(expected) = expected_closers.pop() else {
                    continue;
                };

                if character != expected {
                    expected_closers.clear();
                    start = None;
                    continue;
                }

                if expected_closers.is_empty() {
                    if let Some(start_index) = start.take() {
                        candidates.push(&text[start_index..=index]);
                    }
                }
            }
            _ => {}
        }
    }

    candidates
}

fn extract_json_object_candidates(text: &str) -> Vec<&str> {
    extract_json_value_candidates(text)
        .into_iter()
        .filter(|candidate| candidate.trim_start().starts_with('{'))
        .collect()
}

fn validate_or_remap_relative_keys(
    expected: &BTreeMap<String, String>,
    actual: BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, String> {
    if validate_keys(expected, &actual).is_ok() {
        return Ok(actual);
    }

    let original_error = match validate_keys(expected, &actual) {
        Ok(()) => return Ok(actual),
        Err(error) => error,
    };
    let expected_keys = sorted_subtitle_keys(expected);
    if expected_keys.len() != actual.len() || expected_keys.is_empty() {
        return Err(original_error);
    }

    let actual_key_numbers = actual
        .keys()
        .filter_map(|key| key.parse::<usize>().ok())
        .collect::<HashSet<_>>();
    let is_complete_relative_range = actual_key_numbers.len() == actual.len()
        && (1..=actual.len()).all(|key| actual_key_numbers.contains(&key));

    if !is_complete_relative_range {
        return Err(original_error);
    }

    let mut remapped = BTreeMap::new();
    for (offset, expected_key) in expected_keys.into_iter().enumerate() {
        let relative_key = (offset + 1).to_string();
        let Some(text) = actual.get(&relative_key) else {
            return Err(original_error);
        };
        remapped.insert(expected_key, text.clone());
    }

    Ok(remapped)
}

fn validate_keys(
    expected: &BTreeMap<String, String>,
    actual: &BTreeMap<String, String>,
) -> Result<(), String> {
    let expected_keys = expected.keys().cloned().collect::<HashSet<_>>();
    let actual_keys = actual.keys().cloned().collect::<HashSet<_>>();

    if expected_keys == actual_keys {
        return Ok(());
    }

    let missing = expected_keys
        .difference(&actual_keys)
        .cloned()
        .collect::<Vec<_>>();
    let extra = actual_keys
        .difference(&expected_keys)
        .cloned()
        .collect::<Vec<_>>();

    Err(format!("缺失 key: {:?}; 多余 key: {:?}", missing, extra))
}

fn mark_range_status(
    segments: &mut [TranscriptionSegment],
    start_index: usize,
    end_index: usize,
    status: &str,
) {
    if start_index >= segments.len() {
        return;
    }

    let end_index = end_index.min(segments.len().saturating_sub(1));
    for segment in &mut segments[start_index..=end_index] {
        segment.status = status.to_string();
    }
}

fn assign_segment_metadata(segments: &mut [TranscriptionSegment], uid_prefix: &str, status: &str) {
    for (index, segment) in segments.iter_mut().enumerate() {
        segment.uid = format!("{uid_prefix}-{index}");
        segment.status = status.to_string();
    }
}

fn mark_segments_status(segments: &mut [TranscriptionSegment], status: &str) {
    for segment in segments {
        segment.status = status.to_string();
    }
}

fn build_processing_warnings(stage: &str, failed_count: usize, unit_name: &str) -> Vec<String> {
    if failed_count == 0 {
        Vec::new()
    } else {
        vec![format!(
            "{stage}部分失败，已保留 {failed_count} 个{unit_name}，详情已写入日志"
        )]
    }
}

fn active_ai_work_count(settings: &AppSettings) -> usize {
    settings.translation_thread_count.max(1) as usize
}

fn stage_progress(start: u8, end: u8, completed: usize, total: usize) -> u8 {
    let span = end.saturating_sub(start) as usize;
    let scaled = if total == 0 {
        span
    } else {
        span.saturating_mul(completed) / total
    };
    start.saturating_add(scaled as u8).min(end)
}

fn overall_progress(stages: &SubtitleTranslationStageProgress) -> u8 {
    let visible = [
        stages.subtitle_translation.as_ref(),
        stages.post_translation_optimization.as_ref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    if visible.is_empty() {
        return 0;
    }

    let total = visible
        .iter()
        .map(|stage| stage.progress as u16)
        .sum::<u16>();
    (total / visible.len() as u16).min(100) as u8
}

fn emit_progress_event(
    app: &AppHandle,
    progress: u8,
    message: &str,
    stage_progress: Option<SubtitleTranslationStageProgress>,
    revision: Option<u64>,
    source_segments: Option<Vec<TranscriptionSegment>>,
    translated_segments: Option<Vec<TranscriptionSegment>>,
    warnings: &[String],
) {
    let _ = app.emit(
        PROGRESS_EVENT,
        SubtitleTranslationProgress {
            progress,
            message: message.to_string(),
            stage_progress,
            revision,
            source_segments,
            translated_segments,
            warnings: warnings.to_vec(),
        },
    );
}

fn estimate_max_output_tokens(text: &str) -> u32 {
    ((text.chars().count() as u32) * 6).clamp(1024, 12000)
}

fn log_translation_settings(log_session: &LogSession, settings: &AppSettings) {
    let llm_config = settings.llm_configs.get(&settings.selected_llm_service);
    log_session.info(
        "settings_loaded",
        "已加载字幕翻译相关设置",
        json!({
            "translationService": &settings.translation_service,
            "reflectionEnabled": settings.needs_reflection_translation,
            "translationBatchSize": settings.translation_batch_size,
            "translationThreadCount": settings.translation_thread_count,
            "videoContentType": &settings.video_content_type,
            "outputMode": &settings.output_mode,
            "subtitleTranslationEnabled": settings.is_subtitle_translation_enabled,
            "postTranslationOptimizationEnabled": settings.is_post_translation_optimization_enabled,
            "targetLanguage": &settings.target_language,
            "selectedLlmService": &settings.selected_llm_service,
            "llmBaseUrl": llm_config.map(|config| config.base_url.as_str()).unwrap_or(""),
            "llmModel": llm_config.map(|config| config.model.as_str()).unwrap_or(""),
            "llmReasoningEffort": llm_config
                .map(|config| config.reasoning_effort.as_str())
                .unwrap_or(""),
            "llmStreaming": llm_config.map(|config| config.is_streaming).unwrap_or(false),
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_srt_subtitle() {
        let srt = "1\n00:00:01,000 --> 00:00:02,500\nHello world\n\n2\n00:00:03,000 --> 00:00:04,000\nNext\n";
        let segments = parse_srt(srt).unwrap();

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].start_time, 1000);
        assert_eq!(segments[0].end_time, 2500);
        assert_eq!(segments[0].text, "Hello world");
    }

    #[test]
    fn parses_vtt_subtitle() {
        let vtt = "WEBVTT\n\ncue-1\n00:00:01.000 --> 00:00:02.500\nHello world\n";
        let segments = parse_vtt(vtt).unwrap();

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].start_time, 1000);
        assert_eq!(segments[0].end_time, 2500);
    }

    #[test]
    fn parses_ass_dialogues() {
        let ass = "[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:02.50,Default,,0,0,0,,{\\i1}Hello\\Nworld\n";
        let segments = parse_ass(ass).unwrap();

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Hello\nworld");
        assert_eq!(segments[0].start_time, 1000);
        assert_eq!(segments[0].end_time, 2500);
    }

    #[test]
    fn parses_reflection_translation_native_field() {
        let parsed = parse_translation_response(
            r#"{"1":{"initial_translation":"A","reflection":"B","native_translation":"自然译文"}}"#,
            true,
        )
        .unwrap();

        assert_eq!(parsed.get("1"), Some(&"自然译文".to_string()));
    }

    #[test]
    fn parses_text_map_with_optimized_object_values() {
        let parsed = parse_json_text_map(
            r#"{"1":{"analysis":"略","optimized_translation":"优化译文"},"2":{"text":"保留译文"}}"#,
        )
        .unwrap();

        assert_eq!(parsed.get("1"), Some(&"优化译文".to_string()));
        assert_eq!(parsed.get("2"), Some(&"保留译文".to_string()));
    }

    #[test]
    fn parses_text_map_from_common_wrapper_field() {
        let parsed =
            parse_json_text_map(r#"{"optimized_translations":{"1":"优化译文","2":"下一句"}}"#)
                .unwrap();

        assert_eq!(parsed.get("1"), Some(&"优化译文".to_string()));
        assert_eq!(parsed.get("2"), Some(&"下一句".to_string()));
    }

    #[test]
    fn parses_text_map_from_array_items() {
        let parsed = parse_json_text_map(
            r#"{"items":[{"index":1,"optimized_translation":"优化译文"},{"id":"2","translation":"下一句"}]}"#,
        )
        .unwrap();

        assert_eq!(parsed.get("1"), Some(&"优化译文".to_string()));
        assert_eq!(parsed.get("2"), Some(&"下一句".to_string()));
    }

    #[test]
    fn remaps_complete_relative_keys_to_expected_chunk_keys() {
        let expected = BTreeMap::from([
            ("31".to_string(), "当前译文 31".to_string()),
            ("32".to_string(), "当前译文 32".to_string()),
            ("33".to_string(), "当前译文 33".to_string()),
        ]);
        let actual = BTreeMap::from([
            ("1".to_string(), "优化译文 31".to_string()),
            ("2".to_string(), "优化译文 32".to_string()),
            ("3".to_string(), "优化译文 33".to_string()),
        ]);

        let remapped = validate_or_remap_relative_keys(&expected, actual).unwrap();

        assert_eq!(remapped.get("31"), Some(&"优化译文 31".to_string()));
        assert_eq!(remapped.get("32"), Some(&"优化译文 32".to_string()));
        assert_eq!(remapped.get("33"), Some(&"优化译文 33".to_string()));
    }

    #[test]
    fn rejects_incomplete_relative_keys() {
        let expected = BTreeMap::from([
            ("31".to_string(), "当前译文 31".to_string()),
            ("32".to_string(), "当前译文 32".to_string()),
            ("33".to_string(), "当前译文 33".to_string()),
        ]);
        let actual = BTreeMap::from([
            ("1".to_string(), "优化译文 31".to_string()),
            ("2".to_string(), "优化译文 32".to_string()),
        ]);

        let error = validate_or_remap_relative_keys(&expected, actual).unwrap_err();

        assert!(error.contains("缺失 key"));
        assert!(error.contains("多余 key"));
    }

    #[test]
    fn post_optimization_prompt_preserves_actual_keys() {
        let source = BTreeMap::from([
            ("31".to_string(), "source 31".to_string()),
            ("32".to_string(), "source 32".to_string()),
        ]);
        let translated = BTreeMap::from([
            ("31".to_string(), "译文 31".to_string()),
            ("32".to_string(), "译文 32".to_string()),
        ]);

        let prompt = build_post_optimization_user_prompt(&source, &translated, "");

        assert!(prompt.contains(r#"<required_keys>["31","32"]</required_keys>"#));
        assert!(prompt.contains("禁止从 1 重新编号"));
        assert!(prompt.contains("禁止输出数组"));
        assert!(prompt.contains("key 和 value 都必须使用英文双引号"));
        assert!(prompt.contains("<output_template>"));
        assert!(prompt.contains("复制 output_template 的完整 JSON object 外层结构和全部 key"));
        assert!(prompt.contains(r#""31":"译文 31""#));
        assert!(!prompt.contains(r#""1":"优化后的译文 1""#));
    }
}
