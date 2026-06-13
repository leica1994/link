use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use chrono::{DateTime, Utc};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::app_paths;
use crate::command_utils::create_command;
use crate::settings::SettingsStore;

const YTDLP_COMMAND: &str = "yt-dlp";
const DETAIL_STATUS_PENDING: &str = "pending";
const DETAIL_STATUS_LOADING: &str = "loading";
const DETAIL_STATUS_READY: &str = "ready";
const DETAIL_STATUS_FAILED: &str = "failed";
const SUBTITLE_SOURCE_MANUAL: &str = "manual";
const SUBTITLE_SOURCE_AUTOMATIC: &str = "automatic";
const YTDLP_SOCKET_TIMEOUT_SECONDS: &str = "30";
const YTDLP_YOUTUBE_EXTRACTOR_ARGS: &str = "youtube:lang=zh-CN;player_client=default,-android_vr";
const YTDLP_YOUTUBE_DETAIL_EXTRACTOR_ARGS: &str = "youtube:lang=zh-CN;player_client=ios";
const YOUTUBE_ACCEPT_LANGUAGE: &str = "Accept-Language: zh-CN,zh;q=0.9,en;q=0.8";
const THUMBNAIL_DOWNLOAD_TIMEOUT_SECONDS: u64 = 30;
const MAX_THUMBNAIL_BYTES: usize = 8 * 1024 * 1024;
const HOME_VIDEO_DOWNLOAD_PROGRESS_EVENT: &str = "home-video-download-progress";
const DOWNLOAD_KIND_VIDEO: &str = "video";
const DOWNLOAD_KIND_SUBTITLE: &str = "subtitle";
const DOWNLOAD_PROGRESS_ACTIVE: &str = "active";
const DOWNLOAD_PROGRESS_DONE: &str = "done";
const DOWNLOAD_PROGRESS_FAILED: &str = "failed";
const YTDLP_PROGRESS_PREFIX: &str = "LINK_YTDLP_PROGRESS";
const YTDLP_DOWNLOAD_PROGRESS_TEMPLATE: &str =
    "download:LINK_YTDLP_PROGRESS\tdownload\t%(progress.status)s\t%(progress.downloaded_bytes)s\t%(progress.total_bytes)s\t%(progress.total_bytes_estimate)s\t%(progress._percent_str)s";
const YTDLP_POSTPROCESS_PROGRESS_TEMPLATE: &str =
    "postprocess:LINK_YTDLP_PROGRESS\tpostprocess\t%(progress.status)s\tNA\tNA\tNA\t%(progress._percent_str)s";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeVideoTask {
    pub id: String,
    pub url: String,
    pub source_channel_id: String,
    pub source_video_id: String,
    pub external_id: String,
    pub title: String,
    pub channel_title: String,
    pub channel_url: String,
    pub thumbnail_url: String,
    pub duration: Option<f64>,
    pub webpage_url: String,
    pub description: String,
    pub view_count: Option<i64>,
    pub like_count: Option<i64>,
    pub comment_count: Option<i64>,
    pub upload_date: String,
    pub detail_status: String,
    pub subtitle_options: Vec<HomeVideoSubtitleOption>,
    pub metadata: Value,
    pub error_message: String,
    pub created_at: String,
    pub updated_at: String,
    pub detail_checked_at: Option<String>,
    pub downloaded_subtitles: Vec<HomeVideoSubtitle>,
    pub downloaded_video: Option<HomeVideoDownload>,
    pub partial_video_download: Option<HomePartialVideoDownload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeVideoSubtitleOption {
    pub language: String,
    pub name: String,
    pub source_kind: String,
    pub formats: Vec<String>,
    pub is_auto: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeVideoSubtitle {
    pub id: String,
    pub task_id: String,
    pub language: String,
    pub language_name: String,
    pub source_kind: String,
    pub format: String,
    pub file_path: String,
    pub file_size: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeVideoDownload {
    pub id: String,
    pub task_id: String,
    pub format: String,
    pub file_path: String,
    pub file_name: String,
    pub file_size: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomePartialVideoDownload {
    pub file_count: usize,
    pub file_size: i64,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddHomeVideoTaskRequest {
    pub url: String,
    pub title: Option<String>,
    pub source_channel_id: Option<String>,
    pub source_video_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeVideoTaskRequest {
    pub task_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadHomeVideoTaskSubtitleRequest {
    pub task_id: String,
    pub language: String,
    pub source_kind: String,
}

struct VideoDetail {
    external_id: String,
    title: String,
    channel_title: String,
    channel_url: String,
    thumbnail_url: String,
    duration: Option<f64>,
    webpage_url: String,
    description: String,
    view_count: Option<i64>,
    like_count: Option<i64>,
    comment_count: Option<i64>,
    upload_date: String,
    subtitle_options: Vec<HomeVideoSubtitleOption>,
    metadata: Value,
}

struct SubtitleDownloadOutput {
    path: PathBuf,
    format: String,
    file_size: i64,
}

struct VideoDownloadOutput {
    path: PathBuf,
    file_name: String,
    format: String,
    file_size: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HomeVideoDownloadProgress {
    task_id: String,
    kind: String,
    key: String,
    progress: u8,
    status: String,
    message: String,
    downloaded_bytes: Option<u64>,
    total_bytes: Option<u64>,
    language: Option<String>,
    source_kind: Option<String>,
}

#[derive(Debug, Clone)]
struct DownloadProgressEmitter {
    app: AppHandle,
    task_id: String,
    kind: String,
    key: String,
    language: Option<String>,
    source_kind: Option<String>,
}

#[derive(Debug, Clone)]
struct YtdlpProgressLine {
    phase: String,
    status: String,
    progress: Option<u8>,
    downloaded_bytes: Option<u64>,
    total_bytes: Option<u64>,
}

impl DownloadProgressEmitter {
    fn video(app: AppHandle, task_id: &str) -> Self {
        Self {
            app,
            task_id: task_id.to_string(),
            kind: DOWNLOAD_KIND_VIDEO.to_string(),
            key: DOWNLOAD_KIND_VIDEO.to_string(),
            language: None,
            source_kind: None,
        }
    }

    fn subtitle(app: AppHandle, task_id: &str, option: &HomeVideoSubtitleOption) -> Self {
        Self {
            app,
            task_id: task_id.to_string(),
            kind: DOWNLOAD_KIND_SUBTITLE.to_string(),
            key: subtitle_key(option),
            language: Some(option.language.clone()),
            source_kind: Some(option.source_kind.clone()),
        }
    }

    fn emit(
        &self,
        progress: u8,
        status: &str,
        message: &str,
        downloaded_bytes: Option<u64>,
        total_bytes: Option<u64>,
    ) {
        let _ = self.app.emit(
            HOME_VIDEO_DOWNLOAD_PROGRESS_EVENT,
            HomeVideoDownloadProgress {
                task_id: self.task_id.clone(),
                kind: self.kind.clone(),
                key: self.key.clone(),
                progress: progress.min(100),
                status: status.to_string(),
                message: message.to_string(),
                downloaded_bytes,
                total_bytes,
                language: self.language.clone(),
                source_kind: self.source_kind.clone(),
            },
        );
    }

    fn emit_active(&self, progress: u8, message: &str) {
        self.emit(progress, DOWNLOAD_PROGRESS_ACTIVE, message, None, None);
    }

    fn emit_done(&self, message: &str) {
        self.emit(100, DOWNLOAD_PROGRESS_DONE, message, None, None);
    }

    fn emit_failed(&self, message: &str) {
        self.emit(100, DOWNLOAD_PROGRESS_FAILED, message, None, None);
    }

    fn emit_ytdlp_progress(&self, progress: &YtdlpProgressLine) {
        let value = match progress.phase.as_str() {
            "postprocess" => {
                // 后处理阶段：95-98%
                if progress.status == "finished" {
                    98
                } else {
                    95
                }
            }
            // 下载阶段：将 yt-dlp 的 0-100% 映射到 2-95%
            _ => {
                let raw_progress = progress.progress.unwrap_or(0);
                // 将 0-100 线性映射到 2-95
                let mapped = 2 + (raw_progress as f64 * 0.93).round() as u8;
                mapped.clamp(2, 95)
            }
        };
        let message = download_progress_message(&self.kind, &progress.phase, &progress.status);
        self.emit(
            value,
            DOWNLOAD_PROGRESS_ACTIVE,
            &message,
            progress.downloaded_bytes,
            progress.total_bytes,
        );
    }
}

#[tauri::command]
pub fn list_home_video_tasks(
    store: tauri::State<'_, SettingsStore>,
) -> Result<Vec<HomeVideoTask>, String> {
    store.with_connection(read_home_video_tasks)
}

#[tauri::command]
pub fn add_home_video_task(
    store: tauri::State<'_, SettingsStore>,
    request: AddHomeVideoTaskRequest,
) -> Result<HomeVideoTask, String> {
    let url = normalize_youtube_video_url(&request.url)?;
    let title = request.title.unwrap_or_default();
    let source_channel_id = request.source_channel_id.unwrap_or_default();
    let source_video_id = request.source_video_id.unwrap_or_default();
    let external_id = youtube_video_id_from_url(&url).unwrap_or_default();
    let now = Utc::now().to_rfc3339();

    store.with_connection(|connection| {
        let existing_id = connection
            .query_row(
                "SELECT id FROM home_video_tasks WHERE url = ?1 LIMIT 1",
                params![url],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("无法检查待办任务: {error}"))?;

        if let Some(existing_id) = existing_id {
            connection
                .execute(
                    "
                    UPDATE home_video_tasks
                    SET source_channel_id = CASE WHEN source_channel_id = '' AND ?2 != '' THEN ?2 ELSE source_channel_id END,
                        source_video_id = CASE WHEN source_video_id = '' AND ?3 != '' THEN ?3 ELSE source_video_id END,
                        external_id = CASE WHEN external_id = '' AND ?4 != '' THEN ?4 ELSE external_id END,
                        title = CASE WHEN title = '' AND ?5 != '' THEN ?5 ELSE title END,
                        updated_at = ?6
                    WHERE id = ?1
                    ",
                    params![existing_id, source_channel_id, source_video_id, external_id, title, now],
                )
                .map_err(|error| format!("无法更新待办任务: {error}"))?;
            return read_home_video_task_by_id(connection, &existing_id);
        }

        let id = Uuid::new_v4().to_string();
        connection
            .execute(
                "
                INSERT INTO home_video_tasks (
                    id, url, source_channel_id, source_video_id, external_id, title,
                    detail_status, subtitle_options, metadata, created_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '[]', '{}', ?8, ?9)
                ",
                params![
                    id,
                    url,
                    source_channel_id,
                    source_video_id,
                    external_id,
                    title,
                    DETAIL_STATUS_PENDING,
                    now,
                    now,
                ],
            )
            .map_err(|error| format!("无法添加待办任务: {error}"))?;

        read_home_video_task_by_id(connection, &id)
    })
}

#[tauri::command]
pub fn get_home_video_task(
    store: tauri::State<'_, SettingsStore>,
    request: HomeVideoTaskRequest,
) -> Result<HomeVideoTask, String> {
    store.with_connection(|connection| read_home_video_task_by_id(connection, &request.task_id))
}

#[tauri::command]
pub fn delete_home_video_task(app: AppHandle, request: HomeVideoTaskRequest) -> Result<(), String> {
    let store = app.state::<SettingsStore>();
    let deleted = store.with_connection(|connection| {
        let transaction = connection
            .unchecked_transaction()
            .map_err(|error| format!("无法删除待办任务: {error}"))?;
        transaction
            .execute(
                "DELETE FROM home_video_task_subtitles WHERE task_id = ?1",
                params![request.task_id],
            )
            .map_err(|error| format!("无法删除字幕记录: {error}"))?;
        let changed = transaction
            .execute(
                "DELETE FROM home_video_tasks WHERE id = ?1",
                params![request.task_id],
            )
            .map_err(|error| format!("无法删除待办任务: {error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("无法提交删除操作: {error}"))?;
        Ok(changed)
    })?;

    if deleted == 0 {
        return Err("未找到待办任务".to_string());
    }

    if let Ok(task_dir) = app_paths::youtube_task_dir(&request.task_id) {
        if task_dir.exists() {
            let _ = fs::remove_dir_all(task_dir);
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn refresh_home_video_task_detail(
    app: AppHandle,
    request: HomeVideoTaskRequest,
) -> Result<HomeVideoTask, String> {
    match tauri::async_runtime::spawn_blocking(move || {
        refresh_home_video_task_detail_blocking(app, request)
    })
    .await
    {
        Ok(result) => result,
        Err(error) => Err(format!("视频详情任务执行失败: {error}")),
    }
}

#[tauri::command]
pub async fn download_home_video_task_subtitle(
    app: AppHandle,
    request: DownloadHomeVideoTaskSubtitleRequest,
) -> Result<HomeVideoTask, String> {
    match tauri::async_runtime::spawn_blocking(move || {
        download_home_video_task_subtitle_blocking(app, request)
    })
    .await
    {
        Ok(result) => result,
        Err(error) => Err(format!("字幕下载任务执行失败: {error}")),
    }
}

#[tauri::command]
pub async fn download_home_video_task_video(
    app: AppHandle,
    request: HomeVideoTaskRequest,
) -> Result<HomeVideoTask, String> {
    match tauri::async_runtime::spawn_blocking(move || {
        download_home_video_task_video_blocking(app, request)
    })
    .await
    {
        Ok(result) => result,
        Err(error) => Err(format!("视频下载任务执行失败: {error}")),
    }
}

fn refresh_home_video_task_detail_blocking(
    app: AppHandle,
    request: HomeVideoTaskRequest,
) -> Result<HomeVideoTask, String> {
    let store = app.state::<SettingsStore>();
    let task = store
        .with_connection(|connection| read_home_video_task_by_id(connection, &request.task_id))?;
    let proxy = store.load()?.youtube_monitor_proxy;
    let now = Utc::now().to_rfc3339();
    store.with_connection(|connection| {
        connection
            .execute(
                "
                UPDATE home_video_tasks
                SET detail_status = ?1,
                    error_message = '',
                    updated_at = ?2
                WHERE id = ?3
                ",
                params![DETAIL_STATUS_LOADING, now, task.id],
            )
            .map(|_| ())
            .map_err(|error| format!("无法更新详情读取状态: {error}"))
    })?;

    match fetch_video_detail(&task.url, &proxy) {
        Ok(detail) => {
            let checked_at = Utc::now().to_rfc3339();
            let subtitle_options = serde_json::to_string(&detail.subtitle_options)
                .unwrap_or_else(|_| "[]".to_string());
            let metadata =
                serde_json::to_string(&detail.metadata).unwrap_or_else(|_| "{}".to_string());
            store.with_connection(|connection| {
                connection
                    .execute(
                        "
                        UPDATE home_video_tasks
                        SET external_id = CASE WHEN ?2 != '' THEN ?2 ELSE external_id END,
                            title = CASE WHEN ?3 != '' THEN ?3 ELSE title END,
                            channel_title = ?4,
                            channel_url = ?5,
                            thumbnail_url = ?6,
                            duration = ?7,
                            webpage_url = ?8,
                            description = ?9,
                            view_count = ?10,
                            like_count = ?11,
                            comment_count = ?12,
                            upload_date = ?13,
                            detail_status = ?14,
                            subtitle_options = ?15,
                            metadata = ?16,
                            error_message = '',
                            updated_at = ?17,
                            detail_checked_at = ?18
                        WHERE id = ?1
                        ",
                        params![
                            task.id,
                            detail.external_id,
                            detail.title,
                            detail.channel_title,
                            detail.channel_url,
                            detail.thumbnail_url,
                            detail.duration,
                            detail.webpage_url,
                            detail.description,
                            detail.view_count,
                            detail.like_count,
                            detail.comment_count,
                            detail.upload_date,
                            DETAIL_STATUS_READY,
                            subtitle_options,
                            metadata,
                            checked_at,
                            checked_at,
                        ],
                    )
                    .map_err(|error| format!("无法保存视频详情: {error}"))?;
                read_home_video_task_by_id(connection, &request.task_id)
            })
        }
        Err(error) => {
            let checked_at = Utc::now().to_rfc3339();
            let compact = compact_error(&error);
            let _ = store.with_connection(|connection| {
                connection
                    .execute(
                        "
                        UPDATE home_video_tasks
                        SET detail_status = ?1,
                            error_message = ?2,
                            updated_at = ?3,
                            detail_checked_at = ?4
                        WHERE id = ?5
                        ",
                        params![
                            DETAIL_STATUS_FAILED,
                            compact,
                            checked_at,
                            checked_at,
                            task.id
                        ],
                    )
                    .map(|_| ())
                    .map_err(|error| format!("无法保存详情错误: {error}"))
            });
            Err(compact)
        }
    }
}

fn download_home_video_task_subtitle_blocking(
    app: AppHandle,
    request: DownloadHomeVideoTaskSubtitleRequest,
) -> Result<HomeVideoTask, String> {
    download_home_video_task_subtitle_internal(app, request)
}

pub(crate) fn download_home_video_task_subtitle_internal(
    app: AppHandle,
    request: DownloadHomeVideoTaskSubtitleRequest,
) -> Result<HomeVideoTask, String> {
    let store = app.state::<SettingsStore>();
    let task = store
        .with_connection(|connection| read_home_video_task_by_id(connection, &request.task_id))?;
    let source_kind = normalize_subtitle_source_kind(&request.source_kind);
    let option = task
        .subtitle_options
        .iter()
        .find(|option| option.language == request.language && option.source_kind == source_kind)
        .cloned()
        .ok_or_else(|| "未找到该字幕选项，请先读取视频详情".to_string())?;
    let proxy = store.load()?.youtube_monitor_proxy;
    let task_dir = app_paths::youtube_task_dir(&task.id)?;
    let subtitles_dir = task_dir.join("subtitles");
    fs::create_dir_all(&subtitles_dir).map_err(|error| format!("无法创建字幕目录: {error}"))?;
    let progress = DownloadProgressEmitter::subtitle(app.clone(), &task.id, &option);

    progress.emit_active(2, "准备下载字幕");
    let output = match download_subtitle_file(&task, &option, &proxy, &subtitles_dir, &progress) {
        Ok(output) => output,
        Err(error) => {
            progress.emit_failed(&error);
            return Err(error);
        }
    };
    let now = Utc::now().to_rfc3339();
    let updated_task = match store.with_connection(|connection| {
        upsert_home_video_subtitle(connection, &task.id, &option, output, &now)?;
        read_home_video_task_by_id(connection, &task.id)
    }) {
        Ok(task) => task,
        Err(error) => {
            progress.emit_failed(&error);
            return Err(error);
        }
    };
    progress.emit_done("字幕下载完成");
    Ok(updated_task)
}

fn download_home_video_task_video_blocking(
    app: AppHandle,
    request: HomeVideoTaskRequest,
) -> Result<HomeVideoTask, String> {
    download_home_video_task_video_internal(app, request)
}

pub(crate) fn download_home_video_task_video_internal(
    app: AppHandle,
    request: HomeVideoTaskRequest,
) -> Result<HomeVideoTask, String> {
    let store = app.state::<SettingsStore>();
    let task = store
        .with_connection(|connection| read_home_video_task_by_id(connection, &request.task_id))?;
    let proxy = store.load()?.youtube_monitor_proxy;
    let task_dir = app_paths::youtube_task_dir(&task.id)?;
    let videos_dir = task_dir.join("videos");
    fs::create_dir_all(&videos_dir).map_err(|error| format!("无法创建视频目录: {error}"))?;
    let progress = DownloadProgressEmitter::video(app.clone(), &task.id);

    progress.emit_active(2, "准备下载视频");
    let output = match download_video_file(&task, &proxy, &videos_dir, &progress) {
        Ok(output) => output,
        Err(error) => {
            progress.emit_failed(&error);
            return Err(error);
        }
    };
    let now = Utc::now().to_rfc3339();
    let updated_task = match store.with_connection(|connection| {
        upsert_home_video_download(connection, &task.id, output, &now)?;
        read_home_video_task_by_id(connection, &task.id)
    }) {
        Ok(task) => task,
        Err(error) => {
            progress.emit_failed(&error);
            return Err(error);
        }
    };
    progress.emit_done("视频下载完成");
    Ok(updated_task)
}

fn read_home_video_tasks(connection: &rusqlite::Connection) -> Result<Vec<HomeVideoTask>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, url, source_channel_id, source_video_id, external_id, title,
                   channel_title, channel_url, thumbnail_url, duration, webpage_url,
                   description, view_count, like_count, comment_count, upload_date,
                   detail_status, subtitle_options, metadata, error_message,
                   created_at, updated_at, detail_checked_at
            FROM home_video_tasks
            ORDER BY created_at DESC, id DESC
            ",
        )
        .map_err(|error| format!("无法读取待办任务: {error}"))?;
    let rows = statement
        .query_map([], map_home_video_task)
        .map_err(|error| format!("无法读取待办任务: {error}"))?;
    let mut tasks = Vec::new();
    for row in rows {
        let mut task = row.map_err(|error| format!("无法解析待办任务: {error}"))?;
        task.downloaded_subtitles = read_home_video_subtitles(connection, &task.id)?;
        task.downloaded_video = read_home_video_download(connection, &task.id)?;
        task.partial_video_download =
            read_home_partial_video_download(&task.id, task.downloaded_video.as_ref())?;
        tasks.push(task);
    }

    Ok(tasks)
}

pub(crate) fn read_home_video_task_by_id(
    connection: &rusqlite::Connection,
    task_id: &str,
) -> Result<HomeVideoTask, String> {
    let mut task = connection
        .query_row(
            "
            SELECT id, url, source_channel_id, source_video_id, external_id, title,
                   channel_title, channel_url, thumbnail_url, duration, webpage_url,
                   description, view_count, like_count, comment_count, upload_date,
                   detail_status, subtitle_options, metadata, error_message,
                   created_at, updated_at, detail_checked_at
            FROM home_video_tasks
            WHERE id = ?1
            ",
            params![task_id],
            map_home_video_task,
        )
        .optional()
        .map_err(|error| format!("无法读取待办任务: {error}"))?
        .ok_or_else(|| "未找到待办任务".to_string())?;
    task.downloaded_subtitles = read_home_video_subtitles(connection, &task.id)?;
    task.downloaded_video = read_home_video_download(connection, &task.id)?;
    task.partial_video_download =
        read_home_partial_video_download(&task.id, task.downloaded_video.as_ref())?;
    Ok(task)
}

fn map_home_video_task(row: &Row<'_>) -> rusqlite::Result<HomeVideoTask> {
    let subtitle_options_text: String = row.get(17)?;
    let metadata_text: String = row.get(18)?;
    let mut subtitle_options =
        serde_json::from_str::<Vec<HomeVideoSubtitleOption>>(&subtitle_options_text)
            .unwrap_or_default();
    normalize_stored_subtitle_options(&mut subtitle_options);
    let metadata = serde_json::from_str::<Value>(&metadata_text).unwrap_or_else(|_| json!({}));

    Ok(HomeVideoTask {
        id: row.get(0)?,
        url: row.get(1)?,
        source_channel_id: row.get(2)?,
        source_video_id: row.get(3)?,
        external_id: row.get(4)?,
        title: row.get(5)?,
        channel_title: row.get(6)?,
        channel_url: row.get(7)?,
        thumbnail_url: row.get(8)?,
        duration: row.get(9)?,
        webpage_url: row.get(10)?,
        description: row.get(11)?,
        view_count: row.get(12)?,
        like_count: row.get(13)?,
        comment_count: row.get(14)?,
        upload_date: row.get(15)?,
        detail_status: row.get(16)?,
        subtitle_options,
        metadata,
        error_message: row.get(19)?,
        created_at: row.get(20)?,
        updated_at: row.get(21)?,
        detail_checked_at: row.get(22)?,
        downloaded_subtitles: Vec::new(),
        downloaded_video: None,
        partial_video_download: None,
    })
}

fn read_home_video_subtitles(
    connection: &rusqlite::Connection,
    task_id: &str,
) -> Result<Vec<HomeVideoSubtitle>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, task_id, language, language_name, source_kind, format,
                   file_path, file_size, created_at, updated_at
            FROM home_video_task_subtitles
            WHERE task_id = ?1
            ORDER BY datetime(updated_at) DESC
            ",
        )
        .map_err(|error| format!("无法读取字幕记录: {error}"))?;
    let rows = statement
        .query_map(params![task_id], |row| {
            Ok(HomeVideoSubtitle {
                id: row.get(0)?,
                task_id: row.get(1)?,
                language: row.get(2)?,
                language_name: row.get(3)?,
                source_kind: row.get(4)?,
                format: row.get(5)?,
                file_path: row.get(6)?,
                file_size: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })
        .map_err(|error| format!("无法读取字幕记录: {error}"))?;
    let mut subtitles = Vec::new();
    for row in rows {
        subtitles.push(row.map_err(|error| format!("无法解析字幕记录: {error}"))?);
    }

    Ok(subtitles)
}

fn read_home_video_download(
    connection: &rusqlite::Connection,
    task_id: &str,
) -> Result<Option<HomeVideoDownload>, String> {
    connection
        .query_row(
            "
            SELECT id, task_id, format, file_path, file_name, file_size, created_at, updated_at
            FROM home_video_task_videos
            WHERE task_id = ?1
            LIMIT 1
            ",
            params![task_id],
            |row| {
                Ok(HomeVideoDownload {
                    id: row.get(0)?,
                    task_id: row.get(1)?,
                    format: row.get(2)?,
                    file_path: row.get(3)?,
                    file_name: row.get(4)?,
                    file_size: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            },
        )
        .optional()
        .map_err(|error| format!("无法读取视频下载记录: {error}"))
}

fn read_home_partial_video_download(
    task_id: &str,
    downloaded_video: Option<&HomeVideoDownload>,
) -> Result<Option<HomePartialVideoDownload>, String> {
    if downloaded_video.is_some() {
        return Ok(None);
    }

    let videos_dir = app_paths::youtube_task_dir(task_id)?.join("videos");
    let prefix = home_video_prefix(task_id);
    scan_partial_video_download(&videos_dir, &prefix)
}

fn scan_partial_video_download(
    dir: &Path,
    prefix: &str,
) -> Result<Option<HomePartialVideoDownload>, String> {
    if !dir.exists() {
        return Ok(None);
    }

    let mut file_count = 0_usize;
    let mut file_size = 0_u64;
    let mut updated_at: Option<SystemTime> = None;

    for entry in fs::read_dir(dir).map_err(|error| format!("无法读取视频目录: {error}"))? {
        let entry = entry.map_err(|error| format!("无法读取视频文件: {error}"))?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !path.is_file() || !is_partial_video_download_file_name(name, prefix) {
            continue;
        }

        let metadata =
            fs::metadata(&path).map_err(|error| format!("无法读取未完成视频文件: {error}"))?;
        file_count += 1;
        file_size = file_size.saturating_add(metadata.len());
        if let Ok(modified_at) = metadata.modified() {
            updated_at = match updated_at {
                Some(current) if current >= modified_at => Some(current),
                _ => Some(modified_at),
            };
        }
    }

    if file_count == 0 {
        return Ok(None);
    }

    Ok(Some(HomePartialVideoDownload {
        file_count,
        file_size: file_size.min(i64::MAX as u64) as i64,
        updated_at: updated_at.map(|value| DateTime::<Utc>::from(value).to_rfc3339()),
    }))
}

fn upsert_home_video_subtitle(
    connection: &rusqlite::Connection,
    task_id: &str,
    option: &HomeVideoSubtitleOption,
    output: SubtitleDownloadOutput,
    now: &str,
) -> Result<(), String> {
    let file_path = output.path.to_string_lossy().to_string();
    let existing_id = connection
        .query_row(
            "
            SELECT id
            FROM home_video_task_subtitles
            WHERE task_id = ?1 AND language = ?2 AND source_kind = ?3
            LIMIT 1
            ",
            params![task_id, option.language, option.source_kind],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("无法检查字幕记录: {error}"))?;

    if let Some(existing_id) = existing_id {
        connection
            .execute(
                "
                UPDATE home_video_task_subtitles
                SET language_name = ?1,
                    format = ?2,
                    file_path = ?3,
                    file_size = ?4,
                    updated_at = ?5
                WHERE id = ?6
                ",
                params![
                    option.name,
                    output.format,
                    file_path,
                    output.file_size,
                    now,
                    existing_id,
                ],
            )
            .map(|_| ())
            .map_err(|error| format!("无法更新字幕记录: {error}"))?;
        return Ok(());
    }

    connection
        .execute(
            "
            INSERT INTO home_video_task_subtitles (
                id, task_id, language, language_name, source_kind, format,
                file_path, file_size, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ",
            params![
                Uuid::new_v4().to_string(),
                task_id,
                option.language,
                option.name,
                option.source_kind,
                output.format,
                file_path,
                output.file_size,
                now,
                now,
            ],
        )
        .map(|_| ())
        .map_err(|error| format!("无法保存字幕记录: {error}"))
}

fn upsert_home_video_download(
    connection: &rusqlite::Connection,
    task_id: &str,
    output: VideoDownloadOutput,
    now: &str,
) -> Result<(), String> {
    let file_path = output.path.to_string_lossy().to_string();
    let existing_id = connection
        .query_row(
            "
            SELECT id
            FROM home_video_task_videos
            WHERE task_id = ?1
            LIMIT 1
            ",
            params![task_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("无法检查视频下载记录: {error}"))?;

    if let Some(existing_id) = existing_id {
        connection
            .execute(
                "
                UPDATE home_video_task_videos
                SET format = ?1,
                    file_path = ?2,
                    file_name = ?3,
                    file_size = ?4,
                    updated_at = ?5
                WHERE id = ?6
                ",
                params![
                    output.format,
                    file_path,
                    output.file_name,
                    output.file_size,
                    now,
                    existing_id,
                ],
            )
            .map(|_| ())
            .map_err(|error| format!("无法更新视频下载记录: {error}"))?;
        return Ok(());
    }

    connection
        .execute(
            "
            INSERT INTO home_video_task_videos (
                id, task_id, format, file_path, file_name, file_size, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                Uuid::new_v4().to_string(),
                task_id,
                output.format,
                file_path,
                output.file_name,
                output.file_size,
                now,
                now,
            ],
        )
        .map(|_| ())
        .map_err(|error| format!("无法保存视频下载记录: {error}"))
}

fn fetch_video_detail(url: &str, proxy: &str) -> Result<VideoDetail, String> {
    let mut command = ytdlp_command(proxy);
    command.args([
        "--dump-single-json",
        "--no-playlist",
        "--skip-download",
        "--ignore-no-formats-error",
        "--no-warnings",
        "--no-progress",
        "--extractor-args",
        YTDLP_YOUTUBE_DETAIL_EXTRACTOR_ARGS,
        "--add-headers",
        YOUTUBE_ACCEPT_LANGUAGE,
        "--socket-timeout",
        YTDLP_SOCKET_TIMEOUT_SECONDS,
        url,
    ]);
    let output = command
        .output()
        .map_err(|error| format!("无法启动 yt-dlp: {error}"))?;

    if !output.status.success() {
        return Err(stderr_or_default(&output.stderr, "yt-dlp 读取视频详情失败"));
    }

    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("无法解析 yt-dlp 视频详情: {error}"))?;
    let external_id = first_non_empty(&[
        string_field(&value, "id").unwrap_or_default(),
        string_field(&value, "display_id").unwrap_or_default(),
        youtube_video_id_from_url(url).unwrap_or_default(),
    ]);
    let title = string_field(&value, "title").unwrap_or_else(|| "未命名视频".to_string());
    let channel_title = first_non_empty(&[
        string_field(&value, "channel").unwrap_or_default(),
        string_field(&value, "uploader").unwrap_or_default(),
    ]);
    let channel_url = first_non_empty(&[
        string_field(&value, "channel_url").unwrap_or_default(),
        string_field(&value, "uploader_url").unwrap_or_default(),
    ]);
    let remote_thumbnail_url = first_non_empty(&[
        thumbnail_from_list(&value).unwrap_or_default(),
        string_field(&value, "thumbnail").unwrap_or_default(),
    ]);
    let thumbnail_url = if remote_thumbnail_url.is_empty() {
        String::new()
    } else {
        download_thumbnail_data_url(&remote_thumbnail_url, proxy).unwrap_or_default()
    };
    let webpage_url = first_non_empty(&[
        string_field(&value, "webpage_url").unwrap_or_default(),
        string_field(&value, "original_url").unwrap_or_default(),
        url.to_string(),
    ]);

    Ok(VideoDetail {
        external_id,
        title,
        channel_title,
        channel_url,
        thumbnail_url,
        duration: number_field(&value, "duration"),
        webpage_url,
        description: string_field(&value, "description").unwrap_or_default(),
        view_count: integer_field(&value, "view_count"),
        like_count: integer_field(&value, "like_count"),
        comment_count: integer_field(&value, "comment_count"),
        upload_date: string_field(&value, "upload_date").unwrap_or_default(),
        subtitle_options: extract_subtitle_options(&value),
        metadata: metadata_summary(&value),
    })
}

fn download_subtitle_file(
    task: &HomeVideoTask,
    option: &HomeVideoSubtitleOption,
    proxy: &str,
    subtitles_dir: &Path,
    progress: &DownloadProgressEmitter,
) -> Result<SubtitleDownloadOutput, String> {
    let prefix = format!(
        "{}.{}.{}",
        sanitize_file_segment(&task.id),
        sanitize_file_segment(&option.source_kind),
        sanitize_file_segment(&option.language)
    );
    remove_matching_outputs(subtitles_dir, &prefix)?;

    let output_template = subtitles_dir.join(format!("{prefix}.%(ext)s"));
    let output_template = output_template.to_string_lossy().to_string();
    let mut command = ytdlp_command(proxy);
    command.args([
        "--skip-download",
        "--ignore-no-formats-error",
        "--no-playlist",
        "--no-warnings",
        "--extractor-args",
        YTDLP_YOUTUBE_DETAIL_EXTRACTOR_ARGS,
        "--add-headers",
        YOUTUBE_ACCEPT_LANGUAGE,
        "--socket-timeout",
        YTDLP_SOCKET_TIMEOUT_SECONDS,
    ]);
    add_ytdlp_progress_args(&mut command);
    if option.source_kind == SUBTITLE_SOURCE_AUTOMATIC {
        command.arg("--write-auto-subs");
    } else {
        command.arg("--write-subs");
    }
    command.args([
        "--sub-langs",
        &option.language,
        "--sub-format",
        "srt/vtt/ttml/ass/best",
        "-o",
        &output_template,
        &task.url,
    ]);
    progress.emit_active(10, "字幕下载中");
    run_ytdlp_download_command(&mut command, progress, "yt-dlp 下载字幕失败")?;
    progress.emit_active(92, "字幕写入完成");

    let path = find_subtitle_output(subtitles_dir, &prefix)?;
    let metadata = fs::metadata(&path).map_err(|error| format!("无法读取字幕文件: {error}"))?;
    let format = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();

    Ok(SubtitleDownloadOutput {
        path,
        format,
        file_size: metadata.len().min(i64::MAX as u64) as i64,
    })
}

fn download_video_file(
    task: &HomeVideoTask,
    proxy: &str,
    videos_dir: &Path,
    progress: &DownloadProgressEmitter,
) -> Result<VideoDownloadOutput, String> {
    let prefix = home_video_prefix(&task.id);
    let output_template = videos_dir.join(format!("{prefix}.%(ext)s"));
    let output_template = output_template.to_string_lossy().to_string();
    let mut command = ytdlp_command(proxy);
    command.args([
        "--no-playlist",
        "--no-warnings",
        "--extractor-args",
        YTDLP_YOUTUBE_EXTRACTOR_ARGS,
        "--add-headers",
        YOUTUBE_ACCEPT_LANGUAGE,
        "--socket-timeout",
        YTDLP_SOCKET_TIMEOUT_SECONDS,
        "-f",
        "bv*+ba/best",
        "--merge-output-format",
        "mp4",
        "-o",
        &output_template,
    ]);
    if task.downloaded_video.is_some() {
        command.arg("--force-overwrites");
    } else {
        command.args(["--continue", "--part"]);
    }
    add_ytdlp_progress_args(&mut command);
    command.arg(&task.url);
    progress.emit_active(8, "视频下载中");
    run_ytdlp_download_command(&mut command, progress, "yt-dlp 下载视频失败")?;
    progress.emit_active(98, "视频文件处理中");

    let path = find_video_output(videos_dir, &prefix)?;
    remove_other_matching_outputs(videos_dir, &prefix, &path)?;
    let metadata = fs::metadata(&path).map_err(|error| format!("无法读取视频文件: {error}"))?;
    let format = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("视频文件")
        .to_string();

    Ok(VideoDownloadOutput {
        path,
        file_name,
        format,
        file_size: metadata.len().min(i64::MAX as u64) as i64,
    })
}

fn add_ytdlp_progress_args(command: &mut Command) {
    command.args([
        "--newline",
        "--progress",
        "--progress-delta",
        "0.2",
        "--progress-template",
        YTDLP_DOWNLOAD_PROGRESS_TEMPLATE,
        "--progress-template",
        YTDLP_POSTPROCESS_PROGRESS_TEMPLATE,
    ]);
}

fn run_ytdlp_download_command(
    command: &mut Command,
    progress: &DownloadProgressEmitter,
    failure_message: &str,
) -> Result<(), String> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("无法启动 yt-dlp: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取 yt-dlp 输出".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "无法读取 yt-dlp 错误输出".to_string())?;
    let (sender, receiver) = mpsc::channel::<String>();
    let stdout_reader = spawn_output_reader(stdout, sender.clone());
    let stderr_reader = spawn_output_reader(stderr, sender);
    let mut output_lines = Vec::new();
    let status = loop {
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => handle_ytdlp_output_line(&line, progress, &mut output_lines),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {}
        }

        match child
            .try_wait()
            .map_err(|error| format!("无法等待 yt-dlp 任务: {error}"))?
        {
            Some(status) => break status,
            None => continue,
        }
    };

    let _ = stdout_reader.join();
    let _ = stderr_reader.join();
    for line in receiver.try_iter() {
        handle_ytdlp_output_line(&line, progress, &mut output_lines);
    }

    if !status.success() {
        return Err(lines_or_default(&output_lines, failure_message));
    }

    Ok(())
}

fn spawn_output_reader<R: Read + Send + 'static>(
    reader: R,
    sender: mpsc::Sender<String>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for line in BufReader::new(reader).lines().map_while(Result::ok) {
            let _ = sender.send(line);
        }
    })
}

fn handle_ytdlp_output_line(
    line: &str,
    progress: &DownloadProgressEmitter,
    output_lines: &mut Vec<String>,
) {
    if let Some(parsed) = parse_ytdlp_progress_line(line) {
        progress.emit_ytdlp_progress(&parsed);
        return;
    }

    let line = line.trim();
    if !line.is_empty() {
        output_lines.push(line.to_string());
    }
}

fn parse_ytdlp_progress_line(line: &str) -> Option<YtdlpProgressLine> {
    let trimmed = line.trim();
    if !trimmed.starts_with(YTDLP_PROGRESS_PREFIX) {
        return None;
    }

    let parts = trimmed.split('\t').collect::<Vec<_>>();
    if parts.len() < 7 {
        return None;
    }

    let downloaded_bytes = parse_optional_u64(parts[3]);
    let total_bytes = parse_optional_u64(parts[4]).or_else(|| parse_optional_u64(parts[5]));
    let progress = parse_percent(parts[6]).or_else(|| {
        downloaded_bytes.and_then(|downloaded| {
            total_bytes
                .filter(|total| *total > 0)
                .map(|total| ((downloaded as f64 / total as f64) * 100.0).round() as u8)
        })
    });

    Some(YtdlpProgressLine {
        phase: parts[1].trim().to_string(),
        status: parts[2].trim().to_string(),
        progress,
        downloaded_bytes,
        total_bytes,
    })
}

fn parse_optional_u64(value: &str) -> Option<u64> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("NA") || value.eq_ignore_ascii_case("none") {
        return None;
    }

    value.parse::<u64>().ok()
}

fn parse_percent(value: &str) -> Option<u8> {
    let numeric = value
        .chars()
        .filter(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect::<String>();
    let value = numeric.parse::<f64>().ok()?;
    Some(value.round().clamp(0.0, 100.0) as u8)
}

fn download_progress_message(kind: &str, phase: &str, status: &str) -> String {
    if phase == "postprocess" {
        return if kind == DOWNLOAD_KIND_VIDEO {
            "视频封装中".to_string()
        } else {
            "字幕写入中".to_string()
        };
    }

    if status == "finished" {
        return if kind == DOWNLOAD_KIND_VIDEO {
            "视频下载完成，正在处理文件".to_string()
        } else {
            "字幕下载完成，正在写入文件".to_string()
        };
    }

    if kind == DOWNLOAD_KIND_VIDEO {
        "视频下载中".to_string()
    } else {
        "字幕下载中".to_string()
    }
}

fn download_thumbnail_data_url(url: &str, proxy: &str) -> Result<String, String> {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("Mozilla/5.0 Link/0.1 yt-dlp-thumbnail"),
    );
    headers.insert(
        ACCEPT,
        HeaderValue::from_static(
            "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8",
        ),
    );

    let mut builder = Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(THUMBNAIL_DOWNLOAD_TIMEOUT_SECONDS));
    let proxy = proxy.trim();
    if !proxy.is_empty() {
        builder = builder.proxy(
            reqwest::Proxy::all(proxy).map_err(|error| format!("封面代理配置无效: {error}"))?,
        );
    }
    let client = builder
        .build()
        .map_err(|error| format!("无法创建封面下载客户端: {error}"))?;
    let response = client
        .get(url)
        .send()
        .map_err(|error| format!("无法下载视频封面: {error}"))?;

    if !response.status().is_success() {
        return Err(format!("视频封面下载失败: HTTP {}", response.status()));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| value.starts_with("image/"))
        .unwrap_or("image/jpeg")
        .to_string();
    let bytes = response
        .bytes()
        .map_err(|error| format!("无法读取视频封面: {error}"))?;
    if bytes.len() > MAX_THUMBNAIL_BYTES {
        return Err("视频封面过大".to_string());
    }

    Ok(format!(
        "data:{content_type};base64,{}",
        BASE64_STANDARD.encode(bytes)
    ))
}

fn extract_subtitle_options(value: &Value) -> Vec<HomeVideoSubtitleOption> {
    let mut options = Vec::new();
    let mut seen = HashSet::new();
    collect_subtitle_options(
        value.get("subtitles"),
        SUBTITLE_SOURCE_MANUAL,
        false,
        &mut options,
        &mut seen,
    );
    collect_subtitle_options(
        value.get("automatic_captions"),
        SUBTITLE_SOURCE_AUTOMATIC,
        true,
        &mut options,
        &mut seen,
    );
    options.sort_by(|left, right| {
        subtitle_source_rank(&left.source_kind)
            .cmp(&subtitle_source_rank(&right.source_kind))
            .then_with(|| left.language.cmp(&right.language))
    });
    options
}

fn subtitle_source_rank(source_kind: &str) -> u8 {
    if source_kind == SUBTITLE_SOURCE_MANUAL {
        0
    } else {
        1
    }
}

fn collect_subtitle_options(
    value: Option<&Value>,
    source_kind: &str,
    is_auto: bool,
    options: &mut Vec<HomeVideoSubtitleOption>,
    seen: &mut HashSet<String>,
) {
    let Some(object) = value.and_then(Value::as_object) else {
        return;
    };

    for (language, formats_value) in object {
        let language = language.trim();
        if language.is_empty() {
            continue;
        }
        let key = format!("{source_kind}:{language}");
        if !seen.insert(key) {
            continue;
        }

        let format_items = formats_value.as_array().cloned().unwrap_or_default();
        if source_kind == SUBTITLE_SOURCE_AUTOMATIC
            && should_skip_automatic_caption(language, &format_items, object)
        {
            continue;
        }

        let name = format_items
            .iter()
            .find_map(|item| string_field(item, "name"))
            .unwrap_or_else(|| language.to_string());
        let mut formats = format_items
            .iter()
            .filter_map(|item| string_field(item, "ext"))
            .collect::<Vec<_>>();
        formats.sort();
        formats.dedup();
        if formats.is_empty() {
            formats.push("best".to_string());
        }

        options.push(HomeVideoSubtitleOption {
            language: language.to_string(),
            name,
            source_kind: source_kind.to_string(),
            formats,
            is_auto,
        });
    }
}

fn should_skip_automatic_caption(
    language: &str,
    format_items: &[Value],
    captions: &serde_json::Map<String, Value>,
) -> bool {
    if subtitle_formats_have_query_param(format_items, "tlang") {
        return true;
    }

    if language.ends_with("-orig") {
        return false;
    }

    let original_language = format!("{language}-orig");
    captions
        .get(&original_language)
        .and_then(Value::as_array)
        .is_some_and(|items| !subtitle_formats_have_query_param(items, "tlang"))
}

fn subtitle_formats_have_query_param(format_items: &[Value], param_name: &str) -> bool {
    format_items
        .iter()
        .filter_map(|item| string_field(item, "url"))
        .any(|url| url_has_query_param(&url, param_name))
}

fn url_has_query_param(url: &str, param_name: &str) -> bool {
    let query = url
        .split_once('?')
        .map(|(_, query)| query)
        .unwrap_or_default()
        .split('#')
        .next()
        .unwrap_or_default();

    query.split('&').any(|part| {
        let name = part.split_once('=').map(|(name, _)| name).unwrap_or(part);
        name == param_name
    })
}

fn normalize_stored_subtitle_options(options: &mut Vec<HomeVideoSubtitleOption>) {
    let has_original_auto_caption = options.iter().any(|option| {
        option.source_kind == SUBTITLE_SOURCE_AUTOMATIC && option.language.ends_with("-orig")
    });
    if !has_original_auto_caption {
        return;
    }

    options.retain(|option| {
        option.source_kind != SUBTITLE_SOURCE_AUTOMATIC || option.language.ends_with("-orig")
    });
}

fn normalize_youtube_video_url(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("请输入 YouTube 视频地址".to_string());
    }

    let url = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };

    let Some(video_id) = youtube_video_id_from_url(&url) else {
        return Err("请输入有效的 YouTube 视频地址".to_string());
    };

    Ok(format!("https://www.youtube.com/watch?v={video_id}"))
}

fn youtube_video_id_from_url(url: &str) -> Option<String> {
    let lower = url.to_lowercase();
    if let Some(value) = segment_after_marker(url, &lower, "youtu.be/") {
        return clean_video_id(&value);
    }
    if lower.contains("youtube.com/watch") || lower.contains("music.youtube.com/watch") {
        return query_param(url, "v").and_then(|value| clean_video_id(&value));
    }
    if let Some(value) = segment_after_marker(url, &lower, "youtube.com/shorts/") {
        return clean_video_id(&value);
    }
    if let Some(value) = segment_after_marker(url, &lower, "youtube.com/embed/") {
        return clean_video_id(&value);
    }

    None
}

fn segment_after_marker(url: &str, lower: &str, marker: &str) -> Option<String> {
    let start = lower.find(marker)? + marker.len();
    Some(url[start..].to_string())
}

fn clean_video_id(value: &str) -> Option<String> {
    let id = value
        .split(['?', '&', '/', '#'])
        .next()
        .unwrap_or_default()
        .trim();
    if id.is_empty() || id.contains('.') {
        None
    } else {
        Some(id.to_string())
    }
}

fn query_param(url: &str, key: &str) -> Option<String> {
    let query = url.split('?').nth(1)?;
    for part in query.split('&') {
        let mut pieces = part.splitn(2, '=');
        let name = pieces.next().unwrap_or_default();
        let value = pieces.next().unwrap_or_default();
        if name == key && !value.trim().is_empty() {
            return Some(value.to_string());
        }
    }

    None
}

fn ytdlp_command(proxy: &str) -> std::process::Command {
    let mut command = create_command(YTDLP_COMMAND);

    let proxy = proxy.trim();
    if !proxy.is_empty() {
        command.args(["--proxy", proxy]);
    }

    command
}

fn normalize_subtitle_source_kind(value: &str) -> String {
    if value == SUBTITLE_SOURCE_AUTOMATIC {
        SUBTITLE_SOURCE_AUTOMATIC.to_string()
    } else {
        SUBTITLE_SOURCE_MANUAL.to_string()
    }
}

fn subtitle_key(option: &HomeVideoSubtitleOption) -> String {
    format!("{}:{}", option.source_kind, option.language)
}

fn remove_matching_outputs(dir: &Path, prefix: &str) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir).map_err(|error| format!("无法读取字幕目录: {error}"))? {
        let entry = entry.map_err(|error| format!("无法读取字幕文件: {error}"))?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.starts_with(prefix) && path.is_file() {
            fs::remove_file(&path).map_err(|error| format!("无法清理旧字幕文件: {error}"))?;
        }
    }

    Ok(())
}

fn find_subtitle_output(dir: &Path, prefix: &str) -> Result<PathBuf, String> {
    let mut matches = Vec::new();
    for entry in fs::read_dir(dir).map_err(|error| format!("无法读取字幕目录: {error}"))? {
        let entry = entry.map_err(|error| format!("无法读取字幕文件: {error}"))?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.starts_with(prefix) && path.is_file() {
            matches.push(path);
        }
    }
    matches.sort_by_key(|path| {
        fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    matches
        .pop()
        .ok_or_else(|| "yt-dlp 未生成字幕文件".to_string())
}

fn find_video_output(dir: &Path, prefix: &str) -> Result<PathBuf, String> {
    let mut matches = Vec::new();
    for entry in fs::read_dir(dir).map_err(|error| format!("无法读取视频目录: {error}"))? {
        let entry = entry.map_err(|error| format!("无法读取视频文件: {error}"))?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.starts_with(prefix) && path.is_file() && is_video_output_path(&path) {
            matches.push(path);
        }
    }
    if let Some(final_path) = matches
        .iter()
        .find(|path| path.file_stem().and_then(|value| value.to_str()) == Some(prefix))
    {
        return Ok(final_path.clone());
    }
    matches.sort_by_key(|path| {
        fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    matches
        .pop()
        .ok_or_else(|| "yt-dlp 未生成视频文件".to_string())
}

fn remove_other_matching_outputs(dir: &Path, prefix: &str, keep_path: &Path) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir).map_err(|error| format!("无法读取输出目录: {error}"))? {
        let entry = entry.map_err(|error| format!("无法读取输出文件: {error}"))?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.starts_with(prefix) && path.is_file() && path != keep_path {
            fs::remove_file(&path).map_err(|error| format!("无法清理旧输出文件: {error}"))?;
        }
    }

    Ok(())
}

fn is_video_output_path(path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(extension.as_str(), "mp4" | "mkv" | "webm" | "mov" | "m4v")
}

fn home_video_prefix(task_id: &str) -> String {
    format!("{}.video", sanitize_file_segment(task_id))
}

fn is_partial_video_download_file_name(name: &str, prefix: &str) -> bool {
    if !name.starts_with(prefix) {
        return false;
    }

    let name = name.to_ascii_lowercase();
    name.ends_with(".part") || name.ends_with(".ytdl") || name.contains(".part-")
}

fn sanitize_file_segment(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "subtitle".to_string()
    } else {
        sanitized
    }
}

fn metadata_summary(value: &Value) -> Value {
    json!({
        "extractor": string_field(value, "extractor").unwrap_or_default(),
        "extractorKey": string_field(value, "extractor_key").unwrap_or_default(),
        "liveStatus": string_field(value, "live_status").unwrap_or_default(),
        "availability": string_field(value, "availability").unwrap_or_default(),
    })
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn number_field(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_i64().map(|value| value as f64))
            .or_else(|| value.as_str().and_then(|value| value.parse::<f64>().ok()))
    })
}

fn integer_field(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| value.as_f64().map(|value| value as i64))
            .or_else(|| value.as_str().and_then(|value| value.parse::<i64>().ok()))
    })
}

fn thumbnail_from_list(value: &Value) -> Option<String> {
    value
        .get("thumbnails")
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .rev()
                .find_map(|item| string_field(item, "url"))
        })
}

fn first_non_empty(values: &[String]) -> String {
    values
        .iter()
        .find(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_default()
}

fn stderr_or_default(stderr: &[u8], fallback: &str) -> String {
    let message = String::from_utf8_lossy(stderr).trim().to_string();
    if message.is_empty() {
        fallback.to_string()
    } else {
        compact_error(&message)
    }
}

fn lines_or_default(lines: &[String], fallback: &str) -> String {
    let message = lines
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if message.is_empty() {
        fallback.to_string()
    } else {
        compact_error(&message)
    }
}

fn compact_error(error: &str) -> String {
    let lines = error
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let selected = lines
        .iter()
        .copied()
        .filter(|line| is_relevant_error_line(line))
        .collect::<Vec<_>>();
    let compact = if selected.is_empty() { lines } else { selected }
        .into_iter()
        .take(3)
        .collect::<Vec<_>>()
        .join("；");

    if compact.is_empty() {
        "操作失败".to_string()
    } else {
        compact
    }
}

fn is_relevant_error_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("error:")
        || lower.contains("failed")
        || lower.contains("unable")
        || lower.contains("timed out")
        || lower.contains("http error")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_yt_dlp_partial_video_files() {
        let prefix = "task.video";

        assert!(is_partial_video_download_file_name(
            "task.video.f399.mp4.part",
            prefix
        ));
        assert!(is_partial_video_download_file_name(
            "task.video.f399.mp4.part-Frag12",
            prefix
        ));
        assert!(is_partial_video_download_file_name(
            "task.video.f399.mp4.ytdl",
            prefix
        ));
        assert!(!is_partial_video_download_file_name(
            "task.video.mp4",
            prefix
        ));
        assert!(!is_partial_video_download_file_name(
            "other.video.f399.mp4.part",
            prefix
        ));
    }

    #[test]
    fn scans_partial_video_download_summary() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(dir.path().join("task.video.f399.mp4.part"), vec![0_u8; 10]).expect("write part");
        fs::write(dir.path().join("task.video.f140.m4a.ytdl"), b"12345").expect("write ytdl");
        fs::write(dir.path().join("task.video.mp4"), vec![0_u8; 100]).expect("write final");
        fs::write(dir.path().join("other.video.f399.mp4.part"), vec![0_u8; 7])
            .expect("write other");

        let partial = scan_partial_video_download(dir.path(), "task.video")
            .expect("scan partial")
            .expect("partial summary");

        assert_eq!(partial.file_count, 2);
        assert_eq!(partial.file_size, 15);
        assert!(partial.updated_at.is_some());
    }

    #[test]
    fn compact_error_prefers_relevant_yt_dlp_errors() {
        let error = "[youtube] Extracting URL: https://www.youtube.com/watch?v=abc\n\
            [youtube] abc: Downloading webpage\n\
            [youtube] abc: Downloading android vr player API JSON\n\
            ERROR: [youtube] abc: Sign in to confirm you are not a bot";

        let compact = compact_error(error);

        assert_eq!(
            compact,
            "ERROR: [youtube] abc: Sign in to confirm you are not a bot"
        );
    }
}
