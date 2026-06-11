use chrono::Utc;
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::settings::SettingsStore;

const YTDLP_COMMAND: &str = "yt-dlp";
const REFRESH_EVENT: &str = "youtube-monitor-refresh";
const CHANNEL_STATUS_IDLE: &str = "idle";
const CHANNEL_STATUS_CHECKING: &str = "checking";
const CHANNEL_STATUS_FAILED: &str = "failed";
const RUN_STATUS_RUNNING: &str = "running";
const RUN_STATUS_DONE: &str = "done";
const RUN_STATUS_FAILED: &str = "failed";
const DEFAULT_VIDEO_PAGE_SIZE: u32 = 100;
const MAX_VIDEO_PAGE_SIZE: u32 = 200;
const YTDLP_SOCKET_TIMEOUT_SECONDS: &str = "30";
const YTDLP_YOUTUBE_LANGUAGE_ARGS: &str = "youtube:lang=zh-CN";
const YOUTUBE_ACCEPT_LANGUAGE: &str = "Accept-Language: zh-CN,zh;q=0.9,en;q=0.8";

pub struct YoutubeMonitorService {
    running_channels: Mutex<HashSet<String>>,
}

impl YoutubeMonitorService {
    pub fn new() -> Self {
        Self {
            running_channels: Mutex::new(HashSet::new()),
        }
    }

    fn acquire_channel(&self, channel_id: &str) -> Result<RefreshGuard<'_>, String> {
        let mut running = self
            .running_channels
            .lock()
            .map_err(|error| format!("监控任务锁定失败: {error}"))?;

        if !running.insert(channel_id.to_string()) {
            return Err("该博主正在检查更新".to_string());
        }

        Ok(RefreshGuard {
            service: self,
            channel_id: channel_id.to_string(),
        })
    }

    fn release_channel(&self, channel_id: &str) {
        if let Ok(mut running) = self.running_channels.lock() {
            running.remove(channel_id);
        }
    }
}

struct RefreshGuard<'a> {
    service: &'a YoutubeMonitorService,
    channel_id: String,
}

impl Drop for RefreshGuard<'_> {
    fn drop(&mut self) {
        self.service.release_channel(&self.channel_id);
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YtdlpStatus {
    pub is_available: bool,
    pub version: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeChannel {
    pub id: String,
    pub url: String,
    pub canonical_url: String,
    pub external_id: String,
    pub title: String,
    pub handle: String,
    pub description: String,
    pub thumbnail_url: String,
    pub status: String,
    pub last_checked_at: Option<String>,
    pub last_success_at: Option<String>,
    pub last_error: String,
    pub video_count: u64,
    pub unread_count: u64,
    pub created_at: String,
    pub updated_at: String,
    pub latest_video_title: String,
    pub latest_video_url: String,
    pub latest_video_seen_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeVideo {
    pub id: String,
    pub channel_id: String,
    pub external_id: String,
    pub title: String,
    pub url: String,
    pub duration: Option<f64>,
    pub is_unread: bool,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeRefreshRun {
    pub id: String,
    pub channel_id: String,
    pub status: String,
    pub processed_count: u64,
    pub inserted_count: u64,
    pub updated_count: u64,
    pub message: String,
    pub error_message: String,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeVideoPage {
    pub items: Vec<YoutubeVideo>,
    pub total: u64,
    pub has_more: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddYoutubeChannelRequest {
    pub url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeChannelRequest {
    pub channel_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListYoutubeVideosRequest {
    pub channel_id: String,
    pub query: Option<String>,
    pub unread_only: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug)]
struct ChannelProbe {
    url: String,
    canonical_url: String,
    external_id: String,
    title: String,
    handle: String,
    description: String,
    thumbnail_url: String,
}

#[derive(Debug)]
struct VideoEntry {
    external_id: String,
    title: String,
    url: String,
    duration: Option<f64>,
    metadata: Value,
}

#[tauri::command]
pub fn get_ytdlp_status() -> YtdlpStatus {
    match Command::new(YTDLP_COMMAND).arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            YtdlpStatus {
                is_available: true,
                version: version.clone(),
                message: if version.is_empty() {
                    "yt-dlp 可用".to_string()
                } else {
                    format!("yt-dlp {version}")
                },
            }
        }
        Ok(output) => YtdlpStatus {
            is_available: false,
            version: String::new(),
            message: stderr_or_default(&output.stderr, "yt-dlp 检测失败"),
        },
        Err(error) => YtdlpStatus {
            is_available: false,
            version: String::new(),
            message: format!("未检测到 yt-dlp: {error}"),
        },
    }
}

#[tauri::command]
pub fn list_youtube_channels(
    store: tauri::State<'_, SettingsStore>,
) -> Result<Vec<YoutubeChannel>, String> {
    store.with_connection(read_youtube_channels)
}

#[tauri::command]
pub fn add_youtube_channel(
    store: tauri::State<'_, SettingsStore>,
    request: AddYoutubeChannelRequest,
) -> Result<YoutubeChannel, String> {
    let normalized_url = normalize_youtube_channel_url(&request.url)?;
    let probe = channel_probe_from_url(&normalized_url);
    insert_youtube_channel(&store, probe)
}

#[tauri::command]
pub fn delete_youtube_channel(
    store: tauri::State<'_, SettingsStore>,
    request: YoutubeChannelRequest,
) -> Result<(), String> {
    store.with_connection(|connection| {
        let transaction = connection
            .unchecked_transaction()
            .map_err(|error| format!("无法删除监控博主: {error}"))?;
        transaction
            .execute(
                "DELETE FROM youtube_videos WHERE channel_id = ?1",
                params![request.channel_id],
            )
            .map_err(|error| format!("无法删除监控视频: {error}"))?;
        transaction
            .execute(
                "DELETE FROM youtube_refresh_runs WHERE channel_id = ?1",
                params![request.channel_id],
            )
            .map_err(|error| format!("无法删除检查记录: {error}"))?;
        let changed = transaction
            .execute(
                "DELETE FROM youtube_channels WHERE id = ?1",
                params![request.channel_id],
            )
            .map_err(|error| format!("无法删除监控博主: {error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("无法提交删除操作: {error}"))?;

        if changed == 0 {
            return Err("未找到该监控博主".to_string());
        }

        Ok(())
    })
}

#[tauri::command]
pub fn list_youtube_videos(
    store: tauri::State<'_, SettingsStore>,
    request: ListYoutubeVideosRequest,
) -> Result<YoutubeVideoPage, String> {
    let limit = request
        .limit
        .unwrap_or(DEFAULT_VIDEO_PAGE_SIZE)
        .clamp(1, MAX_VIDEO_PAGE_SIZE);
    let offset = request.offset.unwrap_or(0);
    let query = request.query.unwrap_or_default();
    let unread_only = request.unread_only.unwrap_or(false);

    store.with_connection(|connection| {
        let channel_exists = connection
            .query_row(
                "SELECT 1 FROM youtube_channels WHERE id = ?1",
                params![request.channel_id],
                |_| Ok(()),
            )
            .optional()
            .map_err(|error| format!("无法读取监控博主: {error}"))?
            .is_some();

        if !channel_exists {
            return Err("未找到该监控博主".to_string());
        }

        read_youtube_videos_page(
            connection,
            &request.channel_id,
            &query,
            unread_only,
            limit,
            offset,
        )
    })
}

#[tauri::command]
pub fn mark_youtube_channel_seen(
    store: tauri::State<'_, SettingsStore>,
    request: YoutubeChannelRequest,
) -> Result<YoutubeChannel, String> {
    let now = Utc::now().to_rfc3339();
    store.with_connection(|connection| {
        connection
            .execute(
                "
                UPDATE youtube_videos
                SET is_unread = 0
                WHERE channel_id = ?1
                ",
                params![request.channel_id],
            )
            .map_err(|error| format!("无法标记视频已读: {error}"))?;
        update_channel_counts(connection, &request.channel_id, &now)?;
        read_youtube_channel_by_id(connection, &request.channel_id)
    })
}

#[tauri::command]
pub fn refresh_youtube_channel(
    app: AppHandle,
    store: tauri::State<'_, SettingsStore>,
    service: tauri::State<'_, YoutubeMonitorService>,
    request: YoutubeChannelRequest,
) -> Result<YoutubeRefreshRun, String> {
    let channel_id = request.channel_id.clone();
    let guard = service.acquire_channel(&channel_id)?;
    let channel = store.with_connection(|connection| {
        read_youtube_channel_by_id(connection, &channel_id)
    })?;
    let proxy = store.load()?.youtube_monitor_proxy;
    let run = create_refresh_run(&store, &channel.id)?;
    emit_refresh(&app, &run);
    std::mem::forget(guard);

    let background_app = app.clone();
    let background_channel_id = channel.id.clone();
    let background_run_id = run.id.clone();
    let returned_run = run.clone();
    thread::spawn(move || {
        let store = background_app.state::<SettingsStore>();
        let result = run_ytdlp_refresh(&background_app, &store, &channel, run.clone(), &proxy);

        if let Err(error) = result {
            if let Ok(failed_run) = fail_refresh_run(&store, &background_channel_id, &background_run_id, &error) {
                emit_refresh(&background_app, &failed_run);
            }
        }

        let service = background_app.state::<YoutubeMonitorService>();
        service.release_channel(&background_channel_id);
    });

    Ok(returned_run)
}

fn run_ytdlp_refresh(
    app: &AppHandle,
    store: &SettingsStore,
    channel: &YoutubeChannel,
    mut run: YoutubeRefreshRun,
    proxy: &str,
) -> Result<YoutubeRefreshRun, String> {
    let latest_known_video_id = store.with_connection(|connection| {
        read_latest_youtube_video_external_id(connection, &channel.id)
    })?;
    let is_incremental_refresh = latest_known_video_id.is_some();
    let mut command = ytdlp_command(proxy);
    command.args([
        "--flat-playlist",
        "-j",
        "--ignore-errors",
        "--no-warnings",
        "--no-progress",
        "--extractor-args",
        YTDLP_YOUTUBE_LANGUAGE_ARGS,
        "--add-headers",
        YOUTUBE_ACCEPT_LANGUAGE,
        "--socket-timeout",
        YTDLP_SOCKET_TIMEOUT_SECONDS,
        &channel.url,
    ]);
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("无法启动 yt-dlp: {error}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取 yt-dlp 输出".to_string())?;
    let stderr = child.stderr.take();
    let stderr_handle = stderr.map(|mut value| {
        thread::spawn(move || {
            let mut text = String::new();
            let _ = value.read_to_string(&mut text);
            text
        })
    });

    let reader = BufReader::new(stdout);
    let mut did_update_channel_metadata = false;
    let mut stopped_on_known_video = false;
    let mut first_seen_video_id: Option<String> = None;
    let mut pending_entries = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(|error| format!("读取 yt-dlp 输出失败: {error}"))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let value = match serde_json::from_str::<Value>(trimmed) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if !did_update_channel_metadata {
            update_channel_metadata_from_value(store, &channel.id, &value)?;
            did_update_channel_metadata = true;
        }
        let Some(entry) = extract_video_entry(value) else {
            continue;
        };
        if first_seen_video_id.is_none() {
            first_seen_video_id = Some(entry.external_id.clone());
        }

        if latest_known_video_id
            .as_deref()
            .is_some_and(|latest_id| latest_id == entry.external_id)
        {
            stopped_on_known_video = true;
            let _ = child.kill();
            break;
        }

        run.processed_count += 1;
        pending_entries.push(entry);

        if run.processed_count == 1 || run.processed_count % 25 == 0 {
            run.message = format!("已读取 {} 条视频", run.processed_count);
            update_refresh_run(store, &run)?;
            emit_refresh(app, &run);
        }
    }

    let output_status = child
        .wait()
        .map_err(|error| format!("等待 yt-dlp 结束失败: {error}"))?;
    let stderr_text = stderr_handle
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();

    if !stopped_on_known_video && !output_status.success() {
        let message = if stderr_text.trim().is_empty() {
            "yt-dlp 检查失败".to_string()
        } else {
            stderr_text.trim().to_string()
        };
        return Err(message);
    }

    if !pending_entries.is_empty() {
        run.message = format!("正在保存 {} 条视频", pending_entries.len());
        update_refresh_run(store, &run)?;
        emit_refresh(app, &run);

        let current_max_rank = store.with_connection(|connection| {
            read_max_youtube_video_published_rank(connection, &channel.id)
        })?;
        let entry_count = pending_entries.len() as i64;
        for (index, entry) in pending_entries.into_iter().enumerate() {
            let published_rank = current_max_rank + entry_count - index as i64;
            let inserted = store.with_connection(|connection| {
                upsert_youtube_video(connection, &channel.id, entry, published_rank)
            })?;
            if inserted {
                run.inserted_count += 1;
            } else {
                run.updated_count += 1;
            }
        }
    }

    let now = Utc::now().to_rfc3339();
    run.status = RUN_STATUS_DONE.to_string();
    run.message = if is_incremental_refresh && stopped_on_known_video && run.inserted_count == 0 {
        "检查完成，没有新视频".to_string()
    } else if is_incremental_refresh && stopped_on_known_video {
        format!("检查完成 · 新增 {} 条视频", run.inserted_count)
    } else if run.processed_count == 0 {
        "检查完成，未读取到视频".to_string()
    } else {
        format!("检查完成 · {} 条视频", run.processed_count)
    };
    run.finished_at = Some(now.clone());
    run.error_message.clear();

    store.with_connection(|connection| {
        update_refresh_run_record(connection, &run)?;
        update_channel_after_refresh(
            connection,
            &channel.id,
            CHANNEL_STATUS_IDLE,
            "",
            Some(&now),
            first_seen_video_id.as_deref(),
        )?;
        Ok(())
    })?;
    emit_refresh(app, &run);

    Ok(run)
}

fn create_refresh_run(store: &SettingsStore, channel_id: &str) -> Result<YoutubeRefreshRun, String> {
    let run = YoutubeRefreshRun {
        id: Uuid::new_v4().to_string(),
        channel_id: channel_id.to_string(),
        status: RUN_STATUS_RUNNING.to_string(),
        processed_count: 0,
        inserted_count: 0,
        updated_count: 0,
        message: "准备检查更新".to_string(),
        error_message: String::new(),
        started_at: Utc::now().to_rfc3339(),
        finished_at: None,
    };

    store.with_connection(|connection| {
        insert_refresh_run(connection, &run)?;
        update_channel_after_refresh(connection, channel_id, CHANNEL_STATUS_CHECKING, "", None, None)
    })?;

    Ok(run)
}

fn fail_refresh_run(
    store: &SettingsStore,
    channel_id: &str,
    run_id: &str,
    error: &str,
) -> Result<YoutubeRefreshRun, String> {
    let now = Utc::now().to_rfc3339();
    store.with_connection(|connection| {
        let mut run = read_refresh_run_by_id(connection, run_id)?;
        run.status = RUN_STATUS_FAILED.to_string();
        run.message = "检查失败".to_string();
        run.error_message = compact_error(error);
        run.finished_at = Some(now);
        update_refresh_run_record(connection, &run)?;
        update_channel_after_refresh(
            connection,
            channel_id,
            CHANNEL_STATUS_FAILED,
            &run.error_message,
            None,
            None,
        )?;
        Ok(run)
    })
}

fn ytdlp_command(proxy: &str) -> Command {
    let mut command = Command::new(YTDLP_COMMAND);
    let proxy = proxy.trim();
    if !proxy.is_empty() {
        command.args(["--proxy", proxy]);
    }

    command
}

fn channel_probe_from_url(url: &str) -> ChannelProbe {
    let handle = handle_from_url(url);
    let title = if handle.is_empty() {
        "待检查博主".to_string()
    } else {
        handle.clone()
    };

    ChannelProbe {
        url: url.to_string(),
        canonical_url: url.to_string(),
        external_id: String::new(),
        title,
        handle,
        description: String::new(),
        thumbnail_url: String::new(),
    }
}

fn update_channel_metadata_from_value(
    store: &SettingsStore,
    channel_id: &str,
    value: &Value,
) -> Result<(), String> {
    let canonical_url = first_non_empty(&[
        string_field(value, "channel_url").unwrap_or_default(),
        string_field(value, "uploader_url").unwrap_or_default(),
    ]);
    let title = first_non_empty(&[
        string_field(value, "channel").unwrap_or_default(),
        string_field(value, "uploader").unwrap_or_default(),
    ]);
    let handle = first_non_empty(&[
        handle_from_url(&canonical_url),
        string_field(value, "channel").unwrap_or_default(),
        string_field(value, "uploader").unwrap_or_default(),
    ]);
    let external_id = first_non_empty(&[
        string_field(value, "channel_id").unwrap_or_default(),
        string_field(value, "uploader_id").unwrap_or_default(),
    ]);
    let thumbnail_url = thumbnail_from_list(value).unwrap_or_default();
    let now = Utc::now().to_rfc3339();

    store.with_connection(|connection| {
        connection
            .execute(
                "
                UPDATE youtube_channels
                SET canonical_url = CASE WHEN ?2 != '' THEN ?2 ELSE canonical_url END,
                    external_id = CASE WHEN ?3 != '' THEN ?3 ELSE external_id END,
                    title = CASE WHEN ?4 != '' THEN ?4 ELSE title END,
                    handle = CASE WHEN ?5 != '' THEN ?5 ELSE handle END,
                    thumbnail_url = CASE WHEN ?6 != '' THEN ?6 ELSE thumbnail_url END,
                    updated_at = ?7
                WHERE id = ?1
                ",
                params![channel_id, canonical_url, external_id, title, handle, thumbnail_url, now],
            )
            .map(|_| ())
            .map_err(|error| format!("无法更新博主信息: {error}"))
    })
}

fn insert_youtube_channel(
    store: &SettingsStore,
    probe: ChannelProbe,
) -> Result<YoutubeChannel, String> {
    let now = Utc::now().to_rfc3339();
    let id = Uuid::new_v4().to_string();
    store.with_connection(|connection| {
        if let Some(existing) = read_channel_duplicate(connection, &probe.url, &probe.external_id)? {
            return Ok(existing);
        }

        connection
            .execute(
                "
                INSERT INTO youtube_channels (
                    id, url, canonical_url, external_id, title, handle, description,
                    thumbnail_url, status, last_error, video_count, unread_count,
                    created_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '', 0, 0, ?10, ?11)
                ",
                params![
                    id,
                    probe.url,
                    probe.canonical_url,
                    probe.external_id,
                    probe.title,
                    probe.handle,
                    probe.description,
                    probe.thumbnail_url,
                    CHANNEL_STATUS_IDLE,
                    now,
                    now,
                ],
            )
            .map_err(|error| {
                if error.to_string().contains("UNIQUE") {
                    "该博主已在监控列表中".to_string()
                } else {
                    format!("无法添加监控博主: {error}")
                }
            })?;

        read_youtube_channel_by_id(connection, &id)
    })
}

fn read_channel_duplicate(
    connection: &rusqlite::Connection,
    url: &str,
    external_id: &str,
) -> Result<Option<YoutubeChannel>, String> {
    if !external_id.is_empty() {
        let found = connection
            .query_row(
                "
                SELECT id, url, canonical_url, external_id, title, handle, description,
                       thumbnail_url, status, last_checked_at, last_success_at, last_error,
                       video_count, unread_count, created_at, updated_at,
                       (
                         SELECT title FROM youtube_videos
                         WHERE channel_id = youtube_channels.id
                         ORDER BY published_rank DESC, datetime(first_seen_at) ASC
                         LIMIT 1
                       ),
                       (
                         SELECT url FROM youtube_videos
                         WHERE channel_id = youtube_channels.id
                         ORDER BY published_rank DESC, datetime(first_seen_at) ASC
                         LIMIT 1
                       ),
                       (
                         SELECT first_seen_at FROM youtube_videos
                         WHERE channel_id = youtube_channels.id
                         ORDER BY published_rank DESC, datetime(first_seen_at) ASC
                         LIMIT 1
                       )
                FROM youtube_channels
                WHERE url = ?1 OR external_id = ?2
                LIMIT 1
                ",
                params![url, external_id],
                map_youtube_channel,
            )
            .optional()
            .map_err(|error| format!("无法检查监控博主: {error}"))?;

        return Ok(found);
    }

    connection
        .query_row(
            "
            SELECT id, url, canonical_url, external_id, title, handle, description,
                   thumbnail_url, status, last_checked_at, last_success_at, last_error,
                   video_count, unread_count, created_at, updated_at,
                   (
                     SELECT title FROM youtube_videos
                     WHERE channel_id = youtube_channels.id
                     ORDER BY published_rank DESC, datetime(first_seen_at) ASC
                     LIMIT 1
                   ),
                   (
                     SELECT url FROM youtube_videos
                     WHERE channel_id = youtube_channels.id
                     ORDER BY published_rank DESC, datetime(first_seen_at) ASC
                     LIMIT 1
                   ),
                   (
                     SELECT first_seen_at FROM youtube_videos
                     WHERE channel_id = youtube_channels.id
                     ORDER BY published_rank DESC, datetime(first_seen_at) ASC
                     LIMIT 1
                   )
            FROM youtube_channels
            WHERE url = ?1
            LIMIT 1
            ",
            params![url],
            map_youtube_channel,
        )
        .optional()
        .map_err(|error| format!("无法检查监控博主: {error}"))
}

fn read_youtube_channels(connection: &rusqlite::Connection) -> Result<Vec<YoutubeChannel>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, url, canonical_url, external_id, title, handle, description,
                   thumbnail_url, status, last_checked_at, last_success_at, last_error,
                   video_count, unread_count, created_at, updated_at,
                   (
                     SELECT title FROM youtube_videos
                     WHERE channel_id = youtube_channels.id
                     ORDER BY published_rank DESC, datetime(first_seen_at) ASC
                     LIMIT 1
                   ),
                   (
                     SELECT url FROM youtube_videos
                     WHERE channel_id = youtube_channels.id
                     ORDER BY published_rank DESC, datetime(first_seen_at) ASC
                     LIMIT 1
                   ),
                   (
                     SELECT first_seen_at FROM youtube_videos
                     WHERE channel_id = youtube_channels.id
                     ORDER BY published_rank DESC, datetime(first_seen_at) ASC
                     LIMIT 1
                   )
            FROM youtube_channels
            ORDER BY
              CASE status WHEN 'checking' THEN 0 WHEN 'failed' THEN 1 ELSE 2 END,
              datetime(updated_at) DESC
            ",
        )
        .map_err(|error| format!("无法读取监控博主: {error}"))?;

    let rows = statement
        .query_map([], map_youtube_channel)
        .map_err(|error| format!("无法读取监控博主: {error}"))?;
    let mut channels = Vec::new();
    for row in rows {
        channels.push(row.map_err(|error| format!("无法解析监控博主: {error}"))?);
    }

    Ok(channels)
}

fn read_youtube_channel_by_id(
    connection: &rusqlite::Connection,
    id: &str,
) -> Result<YoutubeChannel, String> {
    connection
        .query_row(
            "
            SELECT id, url, canonical_url, external_id, title, handle, description,
                   thumbnail_url, status, last_checked_at, last_success_at, last_error,
                   video_count, unread_count, created_at, updated_at,
                   (
                     SELECT title FROM youtube_videos
                     WHERE channel_id = youtube_channels.id
                     ORDER BY published_rank DESC, datetime(first_seen_at) ASC
                     LIMIT 1
                   ),
                   (
                     SELECT url FROM youtube_videos
                     WHERE channel_id = youtube_channels.id
                     ORDER BY published_rank DESC, datetime(first_seen_at) ASC
                     LIMIT 1
                   ),
                   (
                     SELECT first_seen_at FROM youtube_videos
                     WHERE channel_id = youtube_channels.id
                     ORDER BY published_rank DESC, datetime(first_seen_at) ASC
                     LIMIT 1
                   )
            FROM youtube_channels
            WHERE id = ?1
            ",
            params![id],
            map_youtube_channel,
        )
        .optional()
        .map_err(|error| format!("无法读取监控博主: {error}"))?
        .ok_or_else(|| "未找到该监控博主".to_string())
}

fn read_latest_youtube_video_external_id(
    connection: &rusqlite::Connection,
    channel_id: &str,
) -> Result<Option<String>, String> {
    let channel_latest = connection
        .query_row(
            "
            SELECT latest_video_external_id
            FROM youtube_channels
            WHERE id = ?1
            ",
            params![channel_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("无法读取历史最新视频: {error}"))?
        .filter(|value| !value.trim().is_empty());

    if channel_latest.is_some() {
        return Ok(channel_latest);
    }

    connection
        .query_row(
            "
            SELECT external_id
            FROM youtube_videos
            WHERE channel_id = ?1
            ORDER BY
              published_rank DESC,
              datetime(first_seen_at) ASC
            LIMIT 1
            ",
            params![channel_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("无法读取历史最新视频: {error}"))
}

fn read_max_youtube_video_published_rank(
    connection: &rusqlite::Connection,
    channel_id: &str,
) -> Result<i64, String> {
    connection
        .query_row(
            "
            SELECT COALESCE(MAX(published_rank), 0)
            FROM youtube_videos
            WHERE channel_id = ?1
            ",
            params![channel_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("无法读取视频排序位置: {error}"))
}

fn map_youtube_channel(row: &Row<'_>) -> rusqlite::Result<YoutubeChannel> {
    Ok(YoutubeChannel {
        id: row.get(0)?,
        url: row.get(1)?,
        canonical_url: row.get(2)?,
        external_id: row.get(3)?,
        title: row.get(4)?,
        handle: row.get(5)?,
        description: row.get(6)?,
        thumbnail_url: row.get(7)?,
        status: row.get(8)?,
        last_checked_at: row.get(9)?,
        last_success_at: row.get(10)?,
        last_error: row.get(11)?,
        video_count: row.get::<_, i64>(12)?.max(0) as u64,
        unread_count: row.get::<_, i64>(13)?.max(0) as u64,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
        latest_video_title: row.get::<_, Option<String>>(16)?.unwrap_or_default(),
        latest_video_url: row.get::<_, Option<String>>(17)?.unwrap_or_default(),
        latest_video_seen_at: row.get(18)?,
    })
}

fn read_youtube_videos_page(
    connection: &rusqlite::Connection,
    channel_id: &str,
    query: &str,
    unread_only: bool,
    limit: u32,
    offset: u32,
) -> Result<YoutubeVideoPage, String> {
    let query_like = normalized_query_like(query);
    let unread_filter = if unread_only { 1 } else { 0 };
    let total = connection
        .query_row(
            "
            SELECT COUNT(*)
            FROM youtube_videos
            WHERE channel_id = ?1
              AND (?2 = '' OR lower(title) LIKE ?2 OR lower(url) LIKE ?2)
              AND (?3 = 0 OR is_unread = 1)
            ",
            params![channel_id, query_like, unread_filter],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("无法统计监控视频: {error}"))?
        .max(0) as u64;

    let mut statement = connection
        .prepare(
            "
            SELECT id, channel_id, external_id, title, url, duration,
                   is_unread, first_seen_at, last_seen_at, metadata
            FROM youtube_videos
            WHERE channel_id = ?1
              AND (?2 = '' OR lower(title) LIKE ?2 OR lower(url) LIKE ?2)
              AND (?3 = 0 OR is_unread = 1)
            ORDER BY
              published_rank DESC,
              datetime(first_seen_at) ASC
            LIMIT ?4 OFFSET ?5
            ",
        )
        .map_err(|error| format!("无法读取监控视频: {error}"))?;
    let rows = statement
        .query_map(
            params![channel_id, query_like, unread_filter, limit, offset],
            map_youtube_video,
        )
        .map_err(|error| format!("无法读取监控视频: {error}"))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|error| format!("无法解析监控视频: {error}"))?);
    }

    let next_offset = offset as u64 + items.len() as u64;
    Ok(YoutubeVideoPage {
        items,
        total,
        has_more: next_offset < total,
    })
}

fn map_youtube_video(row: &Row<'_>) -> rusqlite::Result<YoutubeVideo> {
    let metadata_text: String = row.get(9)?;
    let metadata = serde_json::from_str(&metadata_text).unwrap_or_else(|_| json!({}));

    Ok(YoutubeVideo {
        id: row.get(0)?,
        channel_id: row.get(1)?,
        external_id: row.get(2)?,
        title: row.get(3)?,
        url: row.get(4)?,
        duration: row.get(5)?,
        is_unread: row.get::<_, i64>(6)? != 0,
        first_seen_at: row.get(7)?,
        last_seen_at: row.get(8)?,
        metadata,
    })
}

fn upsert_youtube_video(
    connection: &rusqlite::Connection,
    channel_id: &str,
    entry: VideoEntry,
    published_rank: i64,
) -> Result<bool, String> {
    let now = Utc::now().to_rfc3339();
    let metadata = serde_json::to_string(&entry.metadata).unwrap_or_else(|_| "{}".to_string());
    let existing_id = connection
        .query_row(
            "
            SELECT id
            FROM youtube_videos
            WHERE channel_id = ?1 AND external_id = ?2
            ",
            params![channel_id, entry.external_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("无法检查监控视频: {error}"))?;

    if let Some(existing_id) = existing_id {
        connection
            .execute(
                "
                UPDATE youtube_videos
                SET title = ?1,
                    url = ?2,
                    duration = ?3,
                    metadata = ?4,
                    published_rank = CASE WHEN ?5 > published_rank THEN ?5 ELSE published_rank END,
                    last_seen_at = ?6
                WHERE id = ?7
                ",
                params![
                    entry.title,
                    entry.url,
                    entry.duration,
                    metadata,
                    published_rank,
                    now,
                    existing_id,
                ],
            )
            .map_err(|error| format!("无法更新监控视频: {error}"))?;
        return Ok(false);
    }

    connection
        .execute(
            "
            INSERT INTO youtube_videos (
                id, channel_id, external_id, title, url, duration,
                published_rank, is_unread, first_seen_at, last_seen_at, metadata
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?9, ?10)
            ",
            params![
                Uuid::new_v4().to_string(),
                channel_id,
                entry.external_id,
                entry.title,
                entry.url,
                entry.duration,
                published_rank,
                now,
                now,
                metadata,
            ],
        )
        .map_err(|error| format!("无法保存监控视频: {error}"))?;
    Ok(true)
}

fn insert_refresh_run(
    connection: &rusqlite::Connection,
    run: &YoutubeRefreshRun,
) -> Result<(), String> {
    connection
        .execute(
            "
            INSERT INTO youtube_refresh_runs (
                id, channel_id, status, processed_count, inserted_count, updated_count,
                message, error_message, started_at, finished_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ",
            params![
                run.id,
                run.channel_id,
                run.status,
                run.processed_count as i64,
                run.inserted_count as i64,
                run.updated_count as i64,
                run.message,
                run.error_message,
                run.started_at,
                run.finished_at,
            ],
        )
        .map(|_| ())
        .map_err(|error| format!("无法创建检查记录: {error}"))
}

fn update_refresh_run(store: &SettingsStore, run: &YoutubeRefreshRun) -> Result<(), String> {
    store.with_connection(|connection| update_refresh_run_record(connection, run))
}

fn update_refresh_run_record(
    connection: &rusqlite::Connection,
    run: &YoutubeRefreshRun,
) -> Result<(), String> {
    connection
        .execute(
            "
            UPDATE youtube_refresh_runs
            SET status = ?1,
                processed_count = ?2,
                inserted_count = ?3,
                updated_count = ?4,
                message = ?5,
                error_message = ?6,
                finished_at = ?7
            WHERE id = ?8
            ",
            params![
                run.status,
                run.processed_count as i64,
                run.inserted_count as i64,
                run.updated_count as i64,
                run.message,
                run.error_message,
                run.finished_at,
                run.id,
            ],
        )
        .map(|_| ())
        .map_err(|error| format!("无法更新检查记录: {error}"))
}

fn read_refresh_run_by_id(
    connection: &rusqlite::Connection,
    run_id: &str,
) -> Result<YoutubeRefreshRun, String> {
    connection
        .query_row(
            "
            SELECT id, channel_id, status, processed_count, inserted_count, updated_count,
                   message, error_message, started_at, finished_at
            FROM youtube_refresh_runs
            WHERE id = ?1
            ",
            params![run_id],
            |row| {
                Ok(YoutubeRefreshRun {
                    id: row.get(0)?,
                    channel_id: row.get(1)?,
                    status: row.get(2)?,
                    processed_count: row.get::<_, i64>(3)?.max(0) as u64,
                    inserted_count: row.get::<_, i64>(4)?.max(0) as u64,
                    updated_count: row.get::<_, i64>(5)?.max(0) as u64,
                    message: row.get(6)?,
                    error_message: row.get(7)?,
                    started_at: row.get(8)?,
                    finished_at: row.get(9)?,
                })
            },
        )
        .optional()
        .map_err(|error| format!("无法读取检查记录: {error}"))?
        .ok_or_else(|| "未找到检查记录".to_string())
}

fn update_channel_after_refresh(
    connection: &rusqlite::Connection,
    channel_id: &str,
    status: &str,
    error: &str,
    success_at: Option<&str>,
    latest_video_external_id: Option<&str>,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    let (video_count, unread_count) = channel_counts(connection, channel_id)?;
    connection
        .execute(
            "
            UPDATE youtube_channels
            SET status = ?1,
                last_checked_at = ?2,
                last_success_at = COALESCE(?3, last_success_at),
                last_error = ?4,
                latest_video_external_id = CASE WHEN ?5 IS NOT NULL AND ?5 != '' THEN ?5 ELSE latest_video_external_id END,
                video_count = ?6,
                unread_count = ?7,
                updated_at = ?8
            WHERE id = ?9
            ",
            params![
                status,
                now,
                success_at,
                error,
                latest_video_external_id,
                video_count as i64,
                unread_count as i64,
                now,
                channel_id,
            ],
        )
        .map(|_| ())
        .map_err(|error| format!("无法更新监控博主状态: {error}"))
}

fn update_channel_counts(
    connection: &rusqlite::Connection,
    channel_id: &str,
    updated_at: &str,
) -> Result<(), String> {
    let (video_count, unread_count) = channel_counts(connection, channel_id)?;
    connection
        .execute(
            "
            UPDATE youtube_channels
            SET video_count = ?1,
                unread_count = ?2,
                updated_at = ?3
            WHERE id = ?4
            ",
            params![video_count as i64, unread_count as i64, updated_at, channel_id],
        )
        .map(|_| ())
        .map_err(|error| format!("无法更新监控博主统计: {error}"))
}

fn channel_counts(
    connection: &rusqlite::Connection,
    channel_id: &str,
) -> Result<(u64, u64), String> {
    let video_count = connection
        .query_row(
            "SELECT COUNT(*) FROM youtube_videos WHERE channel_id = ?1",
            params![channel_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("无法统计监控视频: {error}"))?
        .max(0) as u64;
    let unread_count = connection
        .query_row(
            "SELECT COUNT(*) FROM youtube_videos WHERE channel_id = ?1 AND is_unread = 1",
            params![channel_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("无法统计未读视频: {error}"))?
        .max(0) as u64;

    Ok((video_count, unread_count))
}

fn extract_video_entry(value: Value) -> Option<VideoEntry> {
    let external_id = first_non_empty(&[
        string_field(&value, "id").unwrap_or_default(),
        string_field(&value, "display_id").unwrap_or_default(),
        id_from_url(&value),
    ]);
    if external_id.is_empty() {
        return None;
    }

    let url = first_non_empty(&[
        string_field(&value, "webpage_url").unwrap_or_default(),
        string_field(&value, "original_url").unwrap_or_default(),
        string_field(&value, "url").unwrap_or_default(),
        format!("https://www.youtube.com/watch?v={external_id}"),
    ]);
    let url = if url.starts_with("http://") || url.starts_with("https://") {
        url
    } else {
        format!("https://www.youtube.com/watch?v={external_id}")
    };

    Some(VideoEntry {
        external_id,
        title: string_field(&value, "title").unwrap_or_else(|| "未命名视频".to_string()),
        url,
        duration: number_field(&value, "duration"),
        metadata: value,
    })
}

fn normalize_youtube_channel_url(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("请输入 YouTube 博主地址".to_string());
    }

    let url = if let Some(handle) = trimmed.strip_prefix('@') {
        format!("https://www.youtube.com/@{handle}")
    } else if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let lower = url.to_lowercase();

    if !lower.contains("youtube.com/") {
        return Err("请输入 YouTube 频道主页地址".to_string());
    }

    if lower.contains("/watch?") || lower.contains("/shorts/") || lower.contains("/embed/") {
        return Err("请输入博主主页地址，不是单个视频地址".to_string());
    }

    Ok(url)
}

fn normalized_query_like(query: &str) -> String {
    let normalized = query.trim().to_lowercase();
    if normalized.is_empty() {
        String::new()
    } else {
        format!("%{}%", normalized.replace('%', "\\%").replace('_', "\\_"))
    }
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

fn thumbnail_from_list(value: &Value) -> Option<String> {
    value
        .get("thumbnails")
        .and_then(Value::as_array)
        .and_then(|items| items.iter().rev().find_map(|item| string_field(item, "url")))
}

fn first_non_empty(values: &[String]) -> String {
    values
        .iter()
        .find(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_default()
}

fn handle_from_url(url: &str) -> String {
    let marker = "youtube.com/@";
    let lower = url.to_lowercase();
    let Some(index) = lower.find(marker) else {
        return String::new();
    };
    let start = index + marker.len();
    let handle = url[start..]
        .split(['/', '?', '&', '#'])
        .next()
        .unwrap_or_default()
        .trim();

    if handle.is_empty() {
        String::new()
    } else {
        format!("@{handle}")
    }
}

fn id_from_url(value: &Value) -> String {
    let url = string_field(value, "url").unwrap_or_default();
    if url.len() == 11 && !url.contains('/') {
        return url;
    }

    String::new()
}

fn stderr_or_default(stderr: &[u8], fallback: &str) -> String {
    let message = String::from_utf8_lossy(stderr).trim().to_string();
    if message.is_empty() {
        fallback.to_string()
    } else {
        compact_error(&message)
    }
}

fn compact_error(error: &str) -> String {
    let compact = error
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join("；");

    if compact.is_empty() {
        "操作失败".to_string()
    } else {
        compact
    }
}

fn emit_refresh(app: &AppHandle, run: &YoutubeRefreshRun) {
    let _ = app.emit(REFRESH_EVENT, run);
}

