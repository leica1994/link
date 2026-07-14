use crate::app_log::AppLogger;
use crate::command_utils::create_command;
use crate::settings::SettingsStore;
use crate::subtitle_export::serialize_styled_ass;
use crate::subtitle_style::{get_selected_subtitle_style, SubtitleStyle};
use crate::transcription::TranscriptionSegment;
use encoding_rs::GBK;
use html_escape::decode_html_entities;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::http::{
    header::{ACCEPT_RANGES, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE},
    Request, Response, StatusCode,
};
use tauri::{AppHandle, Emitter, Manager};
use tempfile::TempDir;
use uuid::Uuid;

const PROXY_PROGRESS_EVENT: &str = "subtitle-review-proxy-progress";
const EXPORT_PROGRESS_EVENT: &str = "subtitle-review-export-progress";
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "mkv", "avi", "flv", "wmv", "webm", "m4v"];
const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "vtt", "ass"];
const STREAM_MAX_CHUNK_BYTES: u64 = 2 * 1024 * 1024;
const DEFAULT_EVENT_FORMAT: [&str; 10] = [
    "Layer", "Start", "End", "Style", "Name", "MarginL", "MarginR", "MarginV", "Effect", "Text",
];

#[derive(Clone, Default)]
pub struct SubtitleReviewRuntime {
    inner: Arc<Mutex<RuntimeState>>,
}

#[derive(Default)]
struct RuntimeState {
    sessions: HashMap<String, ReviewSession>,
    proxy_cancels: HashMap<String, Arc<AtomicBool>>,
    export_jobs: HashMap<String, ExportControl>,
}

struct ExportControl {
    session_id: String,
    cancel: Arc<AtomicBool>,
}

struct ReviewSession {
    video_path: PathBuf,
    video: ReviewVideoMetadata,
    document: ReviewDocument,
    style: Option<SubtitleStyle>,
    cues: Vec<ReviewCue>,
    revision: u64,
    proxy_temp_dir: Option<TempDir>,
    proxy_path: Option<PathBuf>,
}

pub fn subtitle_review_stream_response(
    runtime: &SubtitleReviewRuntime,
    request: Request<Vec<u8>>,
) -> Result<Response<Vec<u8>>, String> {
    if request.method().as_str() == "OPTIONS" {
        return Response::builder()
            .status(StatusCode::NO_CONTENT)
            .header("access-control-allow-origin", "*")
            .header("access-control-allow-methods", "GET, HEAD, OPTIONS")
            .header("access-control-allow-headers", "Range")
            .header(
                "access-control-expose-headers",
                "Accept-Ranges, Content-Length, Content-Range, Content-Type",
            )
            .body(Vec::new())
            .map_err(|error| format!("无法创建视频预检响应: {error}"));
    }

    let session_id = request.uri().path().trim_matches('/');
    if session_id.is_empty() || session_id.contains('/') {
        return stream_error_response(StatusCode::NOT_FOUND, "字幕审核视频不存在");
    }

    let video_path = {
        let state = runtime
            .inner
            .lock()
            .map_err(|error| format!("字幕审核状态锁定失败: {error}"))?;
        let session = state
            .sessions
            .get(session_id)
            .ok_or_else(|| "字幕审核会话不存在或已结束".to_string())?;
        session
            .proxy_path
            .as_ref()
            .filter(|path| path.is_file())
            .unwrap_or(&session.video_path)
            .clone()
    };

    let mut file =
        fs::File::open(&video_path).map_err(|error| format!("无法打开字幕审核视频: {error}"))?;
    let file_length = file
        .metadata()
        .map_err(|error| format!("无法读取字幕审核视频信息: {error}"))?
        .len();
    if file_length == 0 {
        return stream_error_response(StatusCode::NOT_FOUND, "字幕审核视频为空");
    }

    let requested_range = request
        .headers()
        .get("range")
        .and_then(|value| value.to_str().ok());
    let (start, requested_end) = match requested_range {
        Some(value) => match parse_stream_range(value, file_length) {
            Ok(range) => range,
            Err(()) => {
                return Response::builder()
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header(CONTENT_RANGE, format!("bytes */{file_length}"))
                    .header(ACCEPT_RANGES, "bytes")
                    .header("access-control-allow-origin", "*")
                    .header("access-control-allow-headers", "Range")
                    .header(
                        "access-control-expose-headers",
                        "Accept-Ranges, Content-Length, Content-Range, Content-Type",
                    )
                    .body(Vec::new())
                    .map_err(|error| format!("无法创建视频范围错误响应: {error}"));
            }
        },
        None => (0, file_length - 1),
    };
    let end = requested_end
        .min(file_length - 1)
        .min(start.saturating_add(STREAM_MAX_CHUNK_BYTES - 1));
    let bytes_to_read = end - start + 1;

    file.seek(SeekFrom::Start(start))
        .map_err(|error| format!("无法定位字幕审核视频: {error}"))?;
    let mut body = vec![0_u8; bytes_to_read as usize];
    file.read_exact(&mut body)
        .map_err(|error| format!("无法读取字幕审核视频: {error}"))?;

    let is_partial = requested_range.is_some() || start > 0 || end + 1 < file_length;
    let mut response = Response::builder()
        .status(if is_partial {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        })
        .header(CONTENT_TYPE, video_content_type(&video_path))
        .header(CONTENT_LENGTH, bytes_to_read.to_string())
        .header(ACCEPT_RANGES, "bytes")
        .header(CACHE_CONTROL, "no-store")
        .header("access-control-allow-origin", "*")
        .header("access-control-allow-headers", "Range")
        .header(
            "access-control-expose-headers",
            "Accept-Ranges, Content-Length, Content-Range, Content-Type",
        );
    if is_partial {
        response = response.header(CONTENT_RANGE, format!("bytes {start}-{end}/{file_length}"));
    }
    response
        .body(body)
        .map_err(|error| format!("无法创建字幕审核视频响应: {error}"))
}

fn parse_stream_range(value: &str, file_length: u64) -> Result<(u64, u64), ()> {
    let range = value.strip_prefix("bytes=").ok_or(())?;
    if range.contains(',') {
        return Err(());
    }
    let (start, end) = range.split_once('-').ok_or(())?;
    if start.is_empty() {
        let suffix_length = end.parse::<u64>().map_err(|_| ())?;
        if suffix_length == 0 {
            return Err(());
        }
        return Ok((file_length.saturating_sub(suffix_length), file_length - 1));
    }
    let start = start.parse::<u64>().map_err(|_| ())?;
    if start >= file_length {
        return Err(());
    }
    let end = if end.is_empty() {
        file_length - 1
    } else {
        end.parse::<u64>().map_err(|_| ())?.min(file_length - 1)
    };
    if end < start {
        return Err(());
    }
    Ok((start, end))
}

fn video_content_type(path: &Path) -> &'static str {
    match file_extension(path).as_deref() {
        Some("webm") => "video/webm",
        Some("mov") => "video/quicktime",
        Some("mkv") => "video/x-matroska",
        Some("avi") => "video/x-msvideo",
        _ => "video/mp4",
    }
}

fn stream_error_response(status: StatusCode, message: &str) -> Result<Response<Vec<u8>>, String> {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(CACHE_CONTROL, "no-store")
        .header("access-control-allow-origin", "*")
        .header("access-control-allow-headers", "Range")
        .body(message.as_bytes().to_vec())
        .map_err(|error| format!("无法创建字幕审核视频错误响应: {error}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReviewSubtitleFormat {
    Srt,
    Vtt,
    Ass,
}

impl ReviewSubtitleFormat {
    fn from_path(path: &Path) -> Result<Self, String> {
        match file_extension(path).as_deref() {
            Some("srt") => Ok(Self::Srt),
            Some("vtt") => Ok(Self::Vtt),
            Some("ass") => Ok(Self::Ass),
            _ => Err("请选择 SRT、VTT 或 ASS 字幕文件".to_string()),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Srt => "srt",
            Self::Vtt => "vtt",
            Self::Ass => "ass",
        }
    }
}

enum ReviewDocument {
    Plain,
    Ass(AssDocument),
}

struct AssDocument {
    template: Vec<AssTemplateLine>,
    cue_meta: HashMap<String, AssCueMeta>,
    original_ids: HashSet<String>,
    new_event_format: Vec<String>,
    default_style: String,
}

enum AssTemplateLine {
    Raw(String),
    Cue(String),
    NewCueMarker,
}

struct AssCueMeta {
    format: Vec<String>,
    values: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareSubtitleReviewRequest {
    pub video_path: String,
    pub subtitle_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareSubtitleReviewResult {
    pub session_id: String,
    pub video: ReviewVideoMetadata,
    pub subtitle_format: String,
    pub style_name: String,
    pub cues: Vec<ReviewCue>,
    pub revision: u64,
    pub ass_content: String,
    pub validation: ReviewValidation,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewVideoMetadata {
    pub path: String,
    pub preview_path: String,
    pub file_name: String,
    pub duration_ms: u64,
    pub width: u32,
    pub height: u32,
    pub video_codec: String,
    pub audio_codec: String,
    pub file_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewCue {
    pub id: String,
    pub start_time: u64,
    pub end_time: u64,
    pub text: String,
    pub raw_text: String,
    pub text_mode: String,
    pub style_name: String,
    pub layer: String,
    pub actor: String,
    pub margin_l: String,
    pub margin_r: String,
    pub margin_v: String,
    pub effect: String,
    pub has_inline_tags: bool,
    pub source_order: u32,
    pub is_new: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSubtitleReviewRequest {
    pub session_id: String,
    pub revision: u64,
    pub cues: Vec<ReviewCue>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSubtitleReviewResult {
    pub revision: u64,
    pub ass_content: String,
    pub validation: ReviewValidation,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewValidation {
    pub can_export: bool,
    pub issues: Vec<ReviewValidationIssue>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewValidationIssue {
    pub cue_id: Option<String>,
    pub level: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleReviewProxyProgress {
    pub session_id: String,
    pub progress: u8,
    pub message: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleReviewProxyResult {
    pub session_id: String,
    pub preview_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleReviewExportRequest {
    pub session_id: String,
    pub job_id: String,
    pub revision: u64,
    pub cues: Vec<ReviewCue>,
    pub output_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleReviewExportProgress {
    pub session_id: String,
    pub job_id: String,
    pub progress: u8,
    pub message: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleReviewExportResult {
    pub session_id: String,
    pub job_id: String,
    pub output_path: String,
    pub duration_ms: u64,
}

#[tauri::command]
pub async fn prepare_subtitle_review(
    app: AppHandle,
    runtime: tauri::State<'_, SubtitleReviewRuntime>,
    settings_store: tauri::State<'_, SettingsStore>,
    app_logger: tauri::State<'_, AppLogger>,
    request: PrepareSubtitleReviewRequest,
) -> Result<PrepareSubtitleReviewResult, String> {
    let runtime = runtime.inner().clone();
    let logger = app_logger.inner().clone();
    let settings = settings_store.load()?;
    let style = get_selected_subtitle_style(&settings_store, &settings.selected_subtitle_style_id)?;

    tauri::async_runtime::spawn_blocking(move || {
        prepare_subtitle_review_blocking(app, runtime, logger, request, style)
    })
    .await
    .map_err(|error| format!("字幕审核会话启动失败: {error}"))?
}

fn prepare_subtitle_review_blocking(
    app: AppHandle,
    runtime: SubtitleReviewRuntime,
    logger: AppLogger,
    request: PrepareSubtitleReviewRequest,
    selected_style: SubtitleStyle,
) -> Result<PrepareSubtitleReviewResult, String> {
    let video_path = canonical_input_path(&request.video_path, VIDEO_EXTENSIONS, "视频")?;
    let subtitle_path = canonical_input_path(&request.subtitle_path, SUBTITLE_EXTENSIONS, "字幕")?;
    let subtitle_format = ReviewSubtitleFormat::from_path(&subtitle_path)?;
    let video = probe_video_metadata(&video_path)?;
    let content = decode_subtitle_file(&subtitle_path)?;
    let (document, cues, warnings) = match subtitle_format {
        ReviewSubtitleFormat::Ass => {
            let (document, cues, parse_warnings) = parse_ass_document(&content)?;
            (ReviewDocument::Ass(document), cues, parse_warnings)
        }
        ReviewSubtitleFormat::Srt => (ReviewDocument::Plain, parse_srt_cues(&content)?, Vec::new()),
        ReviewSubtitleFormat::Vtt => (ReviewDocument::Plain, parse_vtt_cues(&content)?, Vec::new()),
    };

    if cues.is_empty() {
        return Err("字幕文件中没有可审核的字幕条目".to_string());
    }

    app.asset_protocol_scope()
        .allow_file(&video_path)
        .map_err(|error| format!("无法授权视频预览: {error}"))?;

    let session_id = Uuid::new_v4().to_string();
    let style_name = if subtitle_format == ReviewSubtitleFormat::Ass {
        "文件内样式".to_string()
    } else {
        selected_style.name.clone()
    };
    let validation = validate_cues(&cues, video.duration_ms);
    let ass_content = serialize_review_document(
        &document,
        &cues,
        if subtitle_format == ReviewSubtitleFormat::Ass {
            None
        } else {
            Some(&selected_style)
        },
    )?;

    let session = ReviewSession {
        video_path: video_path.clone(),
        video: video.clone(),
        document,
        style: (subtitle_format != ReviewSubtitleFormat::Ass).then_some(selected_style),
        cues: cues.clone(),
        revision: 0,
        proxy_temp_dir: None,
        proxy_path: None,
    };

    runtime
        .inner
        .lock()
        .map_err(|error| format!("字幕审核状态锁定失败: {error}"))?
        .sessions
        .insert(session_id.clone(), session);

    logger.info(
        "subtitle_review",
        "session_prepared",
        "字幕审核会话已准备",
        json!({
            "sessionId": &session_id,
            "videoPath": video_path.to_string_lossy(),
            "subtitlePath": subtitle_path.to_string_lossy(),
            "subtitleFormat": subtitle_format.as_str(),
            "cueCount": cues.len(),
        }),
    );

    Ok(PrepareSubtitleReviewResult {
        session_id,
        video,
        subtitle_format: subtitle_format.as_str().to_string(),
        style_name,
        cues,
        revision: 0,
        ass_content,
        validation,
        warnings,
    })
}

#[tauri::command]
pub fn update_subtitle_review(
    runtime: tauri::State<'_, SubtitleReviewRuntime>,
    request: UpdateSubtitleReviewRequest,
) -> Result<UpdateSubtitleReviewResult, String> {
    let mut state = runtime
        .inner
        .lock()
        .map_err(|error| format!("字幕审核状态锁定失败: {error}"))?;
    let session = state
        .sessions
        .get_mut(request.session_id.trim())
        .ok_or_else(|| "字幕审核会话不存在或已结束".to_string())?;

    if request.revision < session.revision {
        let ass_content =
            serialize_review_document(&session.document, &session.cues, session.style.as_ref())?;
        return Ok(UpdateSubtitleReviewResult {
            revision: session.revision,
            ass_content,
            validation: validate_cues(&session.cues, session.video.duration_ms),
        });
    }

    validate_cue_ids(&request.cues)?;
    let validation = validate_cues(&request.cues, session.video.duration_ms);
    let ass_content =
        serialize_review_document(&session.document, &request.cues, session.style.as_ref())?;
    session.cues = request.cues;
    session.revision = request.revision;

    Ok(UpdateSubtitleReviewResult {
        revision: request.revision,
        ass_content,
        validation,
    })
}

#[tauri::command]
pub async fn prepare_subtitle_review_proxy(
    app: AppHandle,
    runtime: tauri::State<'_, SubtitleReviewRuntime>,
    app_logger: tauri::State<'_, AppLogger>,
    session_id: String,
) -> Result<SubtitleReviewProxyResult, String> {
    let runtime = runtime.inner().clone();
    let logger = app_logger.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        prepare_proxy_blocking(app, runtime, logger, session_id)
    })
    .await
    .map_err(|error| format!("预览代理任务异常: {error}"))?
}

fn prepare_proxy_blocking(
    app: AppHandle,
    runtime: SubtitleReviewRuntime,
    logger: AppLogger,
    session_id: String,
) -> Result<SubtitleReviewProxyResult, String> {
    let session_id = session_id.trim().to_string();
    let (video_path, duration_ms, existing_proxy) = {
        let state = runtime
            .inner
            .lock()
            .map_err(|error| format!("字幕审核状态锁定失败: {error}"))?;
        let session = state
            .sessions
            .get(&session_id)
            .ok_or_else(|| "字幕审核会话不存在或已结束".to_string())?;
        (
            session.video_path.clone(),
            session.video.duration_ms,
            session.proxy_path.clone(),
        )
    };

    if let Some(path) = existing_proxy.filter(|path| path.is_file()) {
        app.asset_protocol_scope()
            .allow_file(&path)
            .map_err(|error| format!("无法授权预览代理: {error}"))?;
        return Ok(SubtitleReviewProxyResult {
            session_id,
            preview_path: path_to_string(&path),
        });
    }

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut state = runtime
            .inner
            .lock()
            .map_err(|error| format!("字幕审核状态锁定失败: {error}"))?;
        if state.proxy_cancels.contains_key(&session_id) {
            return Err("预览代理正在生成".to_string());
        }
        state
            .proxy_cancels
            .insert(session_id.clone(), cancel.clone());
    }

    emit_proxy_progress(&app, &session_id, 0, "准备预览代理", "running", None);
    logger.info(
        "subtitle_review",
        "proxy_started",
        "开始生成字幕审核预览代理",
        json!({ "sessionId": &session_id, "videoPath": video_path.to_string_lossy() }),
    );

    let result: Result<SubtitleReviewProxyResult, String> = (|| {
        let temp_dir = tempfile::Builder::new()
            .prefix("link-subtitle-review-")
            .tempdir()
            .map_err(|error| format!("无法创建预览代理临时目录: {error}"))?;
        let proxy_path = temp_dir.path().join("preview.mp4");
        run_proxy_ffmpeg(
            &app,
            &session_id,
            &video_path,
            &proxy_path,
            duration_ms,
            &cancel,
        )?;
        app.asset_protocol_scope()
            .allow_file(&proxy_path)
            .map_err(|error| format!("无法授权预览代理: {error}"))?;

        let mut state = runtime
            .inner
            .lock()
            .map_err(|error| format!("字幕审核状态锁定失败: {error}"))?;
        let session = state
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| "字幕审核会话已结束".to_string())?;
        session.proxy_path = Some(proxy_path.clone());
        session.proxy_temp_dir = Some(temp_dir);

        Ok(SubtitleReviewProxyResult {
            session_id: session_id.clone(),
            preview_path: path_to_string(&proxy_path),
        })
    })();

    if let Ok(mut state) = runtime.inner.lock() {
        state.proxy_cancels.remove(&session_id);
    }

    match &result {
        Ok(value) => {
            emit_proxy_progress(
                &app,
                &session_id,
                100,
                "预览代理已就绪",
                "done",
                Some(value.preview_path.clone()),
            );
            logger.info(
                "subtitle_review",
                "proxy_completed",
                "字幕审核预览代理已生成",
                json!({ "sessionId": &session_id }),
            );
        }
        Err(error) if cancel.load(Ordering::Relaxed) => {
            emit_proxy_progress(&app, &session_id, 0, "已取消预览代理", "cancelled", None);
            logger.info(
                "subtitle_review",
                "proxy_cancelled",
                "字幕审核预览代理已取消",
                json!({ "sessionId": &session_id }),
            );
            let _ = error;
        }
        Err(error) => {
            emit_proxy_progress(&app, &session_id, 0, error, "failed", None);
            logger.warn(
                "subtitle_review",
                "proxy_failed",
                "字幕审核预览代理生成失败",
                json!({ "sessionId": &session_id, "error": error }),
            );
        }
    }

    result
}

#[tauri::command]
pub fn cancel_subtitle_review_proxy(
    runtime: tauri::State<'_, SubtitleReviewRuntime>,
    session_id: String,
) -> Result<(), String> {
    let state = runtime
        .inner
        .lock()
        .map_err(|error| format!("字幕审核状态锁定失败: {error}"))?;
    if let Some(cancel) = state.proxy_cancels.get(session_id.trim()) {
        cancel.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
pub async fn start_subtitle_review_export(
    app: AppHandle,
    runtime: tauri::State<'_, SubtitleReviewRuntime>,
    app_logger: tauri::State<'_, AppLogger>,
    request: SubtitleReviewExportRequest,
) -> Result<SubtitleReviewExportResult, String> {
    let runtime = runtime.inner().clone();
    let logger = app_logger.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        export_review_blocking(app, runtime, logger, request)
    })
    .await
    .map_err(|error| format!("字幕审核导出任务异常: {error}"))?
}

fn export_review_blocking(
    app: AppHandle,
    runtime: SubtitleReviewRuntime,
    logger: AppLogger,
    request: SubtitleReviewExportRequest,
) -> Result<SubtitleReviewExportResult, String> {
    let session_id = request.session_id.trim().to_string();
    let job_id = request.job_id.trim().to_string();
    if job_id.is_empty() {
        return Err("缺少导出任务标识".to_string());
    }
    validate_cue_ids(&request.cues)?;

    let (video_path, duration_ms, ass_content) = {
        let mut state = runtime
            .inner
            .lock()
            .map_err(|error| format!("字幕审核状态锁定失败: {error}"))?;
        if state
            .export_jobs
            .values()
            .any(|job| job.session_id == session_id)
        {
            return Err("当前审核会话已有导出任务".to_string());
        }
        let session = state
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| "字幕审核会话不存在或已结束".to_string())?;
        let validation = validate_cues(&request.cues, session.video.duration_ms);
        if !validation.can_export {
            return Err("字幕中存在无效时间，修正后才能导出".to_string());
        }
        let ass_content =
            serialize_review_document(&session.document, &request.cues, session.style.as_ref())?;
        session.cues = request.cues.clone();
        session.revision = request.revision;
        (
            session.video_path.clone(),
            session.video.duration_ms,
            ass_content,
        )
    };

    let output_path = normalize_export_output_path(&request.output_path, &video_path)?;
    validate_export_output_path(&video_path, &output_path)?;
    let output_path_string = path_to_string(&output_path);
    let cancel = Arc::new(AtomicBool::new(false));
    runtime
        .inner
        .lock()
        .map_err(|error| format!("字幕审核状态锁定失败: {error}"))?
        .export_jobs
        .insert(
            job_id.clone(),
            ExportControl {
                session_id: session_id.clone(),
                cancel: cancel.clone(),
            },
        );

    emit_export_progress(
        &app,
        &session_id,
        &job_id,
        0,
        "准备导出视频",
        "running",
        Some(output_path_string.clone()),
    );
    logger.info(
        "subtitle_review",
        "export_started",
        "开始导出字幕审核视频",
        json!({
            "sessionId": &session_id,
            "jobId": &job_id,
            "outputPath": &output_path_string,
            "cueCount": request.cues.len(),
        }),
    );

    let result: Result<SubtitleReviewExportResult, String> = (|| {
        let temp_dir = tempfile::Builder::new()
            .prefix("link-subtitle-review-export-")
            .tempdir()
            .map_err(|error| format!("无法创建字幕导出临时目录: {error}"))?;
        let ass_path = temp_dir.path().join("review.ass");
        fs::write(&ass_path, ass_content)
            .map_err(|error| format!("无法准备审核字幕文件: {error}"))?;
        run_export_ffmpeg(
            &app,
            &session_id,
            &job_id,
            &video_path,
            &ass_path,
            &output_path,
            duration_ms,
            &cancel,
        )?;
        Ok(SubtitleReviewExportResult {
            session_id: session_id.clone(),
            job_id: job_id.clone(),
            output_path: output_path_string.clone(),
            duration_ms,
        })
    })();

    if let Ok(mut state) = runtime.inner.lock() {
        state.export_jobs.remove(&job_id);
    }

    match &result {
        Ok(_) => {
            emit_export_progress(
                &app,
                &session_id,
                &job_id,
                100,
                "视频导出完成",
                "done",
                Some(output_path_string.clone()),
            );
            logger.info(
                "subtitle_review",
                "export_completed",
                "字幕审核视频导出完成",
                json!({ "sessionId": &session_id, "jobId": &job_id, "outputPath": &output_path_string }),
            );
        }
        Err(error) if cancel.load(Ordering::Relaxed) => {
            emit_export_progress(
                &app,
                &session_id,
                &job_id,
                0,
                "视频导出已取消",
                "cancelled",
                None,
            );
            logger.info(
                "subtitle_review",
                "export_cancelled",
                "字幕审核视频导出已取消",
                json!({ "sessionId": &session_id, "jobId": &job_id }),
            );
            let _ = error;
        }
        Err(error) => {
            emit_export_progress(&app, &session_id, &job_id, 0, error, "failed", None);
            logger.error(
                "subtitle_review",
                "export_failed",
                "字幕审核视频导出失败",
                json!({ "sessionId": &session_id, "jobId": &job_id, "error": error }),
            );
        }
    }

    result
}

#[tauri::command]
pub fn cancel_subtitle_review_export(
    runtime: tauri::State<'_, SubtitleReviewRuntime>,
    job_id: String,
) -> Result<(), String> {
    let state = runtime
        .inner
        .lock()
        .map_err(|error| format!("字幕审核状态锁定失败: {error}"))?;
    if let Some(job) = state.export_jobs.get(job_id.trim()) {
        job.cancel.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
pub fn release_subtitle_review_session(
    runtime: tauri::State<'_, SubtitleReviewRuntime>,
    session_id: String,
) -> Result<(), String> {
    let session_id = session_id.trim();
    let mut state = runtime
        .inner
        .lock()
        .map_err(|error| format!("字幕审核状态锁定失败: {error}"))?;
    if let Some(cancel) = state.proxy_cancels.get(session_id) {
        cancel.store(true, Ordering::Relaxed);
    }
    for job in state.export_jobs.values() {
        if job.session_id == session_id {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }
    state.sessions.remove(session_id);
    Ok(())
}

fn serialize_review_document(
    document: &ReviewDocument,
    cues: &[ReviewCue],
    style: Option<&SubtitleStyle>,
) -> Result<String, String> {
    match document {
        ReviewDocument::Plain => {
            let style = style.ok_or_else(|| "无法读取当前字幕样式".to_string())?;
            let segments = cues
                .iter()
                .map(|cue| TranscriptionSegment {
                    text: cue.text.clone(),
                    start_time: cue.start_time,
                    end_time: cue.end_time,
                    uid: cue.id.clone(),
                    status: "done".to_string(),
                    words: Vec::new(),
                })
                .collect::<Vec<_>>();
            Ok(serialize_styled_ass(&segments, style))
        }
        ReviewDocument::Ass(document) => serialize_ass_document(document, cues),
    }
}

fn serialize_ass_document(document: &AssDocument, cues: &[ReviewCue]) -> Result<String, String> {
    let cue_map = cues
        .iter()
        .map(|cue| (cue.id.as_str(), cue))
        .collect::<HashMap<_, _>>();
    let mut new_cues = cues
        .iter()
        .filter(|cue| cue.is_new || !document.original_ids.contains(&cue.id))
        .collect::<Vec<_>>();
    new_cues.sort_by_key(|cue| (cue.start_time, cue.end_time, cue.source_order));

    let mut output = String::new();
    for line in &document.template {
        match line {
            AssTemplateLine::Raw(line) => push_ass_line(&mut output, line),
            AssTemplateLine::Cue(id) => {
                if let Some(cue) = cue_map.get(id.as_str()) {
                    let meta = document
                        .cue_meta
                        .get(id)
                        .ok_or_else(|| "ASS 字幕事件元数据缺失".to_string())?;
                    push_ass_line(&mut output, &serialize_ass_cue(cue, meta));
                }
            }
            AssTemplateLine::NewCueMarker => {
                for cue in &new_cues {
                    push_ass_line(
                        &mut output,
                        &serialize_new_ass_cue(
                            cue,
                            &document.new_event_format,
                            &document.default_style,
                        ),
                    );
                }
            }
        }
    }
    Ok(output)
}

fn serialize_ass_cue(cue: &ReviewCue, meta: &AssCueMeta) -> String {
    let mut values = meta.values.clone();
    set_ass_field(&mut values, "start", ms_to_ass_time(cue.start_time));
    set_ass_field(&mut values, "end", ms_to_ass_time(cue.end_time));
    set_ass_field(&mut values, "style", cue.style_name.clone());
    set_ass_field(&mut values, "layer", cue.layer.clone());
    set_ass_field(&mut values, "marked", cue.layer.clone());
    set_ass_field(&mut values, "name", cue.actor.clone());
    set_ass_field(&mut values, "actor", cue.actor.clone());
    set_ass_field(&mut values, "marginl", cue.margin_l.clone());
    set_ass_field(&mut values, "marginr", cue.margin_r.clone());
    set_ass_field(&mut values, "marginv", cue.margin_v.clone());
    set_ass_field(&mut values, "effect", cue.effect.clone());
    let text = if cue.text_mode == "raw" {
        cue.raw_text.clone()
    } else {
        escape_plain_ass_text(&cue.text)
    };
    set_ass_field(&mut values, "text", text);
    let fields = meta
        .format
        .iter()
        .map(|field| {
            values
                .get(&field.to_ascii_lowercase())
                .cloned()
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    format!("Dialogue: {}", fields.join(","))
}

fn serialize_new_ass_cue(cue: &ReviewCue, format: &[String], default_style: &str) -> String {
    let format = if format.is_empty() {
        DEFAULT_EVENT_FORMAT
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
    } else {
        format.to_vec()
    };
    let mut values = HashMap::new();
    set_ass_field(&mut values, "layer", fallback_string(&cue.layer, "0"));
    set_ass_field(&mut values, "marked", fallback_string(&cue.layer, "0"));
    set_ass_field(&mut values, "start", ms_to_ass_time(cue.start_time));
    set_ass_field(&mut values, "end", ms_to_ass_time(cue.end_time));
    set_ass_field(
        &mut values,
        "style",
        fallback_string(&cue.style_name, default_style),
    );
    set_ass_field(&mut values, "name", cue.actor.clone());
    set_ass_field(&mut values, "actor", cue.actor.clone());
    set_ass_field(
        &mut values,
        "marginl",
        fallback_string(&cue.margin_l, "0000"),
    );
    set_ass_field(
        &mut values,
        "marginr",
        fallback_string(&cue.margin_r, "0000"),
    );
    set_ass_field(
        &mut values,
        "marginv",
        fallback_string(&cue.margin_v, "0000"),
    );
    set_ass_field(&mut values, "effect", cue.effect.clone());
    let text = if cue.text_mode == "raw" {
        cue.raw_text.clone()
    } else {
        escape_plain_ass_text(&cue.text)
    };
    set_ass_field(&mut values, "text", text);
    let fields = format
        .iter()
        .map(|field| {
            values
                .get(&field.to_ascii_lowercase())
                .cloned()
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    format!("Dialogue: {}", fields.join(","))
}

fn parse_ass_document(content: &str) -> Result<(AssDocument, Vec<ReviewCue>, Vec<String>), String> {
    let normalized = normalize_newlines(content);
    let lines = normalized.lines().collect::<Vec<_>>();
    let mut template = Vec::with_capacity(lines.len() + 1);
    let mut cue_meta = HashMap::new();
    let mut original_ids = HashSet::new();
    let mut cues = Vec::new();
    let mut section = String::new();
    let mut event_format = DEFAULT_EVENT_FORMAT
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    let mut last_event_format = event_format.clone();
    let mut default_style = "Default".to_string();
    let mut marker_added = false;
    let mut found_events = false;
    let mut warnings = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if section == "events" && !marker_added {
                template.push(AssTemplateLine::NewCueMarker);
                marker_added = true;
            }
            section = trimmed[1..trimmed.len() - 1].trim().to_ascii_lowercase();
            found_events |= section == "events";
            template.push(AssTemplateLine::Raw(line.to_string()));
            continue;
        }

        if section == "v4+ styles" || section == "v4 styles" {
            if trimmed.to_ascii_lowercase().starts_with("style:") {
                let payload = trimmed
                    .split_once(':')
                    .map(|(_, value)| value.trim())
                    .unwrap_or_default();
                if let Some(name) = payload
                    .split(',')
                    .next()
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                {
                    if default_style == "Default" || name.eq_ignore_ascii_case("default") {
                        default_style = name.to_string();
                    }
                }
            }
            template.push(AssTemplateLine::Raw(line.to_string()));
            continue;
        }

        if section != "events" {
            template.push(AssTemplateLine::Raw(line.to_string()));
            continue;
        }

        if trimmed.to_ascii_lowercase().starts_with("format:") {
            let payload = trimmed
                .split_once(':')
                .map(|(_, value)| value)
                .unwrap_or_default();
            let parsed = payload
                .split(',')
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>();
            if !parsed.is_empty() {
                event_format = parsed;
                last_event_format = event_format.clone();
            }
            template.push(AssTemplateLine::Raw(line.to_string()));
            continue;
        }

        if !trimmed.to_ascii_lowercase().starts_with("dialogue:") {
            template.push(AssTemplateLine::Raw(line.to_string()));
            continue;
        }

        let payload = trimmed
            .split_once(':')
            .map(|(_, value)| value.trim_start())
            .unwrap_or_default();
        let fields = split_ass_fields(payload, event_format.len());
        if fields.len() != event_format.len() {
            warnings.push(format!(
                "已跳过无法解析的 ASS 事件：{}",
                truncate_text(trimmed, 80)
            ));
            template.push(AssTemplateLine::Raw(line.to_string()));
            continue;
        }
        let values = event_format
            .iter()
            .zip(fields.iter())
            .map(|(field, value)| (field.to_ascii_lowercase(), value.clone()))
            .collect::<HashMap<_, _>>();
        let start_time = parse_ass_time(required_ass_field(&values, "start")?)?;
        let end_time = parse_ass_time(required_ass_field(&values, "end")?)?;
        let raw_text = required_ass_field(&values, "text")?.to_string();
        let id = format!("ass-{}", Uuid::new_v4());
        let layer = ass_field(&values, "layer")
            .or_else(|| ass_field(&values, "marked"))
            .unwrap_or("0")
            .to_string();
        let actor = ass_field(&values, "name")
            .or_else(|| ass_field(&values, "actor"))
            .unwrap_or_default()
            .to_string();
        let has_inline_tags = contains_ass_override_tags(&raw_text);
        let cue = ReviewCue {
            id: id.clone(),
            start_time,
            end_time,
            text: ass_plain_text(&raw_text),
            raw_text,
            text_mode: if has_inline_tags { "raw" } else { "plain" }.to_string(),
            style_name: ass_field(&values, "style")
                .unwrap_or(&default_style)
                .to_string(),
            layer,
            actor,
            margin_l: ass_field(&values, "marginl").unwrap_or("0000").to_string(),
            margin_r: ass_field(&values, "marginr").unwrap_or("0000").to_string(),
            margin_v: ass_field(&values, "marginv").unwrap_or("0000").to_string(),
            effect: ass_field(&values, "effect").unwrap_or_default().to_string(),
            has_inline_tags,
            source_order: cues.len() as u32,
            is_new: false,
        };
        original_ids.insert(id.clone());
        cue_meta.insert(
            id.clone(),
            AssCueMeta {
                format: event_format.clone(),
                values,
            },
        );
        cues.push(cue);
        template.push(AssTemplateLine::Cue(id));
    }

    if !found_events {
        return Err("ASS 字幕缺少 [Events] 章节".to_string());
    }
    if !marker_added {
        template.push(AssTemplateLine::NewCueMarker);
    }

    Ok((
        AssDocument {
            template,
            cue_meta,
            original_ids,
            new_event_format: last_event_format,
            default_style,
        },
        cues,
        warnings,
    ))
}

fn parse_srt_cues(content: &str) -> Result<Vec<ReviewCue>, String> {
    let normalized = normalize_newlines(content);
    let mut cues = Vec::new();
    for (block_index, block) in normalized.split("\n\n").enumerate() {
        let lines = block.lines().map(str::trim_end).collect::<Vec<_>>();
        let Some(time_index) = lines.iter().position(|line| line.contains("-->")) else {
            continue;
        };
        let (start_time, end_time) = parse_time_range(lines[time_index])
            .map_err(|error| format!("SRT 第 {} 段时间无效: {error}", block_index + 1))?;
        let text = clean_subtitle_markup(&lines[time_index + 1..].join("\n"));
        if text.trim().is_empty() {
            continue;
        }
        cues.push(plain_cue(cues.len(), start_time, end_time, text));
    }
    Ok(cues)
}

fn parse_vtt_cues(content: &str) -> Result<Vec<ReviewCue>, String> {
    let normalized = normalize_newlines(content.trim_start_matches('\u{feff}'));
    let mut cues = Vec::new();
    let mut block = Vec::new();
    for line in normalized.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            push_vtt_cue(&mut cues, &block)?;
            block.clear();
        } else if !trimmed.starts_with("WEBVTT") && !trimmed.starts_with("NOTE") {
            block.push(trimmed.to_string());
        }
    }
    push_vtt_cue(&mut cues, &block)?;
    Ok(cues)
}

fn push_vtt_cue(cues: &mut Vec<ReviewCue>, lines: &[String]) -> Result<(), String> {
    let Some(time_index) = lines.iter().position(|line| line.contains("-->")) else {
        return Ok(());
    };
    let (start_time, end_time) = parse_time_range(&lines[time_index])?;
    let text = clean_subtitle_markup(&lines[time_index + 1..].join("\n"));
    if !text.trim().is_empty() {
        cues.push(plain_cue(cues.len(), start_time, end_time, text));
    }
    Ok(())
}

fn plain_cue(index: usize, start_time: u64, end_time: u64, text: String) -> ReviewCue {
    ReviewCue {
        id: format!("cue-{}", Uuid::new_v4()),
        start_time,
        end_time,
        raw_text: text.clone(),
        text,
        text_mode: "plain".to_string(),
        style_name: "Primary".to_string(),
        layer: "0".to_string(),
        actor: String::new(),
        margin_l: "0000".to_string(),
        margin_r: "0000".to_string(),
        margin_v: "0000".to_string(),
        effect: String::new(),
        has_inline_tags: false,
        source_order: index as u32,
        is_new: false,
    }
}

fn validate_cue_ids(cues: &[ReviewCue]) -> Result<(), String> {
    let mut ids = HashSet::new();
    for cue in cues {
        if cue.id.trim().is_empty() || !ids.insert(cue.id.as_str()) {
            return Err("字幕条目标识无效或重复".to_string());
        }
    }
    Ok(())
}

fn validate_cues(cues: &[ReviewCue], duration_ms: u64) -> ReviewValidation {
    let mut issues = Vec::new();
    let mut has_error = false;
    if cues.is_empty() {
        has_error = true;
        issues.push(ReviewValidationIssue {
            cue_id: None,
            level: "error".to_string(),
            code: "empty-document".to_string(),
            message: "没有可导出的字幕".to_string(),
        });
    }
    let mut sorted = cues.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|cue| (cue.start_time, cue.end_time));
    for cue in &sorted {
        if cue.end_time <= cue.start_time {
            has_error = true;
            issues.push(cue_issue(
                cue,
                "error",
                "invalid-range",
                "结束时间必须晚于开始时间",
            ));
        }
        if cue.text.trim().is_empty() && cue.raw_text.trim().is_empty() {
            issues.push(cue_issue(cue, "warning", "empty-text", "字幕文字为空"));
        }
        if duration_ms > 0 && cue.end_time > duration_ms {
            issues.push(cue_issue(
                cue,
                "warning",
                "after-video",
                "字幕结束时间超出视频时长",
            ));
        }
    }
    for pair in sorted.windows(2) {
        if pair[0].end_time > pair[1].start_time && !is_bilingual_pair(pair[0], pair[1]) {
            issues.push(cue_issue(
                pair[1],
                "warning",
                "overlap",
                "与上一条字幕时间重叠",
            ));
        }
    }
    ReviewValidation {
        can_export: !has_error,
        issues,
    }
}

fn is_bilingual_pair(left: &ReviewCue, right: &ReviewCue) -> bool {
    if left.start_time != right.start_time
        || left.end_time != right.end_time
        || left.style_name.eq_ignore_ascii_case(&right.style_name)
    {
        return false;
    }

    let left_has_cjk = left.text.chars().any(is_cjk_character);
    let right_has_cjk = right.text.chars().any(is_cjk_character);
    if left_has_cjk != right_has_cjk {
        return true;
    }

    is_bilingual_style_name(&left.style_name) || is_bilingual_style_name(&right.style_name)
}

fn is_bilingual_style_name(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "default" | "primary" | "secondary" | "source" | "target" | "original" | "translation"
    )
}

fn is_cjk_character(character: char) -> bool {
    matches!(
        character as u32,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x3040..=0x30FF | 0xAC00..=0xD7AF
    )
}

fn cue_issue(cue: &ReviewCue, level: &str, code: &str, message: &str) -> ReviewValidationIssue {
    ReviewValidationIssue {
        cue_id: Some(cue.id.clone()),
        level: level.to_string(),
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn canonical_input_path(path: &str, extensions: &[&str], label: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path.trim());
    if path.as_os_str().is_empty() {
        return Err(format!("请选择{label}文件"));
    }
    if !path.is_file() {
        return Err(format!("{label}文件不存在"));
    }
    let extension = file_extension(&path).unwrap_or_default();
    if !extensions.contains(&extension.as_str()) {
        return Err(format!("{label}文件格式不支持"));
    }
    fs::canonicalize(&path).map_err(|error| format!("无法读取{label}文件路径: {error}"))
}

fn decode_subtitle_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("无法读取字幕文件: {error}"))?;
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8(bytes[3..].to_vec())
            .map_err(|error| format!("字幕 UTF-8 编码无效: {error}"));
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return decode_utf16(&bytes[2..], true);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return decode_utf16(&bytes[2..], false);
    }
    if let Ok(content) = String::from_utf8(bytes.clone()) {
        return Ok(content);
    }
    let (content, _, had_errors) = GBK.decode(&bytes);
    if had_errors {
        return Err("字幕编码无法识别，请转换为 UTF-8、UTF-16 或 GB18030".to_string());
    }
    Ok(content.into_owned())
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> Result<String, String> {
    if !bytes.len().is_multiple_of(2) {
        return Err("UTF-16 字幕字节长度无效".to_string());
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
    String::from_utf16(&units).map_err(|error| format!("UTF-16 字幕编码无效: {error}"))
}

fn probe_video_metadata(path: &Path) -> Result<ReviewVideoMetadata, String> {
    let mut command = create_command("ffprobe");
    command
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration:stream=index,codec_type,codec_name,width,height")
        .arg("-of")
        .arg("json")
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = command
        .output()
        .map_err(|error| format!("无法启动 ffprobe: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "无法读取视频信息: {}",
            summarize_stderr(&String::from_utf8_lossy(&output.stderr))
        ));
    }
    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("无法解析视频信息: {error}"))?;
    let duration_seconds = value
        .get("format")
        .and_then(|format| format.get("duration"))
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0);
    let streams = value
        .get("streams")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let video_stream = streams
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("video"));
    let audio_stream = streams
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("audio"));
    let width = video_stream
        .and_then(|stream| stream.get("width"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    let height = video_stream
        .and_then(|stream| stream.get("height"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    if width == 0 || height == 0 {
        return Err("视频中没有可用的视频画面".to_string());
    }
    Ok(ReviewVideoMetadata {
        path: path_to_string(path),
        preview_path: path_to_string(path),
        file_name: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("视频")
            .to_string(),
        duration_ms: (duration_seconds.max(0.0) * 1000.0).round() as u64,
        width,
        height,
        video_codec: video_stream
            .and_then(|stream| stream.get("codec_name"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        audio_codec: audio_stream
            .and_then(|stream| stream.get("codec_name"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        file_size: fs::metadata(path)
            .map_err(|error| format!("无法读取视频文件大小: {error}"))?
            .len(),
    })
}

fn run_proxy_ffmpeg(
    app: &AppHandle,
    session_id: &str,
    video_path: &Path,
    output_path: &Path,
    duration_ms: u64,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let mut command = create_command("ffmpeg");
    command
        .arg("-hide_banner")
        .arg("-nostdin")
        .arg("-nostats")
        .arg("-v")
        .arg("error")
        .arg("-i")
        .arg(video_path)
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("0:a:0?")
        .arg("-vf")
        .arg("scale=w='min(1280,iw)':h='min(1280,ih)':force_original_aspect_ratio=decrease:force_divisible_by=2")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("veryfast")
        .arg("-crf")
        .arg("28")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg("96k")
        .arg("-movflags")
        .arg("+faststart")
        .arg("-progress")
        .arg("pipe:1")
        .arg("-y")
        .arg(output_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("无法启动 ffmpeg: {error}"))?;
    let result = monitor_ffmpeg(&mut child, duration_ms, cancel, |progress| {
        emit_proxy_progress(
            app,
            session_id,
            progress,
            "正在生成预览代理",
            "running",
            None,
        )
    });
    if result.is_err() {
        let _ = fs::remove_file(output_path);
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn run_export_ffmpeg(
    app: &AppHandle,
    session_id: &str,
    job_id: &str,
    video_path: &Path,
    ass_path: &Path,
    output_path: &Path,
    duration_ms: u64,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let work_dir = ass_path
        .parent()
        .ok_or_else(|| "审核字幕临时目录无效".to_string())?;
    let mut command = create_command("ffmpeg");
    command
        .current_dir(work_dir)
        .arg("-hide_banner")
        .arg("-nostdin")
        .arg("-nostats")
        .arg("-v")
        .arg("error")
        .arg("-i")
        .arg(video_path)
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("0:a?")
        .arg("-vf")
        .arg("subtitles=review.ass")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("veryfast")
        .arg("-crf")
        .arg("18")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg("192k")
        .arg("-movflags")
        .arg("+faststart")
        .arg("-progress")
        .arg("pipe:1")
        .arg("-y")
        .arg(output_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output_path_string = path_to_string(output_path);
    let result = monitor_ffmpeg(
        &mut command
            .spawn()
            .map_err(|error| format!("无法启动 ffmpeg: {error}"))?,
        duration_ms,
        cancel,
        |progress| {
            emit_export_progress(
                app,
                session_id,
                job_id,
                progress,
                "正在烧录审核字幕",
                "running",
                Some(output_path_string.clone()),
            )
        },
    );
    if result.is_err() {
        let _ = fs::remove_file(output_path);
    }
    result
}

fn monitor_ffmpeg<F>(
    child: &mut Child,
    duration_ms: u64,
    cancel: &AtomicBool,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(u8),
{
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取 ffmpeg 进度".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "无法读取 ffmpeg 错误输出".to_string())?;
    let stderr_handle = thread::spawn(move || read_stream_to_string(stderr));
    let mut last_progress = 1_u8;
    on_progress(last_progress);
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stderr_handle.join();
            return Err("任务已取消".to_string());
        }
        if let Some(time_ms) = parse_progress_time_ms(&line) {
            let progress = if duration_ms > 0 {
                ((time_ms.min(duration_ms) * 96) / duration_ms).clamp(1, 96) as u8
            } else {
                last_progress
            };
            if progress > last_progress {
                last_progress = progress;
                on_progress(progress);
            }
        }
    }
    if cancel.load(Ordering::Relaxed) {
        let _ = child.kill();
    }
    let status = child
        .wait()
        .map_err(|error| format!("等待 ffmpeg 结束失败: {error}"))?;
    let stderr = stderr_handle.join().unwrap_or_default();
    if cancel.load(Ordering::Relaxed) {
        Err("任务已取消".to_string())
    } else if status.success() {
        Ok(())
    } else {
        Err(format!("ffmpeg 处理失败: {}", summarize_stderr(&stderr)))
    }
}

fn emit_proxy_progress(
    app: &AppHandle,
    session_id: &str,
    progress: u8,
    message: &str,
    status: &str,
    preview_path: Option<String>,
) {
    let _ = app.emit(
        PROXY_PROGRESS_EVENT,
        SubtitleReviewProxyProgress {
            session_id: session_id.to_string(),
            progress: progress.min(100),
            message: message.to_string(),
            status: status.to_string(),
            preview_path,
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_export_progress(
    app: &AppHandle,
    session_id: &str,
    job_id: &str,
    progress: u8,
    message: &str,
    status: &str,
    output_path: Option<String>,
) {
    let _ = app.emit(
        EXPORT_PROGRESS_EVENT,
        SubtitleReviewExportProgress {
            session_id: session_id.to_string(),
            job_id: job_id.to_string(),
            progress: progress.min(100),
            message: message.to_string(),
            status: status.to_string(),
            output_path,
        },
    );
}

fn normalize_export_output_path(value: &str, video_path: &Path) -> Result<PathBuf, String> {
    let mut path = if value.trim().is_empty() {
        let parent = video_path.parent().unwrap_or_else(|| Path::new("."));
        let stem = video_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("video");
        parent.join(format!("{stem}_reviewed.mp4"))
    } else {
        PathBuf::from(value.trim())
    };
    if path.extension().is_none() {
        path.set_extension("mp4");
    }
    if file_extension(&path).as_deref() != Some("mp4") {
        return Err("审核视频需要导出为 MP4 文件".to_string());
    }
    Ok(path)
}

fn validate_export_output_path(video_path: &Path, output_path: &Path) -> Result<(), String> {
    if paths_refer_same_file(video_path, output_path) {
        return Err("输出文件不能覆盖源视频".to_string());
    }
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.is_dir() {
            return Err("输出目录不存在".to_string());
        }
    }
    Ok(())
}

fn paths_refer_same_file(first: &Path, second: &Path) -> bool {
    match (fs::canonicalize(first), fs::canonicalize(second)) {
        (Ok(first), Ok(second)) => first == second,
        _ => first
            .to_string_lossy()
            .eq_ignore_ascii_case(&second.to_string_lossy()),
    }
}

fn parse_progress_time_ms(line: &str) -> Option<u64> {
    let value = line.strip_prefix("out_time=")?;
    let mut parts = value.trim().split(':');
    let hours = parts.next()?.parse::<u64>().ok()?;
    let minutes = parts.next()?.parse::<u64>().ok()?;
    let seconds = parts.next()?.parse::<f64>().ok()?;
    Some((((hours * 60 + minutes) * 60) as f64 * 1000.0 + seconds * 1000.0).round() as u64)
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

fn parse_ass_time(value: &str) -> Result<u64, String> {
    parse_subtitle_time(value)
}

fn parse_subtitle_time(value: &str) -> Result<u64, String> {
    let normalized = value.trim().replace(',', ".");
    let parts = normalized.split(':').collect::<Vec<_>>();
    if parts.len() < 2 || parts.len() > 3 {
        return Err(format!("无效字幕时间: {value}"));
    }
    let (hours, minutes, seconds_text) = if parts.len() == 3 {
        (parts[0], parts[1], parts[2])
    } else {
        ("0", parts[0], parts[1])
    };
    let hours = hours
        .parse::<u64>()
        .map_err(|_| format!("无效字幕时间: {value}"))?;
    let minutes = minutes
        .parse::<u64>()
        .map_err(|_| format!("无效字幕时间: {value}"))?;
    let (seconds, fraction) = seconds_text.split_once('.').unwrap_or((seconds_text, ""));
    let seconds = seconds
        .parse::<u64>()
        .map_err(|_| format!("无效字幕时间: {value}"))?;
    let mut millis_text = fraction.chars().take(3).collect::<String>();
    while millis_text.len() < 3 {
        millis_text.push('0');
    }
    let millis = if millis_text.is_empty() {
        0
    } else {
        millis_text
            .parse::<u64>()
            .map_err(|_| format!("无效字幕时间: {value}"))?
    };
    Ok((((hours * 60 + minutes) * 60 + seconds) * 1000) + millis)
}

fn ms_to_ass_time(ms: u64) -> String {
    let centiseconds = (ms % 1000) / 10;
    let total_seconds = ms / 1000;
    let seconds = total_seconds % 60;
    let total_minutes = total_seconds / 60;
    let minutes = total_minutes % 60;
    let hours = total_minutes / 60;
    format!("{hours}:{minutes:02}:{seconds:02}.{centiseconds:02}")
}

fn split_ass_fields(payload: &str, expected: usize) -> Vec<String> {
    if expected <= 1 {
        return vec![payload.to_string()];
    }
    payload
        .splitn(expected, ',')
        .map(|value| value.trim().to_string())
        .collect()
}

fn required_ass_field<'a>(
    values: &'a HashMap<String, String>,
    name: &str,
) -> Result<&'a str, String> {
    ass_field(values, name).ok_or_else(|| format!("ASS 字幕事件缺少 {name} 字段"))
}

fn ass_field<'a>(values: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    values.get(&name.to_ascii_lowercase()).map(String::as_str)
}

fn set_ass_field(values: &mut HashMap<String, String>, name: &str, value: String) {
    values.insert(name.to_ascii_lowercase(), value);
}

fn contains_ass_override_tags(text: &str) -> bool {
    let mut in_tag = false;
    for character in text.chars() {
        match character {
            '{' => in_tag = true,
            '}' if in_tag => return true,
            _ => {}
        }
    }
    false
}

fn ass_plain_text(text: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '{' => in_tag = true,
            '}' if in_tag => in_tag = false,
            '\\' if !in_tag => match chars.peek().copied() {
                Some('N') | Some('n') => {
                    chars.next();
                    output.push('\n');
                }
                Some('h') => {
                    chars.next();
                    output.push(' ');
                }
                _ => output.push(character),
            },
            _ if !in_tag => output.push(character),
            _ => {}
        }
    }
    decode_html_entities(output.trim()).into_owned()
}

fn escape_plain_ass_text(text: &str) -> String {
    text.replace('\r', "")
        .replace('\n', "\\N")
        .replace(['{', '}'], "")
}

fn clean_subtitle_markup(text: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;
    for character in text.chars() {
        match character {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => output.push(character),
            _ => {}
        }
    }
    decode_html_entities(output.trim()).into_owned()
}

fn normalize_newlines(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n")
}

fn push_ass_line(output: &mut String, line: &str) {
    if !output.is_empty() {
        output.push('\n');
    }
    output.push_str(line);
}

fn fallback_string(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

fn file_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn truncate_text(value: &str, max_chars: usize) -> String {
    let mut output = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars {
        output.push('…');
    }
    output
}

fn summarize_stderr(stderr: &str) -> String {
    stderr
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .unwrap_or("ffmpeg 未返回错误详情")
        .chars()
        .take(500)
        .collect()
}

fn read_stream_to_string<R: Read>(mut stream: R) -> String {
    let mut output = String::new();
    let _ = stream.read_to_string(&mut output);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASS_SAMPLE: &str = "[Script Info]\nTitle: Review\n\n[V4+ Styles]\nFormat: Name,Fontname,Fontsize,PrimaryColour,SecondaryColour,OutlineColour,BackColour,Bold,Italic,Underline,StrikeOut,ScaleX,ScaleY,Spacing,Angle,BorderStyle,Outline,Shadow,Alignment,MarginL,MarginR,MarginV,Encoding\nStyle: Default,Arial,42,&H00FFFFFF,&H000000FF,&H00000000,&H64000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,40,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nComment: 0,0:00:00.00,0:00:01.00,Default,,0,0,0,,keep me\nDialogue: 1,0:00:01.20,0:00:03.40,Default,Actor,0010,0020,0030,fx,{\\pos(100,200)}Hello\\Nworld\n\n[Fonts]\nfontname: embedded.ttf";

    #[test]
    fn ass_round_trip_preserves_structure_and_tags() {
        let (document, cues, _) = parse_ass_document(ASS_SAMPLE).expect("parse ass");
        assert_eq!(cues.len(), 1);
        assert!(cues[0].has_inline_tags);
        assert_eq!(cues[0].text, "Hello\nworld");
        let output = serialize_ass_document(&document, &cues).expect("serialize ass");
        assert!(output.contains("Comment: 0,0:00:00.00"));
        assert!(output.contains("{\\pos(100,200)}Hello\\Nworld"));
        assert!(output.contains("[Fonts]\nfontname: embedded.ttf"));
    }

    #[test]
    fn ass_plain_edit_removes_only_inline_tags_for_target_cue() {
        let (document, mut cues, _) = parse_ass_document(ASS_SAMPLE).expect("parse ass");
        cues[0].text_mode = "plain".to_string();
        cues[0].text = "修改后的文字".to_string();
        let output = serialize_ass_document(&document, &cues).expect("serialize ass");
        assert!(output.contains(",fx,修改后的文字"));
        assert!(!output.contains("\\pos(100,200)"));
    }

    #[test]
    fn parses_srt_and_vtt_times() {
        let srt = "1\n00:00:01,250 --> 00:00:02,500\n你好\n";
        let vtt = "WEBVTT\n\n00:01.000 --> 00:02.750\nHello\n";
        let srt_cues = parse_srt_cues(srt).expect("parse srt");
        let vtt_cues = parse_vtt_cues(vtt).expect("parse vtt");
        assert_eq!((srt_cues[0].start_time, srt_cues[0].end_time), (1250, 2500));
        assert_eq!((vtt_cues[0].start_time, vtt_cues[0].end_time), (1000, 2750));
    }

    #[test]
    fn validation_blocks_invalid_ranges_but_allows_overlap() {
        let mut cues = vec![
            plain_cue(0, 0, 1000, "一".to_string()),
            plain_cue(1, 900, 800, "二".to_string()),
        ];
        let validation = validate_cues(&cues, 2000);
        assert!(!validation.can_export);
        cues[1].end_time = 1500;
        let validation = validate_cues(&cues, 2000);
        assert!(validation.can_export);
        assert!(validation
            .issues
            .iter()
            .any(|issue| issue.code == "overlap"));
    }

    #[test]
    fn validation_treats_same_time_bilingual_events_as_one_logical_cue() {
        let mut source = plain_cue(0, 1_000, 2_000, "Hello".to_string());
        source.style_name = "Secondary".to_string();
        let mut target = plain_cue(1, 1_000, 2_000, "你好".to_string());
        target.style_name = "Default".to_string();
        let validation = validate_cues(&[source, target], 5_000);
        assert!(validation.can_export);
        assert!(!validation
            .issues
            .iter()
            .any(|issue| issue.code == "overlap"));
    }

    #[test]
    fn parses_video_stream_ranges() {
        assert_eq!(parse_stream_range("bytes=0-", 10_000), Ok((0, 9_999)));
        assert_eq!(parse_stream_range("bytes=100-499", 10_000), Ok((100, 499)));
        assert_eq!(parse_stream_range("bytes=-500", 10_000), Ok((9_500, 9_999)));
        assert!(parse_stream_range("bytes=10000-", 10_000).is_err());
        assert!(parse_stream_range("bytes=0-10,20-30", 10_000).is_err());
    }

    #[test]
    fn streams_review_video_with_content_range() {
        let directory = tempfile::tempdir().expect("temp directory");
        let video_path = directory.path().join("preview.mp4");
        fs::write(&video_path, b"0123456789").expect("write preview video");
        let runtime = SubtitleReviewRuntime::default();
        runtime
            .inner
            .lock()
            .expect("review runtime")
            .sessions
            .insert(
                "session".to_string(),
                ReviewSession {
                    video_path: video_path.clone(),
                    video: ReviewVideoMetadata {
                        path: path_to_string(&video_path),
                        preview_path: path_to_string(&video_path),
                        file_name: "preview.mp4".to_string(),
                        duration_ms: 1_000,
                        width: 16,
                        height: 9,
                        video_codec: "h264".to_string(),
                        audio_codec: "aac".to_string(),
                        file_size: 10,
                    },
                    document: ReviewDocument::Plain,
                    style: None,
                    cues: Vec::new(),
                    revision: 0,
                    proxy_temp_dir: None,
                    proxy_path: None,
                },
            );
        let request = Request::builder()
            .uri("review-video://localhost/session")
            .header("range", "bytes=2-5")
            .body(Vec::new())
            .expect("stream request");

        let response = subtitle_review_stream_response(&runtime, request).expect("stream response");
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(response.headers()[CONTENT_RANGE], "bytes 2-5/10");
        assert_eq!(response.body(), b"2345");
    }
}
