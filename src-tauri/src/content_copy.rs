use crate::ai::AiService;
use crate::app_log::AppLogger;
use crate::settings::{AppSettings, SettingsStore};
use crate::subtitle_translation::load_subtitle_segments;
use crate::transcription::TranscriptionSegment;
use chrono::Utc;
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const DEFAULT_PLATFORM: &str = "bilibili";
const SOURCE_COPYWRITING: &str = "copywriting";
const SOURCE_WORKBENCH: &str = "workbench";
const MAX_GENERATION_ATTEMPTS: usize = 3;
const MAX_SUMMARY_ATTEMPTS: usize = 2;
const DIRECT_TRANSCRIPT_CHAR_LIMIT: usize = 14_000;
const SUMMARY_CHUNK_CHAR_LIMIT: usize = 8_000;
const MAX_PROMPT_TRANSCRIPT_CHARS: usize = 18_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateContentCopyRequest {
    pub subtitle_path: String,
    #[serde(default)]
    pub extra_context: String,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentCopyOptions {
    pub platform: String,
    pub title_count: u8,
    pub cover_text_count: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentCopyCategory {
    pub primary: String,
    pub secondary: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentCopyTitle {
    pub title: String,
    pub hook: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentCopyCoverText {
    pub lines: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentCopyTimelineItem {
    pub time: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentCopyDescription {
    pub intro: String,
    pub timeline: Vec<ContentCopyTimelineItem>,
    pub call_to_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentCopyTags {
    pub core: Vec<String>,
    pub category: Vec<String>,
    pub long_tail: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentCopyResult {
    pub summary: String,
    pub audience: String,
    pub category: ContentCopyCategory,
    pub titles: Vec<ContentCopyTitle>,
    pub cover_texts: Vec<ContentCopyCoverText>,
    pub description: ContentCopyDescription,
    pub tags: ContentCopyTags,
    pub pinned_comment: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentCopyRecord {
    pub id: String,
    pub source: String,
    pub platform: String,
    pub subtitle_path: String,
    pub subtitle_file_name: String,
    pub subtitle_format: String,
    pub segment_count: u32,
    pub duration_ms: u64,
    pub extra_context: String,
    pub options: ContentCopyOptions,
    pub result: ContentCopyResult,
    pub log_path: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentCopyRecordRequest {
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteContentCopyRecordRequest {
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListContentCopyRecordsRequest {
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContentChunkSummary {
    summary: String,
    key_points: Vec<String>,
    timeline: Vec<ContentCopyTimelineItem>,
    keywords: Vec<String>,
}

#[tauri::command]
pub async fn generate_content_copy(
    settings_store: tauri::State<'_, SettingsStore>,
    ai_service: tauri::State<'_, AiService>,
    app_logger: tauri::State<'_, AppLogger>,
    request: GenerateContentCopyRequest,
) -> Result<ContentCopyRecord, String> {
    generate_content_copy_record(
        settings_store.inner(),
        ai_service.inner(),
        app_logger.inner(),
        request,
    )
    .await
}

pub(crate) async fn generate_content_copy_record(
    settings_store: &SettingsStore,
    ai_service: &AiService,
    app_logger: &AppLogger,
    request: GenerateContentCopyRequest,
) -> Result<ContentCopyRecord, String> {
    let settings = settings_store.load()?;
    let log_session = app_logger.start_session("content_copy")?;
    let subtitle_path = PathBuf::from(request.subtitle_path.trim());
    let options = ContentCopyOptions {
        platform: normalize_platform(request.platform.as_deref()),
        title_count: 6,
        cover_text_count: 4,
    };
    let source = normalize_source(request.source.as_deref());

    log_session.info(
        "request_received",
        "收到文案生成请求",
        json!({
            "subtitlePath": subtitle_path.to_string_lossy(),
            "platform": &options.platform,
            "source": &source,
        }),
    );

    let segments = match load_subtitle_segments(&subtitle_path) {
        Ok(segments) => segments,
        Err(error) => {
            log_session.error(
                "subtitle_load_failed",
                "读取文案生成字幕文件失败",
                json!({ "subtitlePath": subtitle_path.to_string_lossy(), "error": &error }),
            );
            return Err(error);
        }
    };
    if segments.is_empty() {
        let error = "字幕文件没有可用文本".to_string();
        log_session.warn(
            "subtitle_empty",
            "文案生成字幕内容为空",
            json!({ "subtitlePath": subtitle_path.to_string_lossy() }),
        );
        return Err(error);
    }

    let transcript = match build_prompt_transcript(&settings, &ai_service, &segments).await {
        Ok(transcript) => transcript,
        Err(error) => {
            log_session.error(
                "transcript_prepare_failed",
                "文案生成转录文本准备失败",
                json!({ "segmentCount": segments.len(), "error": &error }),
            );
            return Err(error);
        }
    };
    let result = match generate_result_by_llm(
        &settings,
        &ai_service,
        &transcript,
        &request.extra_context,
        &options,
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            log_session.error(
                "content_generation_failed",
                "AI 文案生成失败",
                json!({ "segmentCount": segments.len(), "error": &error }),
            );
            return Err(error);
        }
    };
    let now = Utc::now().to_rfc3339();
    let record = ContentCopyRecord {
        id: Uuid::new_v4().to_string(),
        source,
        platform: options.platform.clone(),
        subtitle_path: subtitle_path.to_string_lossy().to_string(),
        subtitle_file_name: file_name(&subtitle_path),
        subtitle_format: file_extension(&subtitle_path),
        segment_count: segments.len().min(u32::MAX as usize) as u32,
        duration_ms: subtitle_duration_ms(&segments),
        extra_context: request.extra_context,
        options,
        result,
        log_path: log_session.path_string(),
        created_at: now.clone(),
        updated_at: now,
    };

    if let Err(error) =
        settings_store.with_connection(|connection| insert_content_copy_record(connection, &record))
    {
        log_session.error(
            "record_save_failed",
            "保存文案生成结果失败",
            json!({ "recordId": &record.id, "error": &error }),
        );
        return Err(error);
    }
    log_session.info(
        "record_saved",
        "文案生成结果已保存",
        json!({
            "recordId": &record.id,
            "segmentCount": record.segment_count,
            "durationMs": record.duration_ms,
        }),
    );

    Ok(record)
}

#[tauri::command]
pub fn list_content_copy_records(
    settings_store: tauri::State<'_, SettingsStore>,
    request: ListContentCopyRecordsRequest,
) -> Result<Vec<ContentCopyRecord>, String> {
    let limit = request.limit.unwrap_or(30);
    let source = normalize_source(request.source.as_deref());
    settings_store
        .with_connection(|connection| read_content_copy_records(connection, limit, &source))
}

#[tauri::command]
pub fn get_content_copy_record(
    settings_store: tauri::State<'_, SettingsStore>,
    request: ContentCopyRecordRequest,
) -> Result<ContentCopyRecord, String> {
    settings_store.with_connection(|connection| {
        read_content_copy_record(connection, &request.id)?
            .ok_or_else(|| "文案历史不存在".to_string())
    })
}

#[tauri::command]
pub fn delete_content_copy_record(
    settings_store: tauri::State<'_, SettingsStore>,
    app_logger: tauri::State<'_, AppLogger>,
    request: DeleteContentCopyRecordRequest,
) -> Result<(), String> {
    app_logger.info(
        "content_copy",
        "record_delete_start",
        "开始删除文案历史",
        json!({ "recordId": &request.id }),
    );
    let result = settings_store.with_connection(|connection| {
        connection
            .execute(
                "DELETE FROM content_copy_records WHERE id = ?1",
                params![&request.id],
            )
            .map(|affected| affected)
            .map_err(|error| format!("无法删除文案历史: {error}"))
    });

    match result {
        Ok(affected) => {
            app_logger.info(
                "content_copy",
                "record_delete_success",
                "文案历史已删除",
                json!({ "recordId": &request.id, "affectedRows": affected }),
            );
            Ok(())
        }
        Err(error) => {
            app_logger.error(
                "content_copy",
                "record_delete_failed",
                "删除文案历史失败",
                json!({ "recordId": &request.id, "error": &error }),
            );
            Err(error)
        }
    }
}

async fn build_prompt_transcript(
    settings: &AppSettings,
    ai_service: &AiService,
    segments: &[TranscriptionSegment],
) -> Result<String, String> {
    let transcript = format_segments_for_prompt(segments);
    if transcript.chars().count() <= DIRECT_TRANSCRIPT_CHAR_LIMIT {
        return Ok(transcript);
    }

    let chunks = chunk_segments_for_summary(segments, SUMMARY_CHUNK_CHAR_LIMIT);
    let mut summaries = Vec::with_capacity(chunks.len());
    for (index, chunk) in chunks.iter().enumerate() {
        let summary =
            summarize_chunk_by_llm(settings, ai_service, index + 1, chunks.len(), chunk).await?;
        summaries.push(summary);
    }

    Ok(format_chunk_summaries(&summaries))
}

async fn summarize_chunk_by_llm(
    settings: &AppSettings,
    ai_service: &AiService,
    index: usize,
    total: usize,
    chunk: &str,
) -> Result<ContentChunkSummary, String> {
    let mut feedback = String::new();
    for attempt in 1..=MAX_SUMMARY_ATTEMPTS {
        let user_prompt = format!(
            "这是第 {index}/{total} 段字幕。请压缩为 JSON，保留核心观点、人物/产品/事件、可做时间轴的节点、关键词。\n\n{feedback}\n\n字幕：\n{chunk}"
        );
        let response = ai_service
            .chat_for_json_output(
                settings,
                build_summary_system_prompt(),
                user_prompt,
                estimate_output_tokens(chunk, 1800),
            )
            .await?;

        match parse_json_response::<ContentChunkSummary>(&response).and_then(validate_chunk_summary)
        {
            Ok(summary) => return Ok(summary),
            Err(error) => {
                if attempt == MAX_SUMMARY_ATTEMPTS {
                    return Err(format!("字幕摘要结果无效: {error}"));
                }
                feedback = format!("上一次 JSON 无效: {error}。请只输出符合字段要求的 JSON。");
            }
        }
    }

    Err("字幕摘要失败".to_string())
}

async fn generate_result_by_llm(
    settings: &AppSettings,
    ai_service: &AiService,
    transcript: &str,
    extra_context: &str,
    options: &ContentCopyOptions,
) -> Result<ContentCopyResult, String> {
    let mut feedback = String::new();
    for attempt in 1..=MAX_GENERATION_ATTEMPTS {
        let user_prompt =
            build_generation_user_prompt(transcript, extra_context, options, &feedback);
        let response = ai_service
            .chat_for_json_output(
                settings,
                build_generation_system_prompt(options),
                user_prompt,
                estimate_output_tokens(transcript, 4200),
            )
            .await?;

        match parse_json_response::<ContentCopyResult>(&response).and_then(validate_result) {
            Ok(result) => return Ok(result),
            Err(error) => {
                if attempt == MAX_GENERATION_ATTEMPTS {
                    return Err(format!("文案生成结果无效: {error}"));
                }
                feedback =
                    format!("上一次 JSON 无效: {error}。请修正字段、数量和空值，只输出 JSON。");
            }
        }
    }

    Err("文案生成失败".to_string())
}

fn build_summary_system_prompt() -> String {
    [
        "你是视频字幕摘要助手。",
        "只输出 JSON，不输出解释。",
        "JSON 字段：summary 字符串；keyPoints 字符串数组；timeline 数组，每项含 time 和 text；keywords 字符串数组。",
        "摘要要忠于字幕，不编造不存在的信息。",
    ]
    .join("\n")
}

fn build_generation_system_prompt(options: &ContentCopyOptions) -> String {
    r#"你是内容发布文案助手，当前内部平台策略为 __PLATFORM__，但输出文案中不要主动出现平台名称。
你会根据字幕内容生成发布前可直接使用的标题、封面字、简介、标签、分类和互动评论。
内部策略：
- 标题参考疑问式、教程式、测评式、挑战式、盘点式、强情绪式，但避免夸张失真。
- 标签按核心标签、分类标签、长尾标签组合，合计 8-12 个。
- 简介包含一句话概括、时间轴和互动引导。
- 分类给出一级和二级分类，不要在分类字段写平台名。
- 封面字只生成文字，不生成图片。每组 1-2 行，每行尽量不超过 10 个汉字。
- 借鉴短视频封面标题的钩子方法，但语气要适合内容发布场景，不能小红书化、焦虑化。

必须只输出 JSON，字段如下：
{
  "summary": "内容摘要",
  "audience": "目标观众",
  "category": { "primary": "一级分类", "secondary": "二级分类", "reason": "推荐理由" },
  "titles": [
    { "title": "标题候选", "hook": "标题方法", "reason": "推荐理由" }
  ],
  "coverTexts": [
    { "lines": ["封面字第一行", "封面字第二行"], "reason": "推荐理由" }
  ],
  "description": {
    "intro": "简介正文",
    "timeline": [{ "time": "00:00", "text": "节点说明" }],
    "callToAction": "互动引导"
  },
  "tags": { "core": ["标签"], "category": ["标签"], "longTail": ["标签"] },
  "pinnedComment": "互动评论"
}
"#
    .replace("__PLATFORM__", &options.platform)
}

fn build_generation_user_prompt(
    transcript: &str,
    extra_context: &str,
    options: &ContentCopyOptions,
    feedback: &str,
) -> String {
    format!(
        r#"请基于字幕生成内容文案。
数量要求：
- titles 输出 {title_count} 条。
- coverTexts 输出 {cover_count} 组。
- tags 合计 8-12 个。

补充信息：
{extra_context}

校验反馈：
{feedback}

字幕内容：
{transcript}
"#,
        title_count = options.title_count,
        cover_count = options.cover_text_count,
        extra_context = if extra_context.trim().is_empty() {
            "无"
        } else {
            extra_context.trim()
        },
        feedback = if feedback.trim().is_empty() {
            "无"
        } else {
            feedback.trim()
        },
        transcript = truncate_chars(transcript, MAX_PROMPT_TRANSCRIPT_CHARS),
    )
}

fn validate_chunk_summary(summary: ContentChunkSummary) -> Result<ContentChunkSummary, String> {
    if summary.summary.trim().is_empty() {
        return Err("summary 不能为空".to_string());
    }
    Ok(ContentChunkSummary {
        summary: summary.summary.trim().to_string(),
        key_points: trim_string_vec(summary.key_points),
        timeline: summary
            .timeline
            .into_iter()
            .filter(|item| !item.time.trim().is_empty() && !item.text.trim().is_empty())
            .map(|item| ContentCopyTimelineItem {
                time: item.time.trim().to_string(),
                text: item.text.trim().to_string(),
            })
            .collect(),
        keywords: trim_string_vec(summary.keywords),
    })
}

fn validate_result(result: ContentCopyResult) -> Result<ContentCopyResult, String> {
    if result.summary.trim().is_empty() {
        return Err("summary 不能为空".to_string());
    }
    if result.audience.trim().is_empty() {
        return Err("audience 不能为空".to_string());
    }
    if result.category.primary.trim().is_empty() || result.category.secondary.trim().is_empty() {
        return Err("category.primary 和 category.secondary 不能为空".to_string());
    }
    if result.titles.len() < 3 {
        return Err("titles 至少需要 3 条".to_string());
    }
    if result.cover_texts.len() < 2 {
        return Err("coverTexts 至少需要 2 组".to_string());
    }
    let tag_count =
        result.tags.core.len() + result.tags.category.len() + result.tags.long_tail.len();
    if tag_count < 6 {
        return Err("tags 数量不足".to_string());
    }
    if result.description.intro.trim().is_empty() {
        return Err("description.intro 不能为空".to_string());
    }
    if result.pinned_comment.trim().is_empty() {
        return Err("pinnedComment 不能为空".to_string());
    }

    Ok(ContentCopyResult {
        summary: result.summary.trim().to_string(),
        audience: result.audience.trim().to_string(),
        category: ContentCopyCategory {
            primary: result.category.primary.trim().to_string(),
            secondary: result.category.secondary.trim().to_string(),
            reason: result.category.reason.trim().to_string(),
        },
        titles: result
            .titles
            .into_iter()
            .filter(|title| !title.title.trim().is_empty())
            .map(|title| ContentCopyTitle {
                title: title.title.trim().to_string(),
                hook: title.hook.trim().to_string(),
                reason: title.reason.trim().to_string(),
            })
            .collect(),
        cover_texts: result
            .cover_texts
            .into_iter()
            .filter(|cover| cover.lines.iter().any(|line| !line.trim().is_empty()))
            .map(|cover| ContentCopyCoverText {
                lines: trim_string_vec(cover.lines).into_iter().take(2).collect(),
                reason: cover.reason.trim().to_string(),
            })
            .collect(),
        description: ContentCopyDescription {
            intro: result.description.intro.trim().to_string(),
            timeline: result
                .description
                .timeline
                .into_iter()
                .filter(|item| !item.time.trim().is_empty() && !item.text.trim().is_empty())
                .map(|item| ContentCopyTimelineItem {
                    time: item.time.trim().to_string(),
                    text: item.text.trim().to_string(),
                })
                .collect(),
            call_to_action: result.description.call_to_action.trim().to_string(),
        },
        tags: ContentCopyTags {
            core: trim_string_vec(result.tags.core),
            category: trim_string_vec(result.tags.category),
            long_tail: trim_string_vec(result.tags.long_tail),
        },
        pinned_comment: result.pinned_comment.trim().to_string(),
    })
}

fn parse_json_response<T>(text: &str) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    let candidates = extract_json_object_candidates(text);
    if candidates.is_empty() {
        return Err("未找到 JSON 对象".to_string());
    }

    let mut last_error = String::new();
    for candidate in candidates.iter().rev() {
        match serde_json::from_str::<T>(candidate) {
            Ok(value) => return Ok(value),
            Err(error) => last_error = format!("JSON 解析失败: {error}"),
        }
    }

    Err(last_error)
}

fn extract_json_object_candidates(text: &str) -> Vec<&str> {
    let mut candidates = Vec::new();
    let bytes = text.as_bytes();
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    let mut start = None;

    for (index, byte) in bytes.iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }

        match *byte {
            b'"' => in_string = true,
            b'{' => {
                if stack.is_empty() {
                    start = Some(index);
                }
                stack.push(*byte);
            }
            b'}' => {
                if stack.pop().is_some() && stack.is_empty() {
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

fn format_segments_for_prompt(segments: &[TranscriptionSegment]) -> String {
    segments
        .iter()
        .map(|segment| {
            format!(
                "{} {}",
                format_time(segment.start_time),
                normalize_prompt_text(&segment.text)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn chunk_segments_for_summary(segments: &[TranscriptionSegment], max_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();

    for segment in segments {
        let line = format!(
            "{} {}\n",
            format_time(segment.start_time),
            normalize_prompt_text(&segment.text)
        );
        if !current.is_empty() && current.chars().count() + line.chars().count() > max_chars {
            chunks.push(current.trim().to_string());
            current.clear();
        }
        current.push_str(&line);
    }

    if !current.trim().is_empty() {
        chunks.push(current.trim().to_string());
    }

    chunks
}

fn format_chunk_summaries(summaries: &[ContentChunkSummary]) -> String {
    summaries
        .iter()
        .enumerate()
        .map(|(index, summary)| {
            let key_points = summary.key_points.join("；");
            let timeline = summary
                .timeline
                .iter()
                .map(|item| format!("{} {}", item.time, item.text))
                .collect::<Vec<_>>()
                .join("；");
            let keywords = summary.keywords.join("、");
            format!(
                "片段{}：{}\n要点：{}\n时间轴：{}\n关键词：{}",
                index + 1,
                summary.summary,
                key_points,
                timeline,
                keywords
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn insert_content_copy_record(
    connection: &rusqlite::Connection,
    record: &ContentCopyRecord,
) -> Result<(), String> {
    let options_text = serde_json::to_string(&record.options)
        .map_err(|error| format!("无法保存文案参数: {error}"))?;
    let result_text = serde_json::to_string(&record.result)
        .map_err(|error| format!("无法保存文案结果: {error}"))?;

    connection
        .execute(
            "
            INSERT INTO content_copy_records (
                id, source, platform, subtitle_path, subtitle_file_name, subtitle_format,
                segment_count, duration_ms, extra_context, options, result,
                log_path, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            ",
            params![
                &record.id,
                &record.source,
                &record.platform,
                &record.subtitle_path,
                &record.subtitle_file_name,
                &record.subtitle_format,
                record.segment_count as i64,
                record.duration_ms.min(i64::MAX as u64) as i64,
                &record.extra_context,
                options_text,
                result_text,
                &record.log_path,
                &record.created_at,
                &record.updated_at,
            ],
        )
        .map(|_| ())
        .map_err(|error| format!("无法保存文案历史: {error}"))
}

fn read_content_copy_records(
    connection: &rusqlite::Connection,
    limit: u32,
    source: &str,
) -> Result<Vec<ContentCopyRecord>, String> {
    let normalized_limit = limit.clamp(1, 100);
    let mut statement = connection
        .prepare(
            "
            SELECT id, source, platform, subtitle_path, subtitle_file_name, subtitle_format,
                   segment_count, duration_ms, extra_context, options, result,
                   log_path, created_at, updated_at
            FROM content_copy_records
            WHERE source = ?1
            ORDER BY datetime(updated_at) DESC
            LIMIT ?2
            ",
        )
        .map_err(|error| format!("无法读取文案历史: {error}"))?;
    let rows = statement
        .query_map(
            params![source, normalized_limit as i64],
            map_content_copy_record,
        )
        .map_err(|error| format!("无法读取文案历史: {error}"))?;
    let mut records = Vec::new();
    for row in rows {
        records.push(row.map_err(|error| format!("无法解析文案历史: {error}"))?);
    }
    Ok(records)
}

fn read_content_copy_record(
    connection: &rusqlite::Connection,
    id: &str,
) -> Result<Option<ContentCopyRecord>, String> {
    connection
        .query_row(
            "
            SELECT id, source, platform, subtitle_path, subtitle_file_name, subtitle_format,
                   segment_count, duration_ms, extra_context, options, result,
                   log_path, created_at, updated_at
            FROM content_copy_records
            WHERE id = ?1
            LIMIT 1
            ",
            params![id],
            map_content_copy_record,
        )
        .optional()
        .map_err(|error| format!("无法读取文案历史: {error}"))
}

fn map_content_copy_record(row: &Row<'_>) -> rusqlite::Result<ContentCopyRecord> {
    let segment_count: i64 = row.get(6)?;
    let duration_ms: i64 = row.get(7)?;
    let options_text: String = row.get(9)?;
    let result_text: String = row.get(10)?;

    Ok(ContentCopyRecord {
        id: row.get(0)?,
        source: row.get(1)?,
        platform: row.get(2)?,
        subtitle_path: row.get(3)?,
        subtitle_file_name: row.get(4)?,
        subtitle_format: row.get(5)?,
        segment_count: segment_count.max(0).min(u32::MAX as i64) as u32,
        duration_ms: duration_ms.max(0) as u64,
        extra_context: row.get(8)?,
        options: serde_json::from_str(&options_text).unwrap_or_else(|_| ContentCopyOptions {
            platform: DEFAULT_PLATFORM.to_string(),
            title_count: 6,
            cover_text_count: 4,
        }),
        result: serde_json::from_str(&result_text).unwrap_or_else(|_| empty_result()),
        log_path: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

fn empty_result() -> ContentCopyResult {
    ContentCopyResult {
        summary: String::new(),
        audience: String::new(),
        category: ContentCopyCategory {
            primary: String::new(),
            secondary: String::new(),
            reason: String::new(),
        },
        titles: Vec::new(),
        cover_texts: Vec::new(),
        description: ContentCopyDescription {
            intro: String::new(),
            timeline: Vec::new(),
            call_to_action: String::new(),
        },
        tags: ContentCopyTags {
            core: Vec::new(),
            category: Vec::new(),
            long_tail: Vec::new(),
        },
        pinned_comment: String::new(),
    }
}

fn normalize_platform(platform: Option<&str>) -> String {
    match platform.map(str::trim).filter(|value| !value.is_empty()) {
        Some("bilibili") => "bilibili".to_string(),
        _ => DEFAULT_PLATFORM.to_string(),
    }
}

fn normalize_source(source: Option<&str>) -> String {
    match source.map(str::trim).filter(|value| !value.is_empty()) {
        Some(SOURCE_WORKBENCH) => SOURCE_WORKBENCH.to_string(),
        _ => SOURCE_COPYWRITING.to_string(),
    }
}

fn subtitle_duration_ms(segments: &[TranscriptionSegment]) -> u64 {
    segments
        .iter()
        .map(|segment| segment.end_time)
        .max()
        .unwrap_or_default()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string()
}

fn file_extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn format_time(milliseconds: u64) -> String {
    let total_seconds = milliseconds / 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

fn normalize_prompt_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn trim_string_vec(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}\n...")
    } else {
        truncated
    }
}

fn estimate_output_tokens(input: &str, minimum: u32) -> u32 {
    let estimated = (input.chars().count() as u32 / 2).saturating_add(1600);
    estimated.clamp(minimum, 8000)
}
