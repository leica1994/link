use reqwest::blocking::Client;
use reqwest::header::{CONTENT_TYPE, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tempfile::{NamedTempFile, TempPath};

const API_REQ_UPLOAD: &str = "https://member.bilibili.com/x/bcut/rubick-interface/resource/create";
const API_COMMIT_UPLOAD: &str =
    "https://member.bilibili.com/x/bcut/rubick-interface/resource/create/complete";
const API_CREATE_TASK: &str = "https://member.bilibili.com/x/bcut/rubick-interface/task";
const API_QUERY_RESULT: &str =
    "https://member.bilibili.com/x/bcut/rubick-interface/task/result";
const BCUT_MODEL_ID_FOR_UPLOAD: &str = "8";
const BCUT_MODEL_ID_FOR_RESULT: &str = "7";
const CHUNK_THRESHOLD_MS: u64 = 2 * 60 * 60 * 1000;
const SMART_SPLIT_WINDOW_MS: u64 = 60 * 1000;
const SILENCE_OFFSET_DB: f64 = 14.0;
const SILENCE_THRESH_MIN_DB: f64 = -55.0;
const SILENCE_THRESH_MAX_DB: f64 = -25.0;
const MAX_UTTERANCE_DURATION_MS: u64 = 60 * 1000;
const BLOB_SUB_CHUNK_DURATION_MS: u64 = 5 * 60 * 1000;
const PROGRESS_EVENT: &str = "transcription-progress";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionRequest {
    pub file_path: String,
    pub model: String,
    pub source_language: String,
    pub output_format: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionProgress {
    pub progress: u8,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionSegment {
    pub text: String,
    pub start_time: u64,
    pub end_time: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionResult {
    pub segments: Vec<TranscriptionSegment>,
    pub subtitle_text: String,
    pub output_path: String,
    pub output_format: String,
}

#[derive(Debug, Deserialize)]
struct UploadCreateResponse {
    data: UploadCreateData,
}

#[derive(Debug, Deserialize)]
struct UploadCreateData {
    in_boss_key: String,
    resource_id: String,
    upload_id: String,
    upload_urls: Vec<String>,
    per_size: usize,
}

#[derive(Debug, Deserialize)]
struct UploadCompleteResponse {
    data: UploadCompleteData,
}

#[derive(Debug, Deserialize)]
struct UploadCompleteData {
    download_url: String,
}

#[derive(Debug, Deserialize)]
struct CreateTaskResponse {
    data: CreateTaskData,
}

#[derive(Debug, Deserialize)]
struct CreateTaskData {
    task_id: String,
}

#[derive(Debug, Deserialize)]
struct QueryTaskResponse {
    data: QueryTaskData,
}

#[derive(Debug, Deserialize)]
struct QueryTaskData {
    state: i64,
    result: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BcutResult {
    #[serde(default)]
    utterances: Vec<BcutUtterance>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct BcutUtterance {
    #[serde(default)]
    transcript: String,
    start_time: u64,
    end_time: u64,
    #[serde(default)]
    words: Vec<BcutWord>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct BcutWord {
    #[serde(default)]
    label: String,
    start_time: u64,
    end_time: u64,
}

#[derive(Debug, Clone, Copy)]
struct AudioChunk {
    start_ms: u64,
    end_ms: u64,
}

struct TranscriptionOptions {
    source_language: String,
}

trait TranscriptionStrategy {
    fn transcribe(
        &self,
        audio_path: &Path,
        options: &TranscriptionOptions,
        progress: &mut dyn FnMut(u8, &str),
    ) -> Result<Vec<TranscriptionSegment>, String>;
}

struct BilibiliTranscriptionStrategy {
    client: Client,
}

impl BilibiliTranscriptionStrategy {
    fn new() -> Result<Self, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|error| format!("无法初始化 B站转录客户端: {error}"))?;

        Ok(Self { client })
    }

    fn run_chunks(
        &self,
        audio_path: &Path,
        duration_ms: u64,
        progress: &mut dyn FnMut(u8, &str),
    ) -> Result<Vec<BcutUtterance>, String> {
        if duration_ms <= CHUNK_THRESHOLD_MS {
            let audio_bytes =
                fs::read(audio_path).map_err(|error| format!("无法读取音频文件: {error}"))?;
            let mut utterances = self.run_single_chunk(&audio_bytes, 0, 100, progress)?;
            utterances = self.fix_blob_utterances(audio_path, utterances, progress)?;
            return Ok(utterances);
        }

        let chunk_count = duration_ms.div_ceil(CHUNK_THRESHOLD_MS);
        progress(0, &format!("音频较长，将分成 {chunk_count} 段处理"));

        let chunks = build_audio_chunks(audio_path, duration_ms, chunk_count);
        let mut all_utterances = Vec::new();

        for (index, chunk) in chunks.iter().enumerate() {
            progress(
                (index as u64 * 100 / chunk_count) as u8,
                &format!("正在分割第 {}/{} 段音频", index + 1, chunk_count),
            );

            let chunk_file_path = export_audio_clip(audio_path, chunk.start_ms, chunk.end_ms)?;
            let chunk_path: &Path = chunk_file_path.as_ref();
            let chunk_bytes = fs::read(chunk_path)
                .map_err(|error| format!("无法读取分段音频文件: {error}"))?;

            let progress_offset = index as u8 * (100 / chunk_count as u8).max(1);
            let progress_span = if index as u64 == chunk_count - 1 {
                100_u8.saturating_sub(progress_offset)
            } else {
                (100 / chunk_count as u8).max(1)
            };

            let mut chunk_utterances =
                self.run_single_chunk(&chunk_bytes, progress_offset, progress_span, progress)?;
            chunk_utterances =
                self.fix_blob_utterances(chunk_path, chunk_utterances, progress)?;

            for utterance in &mut chunk_utterances {
                offset_utterance(utterance, chunk.start_ms);
            }

            all_utterances.extend(chunk_utterances);
        }

        progress(100, "全部转录完成");
        Ok(all_utterances)
    }

    fn run_single_chunk(
        &self,
        audio_bytes: &[u8],
        progress_offset: u8,
        progress_span: u8,
        progress: &mut dyn FnMut(u8, &str),
    ) -> Result<Vec<BcutUtterance>, String> {
        self.report_scaled_progress(progress, progress_offset, progress_span, 0, "上传中");
        let download_url = self.upload(audio_bytes)?;

        self.report_scaled_progress(
            progress,
            progress_offset,
            progress_span,
            40,
            "创建任务中",
        );
        let task_id = self.create_task(&download_url)?;

        self.report_scaled_progress(progress, progress_offset, progress_span, 60, "正在转录");
        let result = self.poll_result(&task_id)?;

        self.report_scaled_progress(progress, progress_offset, progress_span, 100, "转录成功");
        Ok(result.utterances)
    }

    fn report_scaled_progress(
        &self,
        progress: &mut dyn FnMut(u8, &str),
        offset: u8,
        span: u8,
        value: u8,
        message: &str,
    ) {
        let scaled = offset.saturating_add(((value as u16 * span as u16) / 100) as u8);
        progress(scaled.min(100), message);
    }

    fn upload(&self, audio_bytes: &[u8]) -> Result<String, String> {
        let create_payload = json!({
            "type": 2,
            "name": "audio.mp3",
            "size": audio_bytes.len(),
            "ResourceFileType": "mp3",
            "model_id": BCUT_MODEL_ID_FOR_UPLOAD,
        });

        let create_response = self
            .client
            .post(API_REQ_UPLOAD)
            .header(USER_AGENT, "Bilibili/1.0.0 (https://www.bilibili.com)")
            .header(CONTENT_TYPE, "application/json")
            .body(create_payload.to_string())
            .send()
            .and_then(|response| response.error_for_status())
            .map_err(|error| format!("B站转录申请上传失败: {error}"))?
            .json::<UploadCreateResponse>()
            .map_err(|error| format!("B站转录申请上传响应解析失败: {error}"))?;

        let upload_data = create_response.data;
        let mut etags = Vec::with_capacity(upload_data.upload_urls.len());

        for (clip_index, upload_url) in upload_data.upload_urls.iter().enumerate() {
            let start = clip_index * upload_data.per_size;
            let end = ((clip_index + 1) * upload_data.per_size).min(audio_bytes.len());
            let etag = self
                .client
                .put(upload_url)
                .header(USER_AGENT, "Bilibili/1.0.0 (https://www.bilibili.com)")
                .body(audio_bytes[start..end].to_vec())
                .send()
                .and_then(|response| response.error_for_status())
                .map_err(|error| format!("B站转录上传分片失败: {error}"))?
                .headers()
                .get("Etag")
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned)
                .unwrap_or_default();

            if !etag.is_empty() {
                etags.push(etag);
            }
        }

        let complete_payload = json!({
            "InBossKey": upload_data.in_boss_key,
            "ResourceId": upload_data.resource_id,
            "Etags": etags.join(","),
            "UploadId": upload_data.upload_id,
            "model_id": BCUT_MODEL_ID_FOR_UPLOAD,
        });

        let complete_response = self
            .client
            .post(API_COMMIT_UPLOAD)
            .header(USER_AGENT, "Bilibili/1.0.0 (https://www.bilibili.com)")
            .header(CONTENT_TYPE, "application/json")
            .body(complete_payload.to_string())
            .send()
            .and_then(|response| response.error_for_status())
            .map_err(|error| format!("B站转录提交上传失败: {error}"))?
            .json::<UploadCompleteResponse>()
            .map_err(|error| format!("B站转录提交上传响应解析失败: {error}"))?;

        Ok(complete_response.data.download_url)
    }

    fn create_task(&self, download_url: &str) -> Result<String, String> {
        let response = self
            .client
            .post(API_CREATE_TASK)
            .header(USER_AGENT, "Bilibili/1.0.0 (https://www.bilibili.com)")
            .json(&json!({
                "resource": download_url,
                "model_id": BCUT_MODEL_ID_FOR_UPLOAD,
            }))
            .send()
            .and_then(|response| response.error_for_status())
            .map_err(|error| format!("B站转录创建任务失败: {error}"))?
            .json::<CreateTaskResponse>()
            .map_err(|error| format!("B站转录创建任务响应解析失败: {error}"))?;

        Ok(response.data.task_id)
    }

    fn poll_result(&self, task_id: &str) -> Result<BcutResult, String> {
        for _ in 0..1000 {
            let response = self
                .client
                .get(API_QUERY_RESULT)
                .header(USER_AGENT, "Bilibili/1.0.0 (https://www.bilibili.com)")
                .query(&[
                    ("model_id", BCUT_MODEL_ID_FOR_RESULT),
                    ("task_id", task_id),
                ])
                .send()
                .and_then(|response| response.error_for_status())
                .map_err(|error| format!("B站转录查询任务失败: {error}"))?
                .json::<QueryTaskResponse>()
                .map_err(|error| format!("B站转录查询任务响应解析失败: {error}"))?;

            if response.data.state == 4 {
                let result_text = response
                    .data
                    .result
                    .ok_or_else(|| "B站转录结果为空".to_string())?;
                return serde_json::from_str::<BcutResult>(&result_text)
                    .map_err(|error| format!("B站转录结果解析失败: {error}"));
            }

            thread::sleep(Duration::from_secs(1));
        }

        Err("B站转录任务超时，请稍后重试".to_string())
    }

    fn fix_blob_utterances(
        &self,
        audio_path: &Path,
        utterances: Vec<BcutUtterance>,
        progress: &mut dyn FnMut(u8, &str),
    ) -> Result<Vec<BcutUtterance>, String> {
        let mut fixed_utterances = Vec::new();

        for utterance in utterances {
            let duration = utterance.end_time.saturating_sub(utterance.start_time);
            if duration <= MAX_UTTERANCE_DURATION_MS {
                fixed_utterances.push(utterance);
                continue;
            }

            progress(0, "检测到异常长段落，正在分段重新转录");
            let blob_start = utterance.start_time;
            let blob_end = utterance.end_time;
            let blob_duration = blob_end.saturating_sub(blob_start);
            let sub_chunk_count = blob_duration.div_ceil(BLOB_SUB_CHUNK_DURATION_MS).max(2);
            let blob_file_path = export_audio_clip(audio_path, blob_start, blob_end)?;
            let blob_path: &Path = blob_file_path.as_ref();
            let sub_chunks = build_audio_chunks(blob_path, blob_duration, sub_chunk_count);
            let mut sub_utterances = Vec::new();

            for (index, sub_chunk) in sub_chunks.iter().enumerate() {
                progress(
                    0,
                    &format!(
                        "正在重新转录异常段落 {}/{}",
                        index + 1,
                        sub_chunk_count
                    ),
                );

                let sub_file_path = export_audio_clip(
                    blob_path,
                    sub_chunk.start_ms,
                    sub_chunk.end_ms,
                )?;
                let sub_path: &Path = sub_file_path.as_ref();
                let sub_bytes = fs::read(sub_path)
                    .map_err(|error| format!("无法读取异常段落音频文件: {error}"))?;

                match self.run_single_chunk(&sub_bytes, 0, 100, progress) {
                    Ok(mut result) => {
                        for sub_utterance in &mut result {
                            offset_utterance(sub_utterance, blob_start + sub_chunk.start_ms);
                        }
                        sub_utterances.extend(result);
                    }
                    Err(error) => {
                        eprintln!("异常段落重新转录失败: {error}");
                    }
                }
            }

            if sub_utterances.is_empty() {
                fixed_utterances.push(utterance);
            } else {
                fixed_utterances.extend(sub_utterances);
            }
        }

        Ok(fixed_utterances)
    }
}

impl TranscriptionStrategy for BilibiliTranscriptionStrategy {
    fn transcribe(
        &self,
        audio_path: &Path,
        options: &TranscriptionOptions,
        progress: &mut dyn FnMut(u8, &str),
    ) -> Result<Vec<TranscriptionSegment>, String> {
        if options.source_language == "auto" {
            progress(0, "源语言自动识别");
        } else {
            progress(0, "使用指定源语言配置");
        }

        let duration_ms = probe_duration_ms(audio_path)?;
        progress(0, &format!("音频时长 {:.1} 分钟", duration_ms as f64 / 60000.0));

        let mut utterances = self.run_chunks(audio_path, duration_ms, progress)?;
        utterances.sort_by_key(|utterance| utterance.start_time);

        let mut segments: Vec<TranscriptionSegment> = utterances
            .into_iter()
            .filter_map(|utterance| {
                let text = utterance.transcript.trim().to_string();
                if text.is_empty() {
                    None
                } else {
                    Some(TranscriptionSegment {
                        text,
                        start_time: utterance.start_time,
                        end_time: utterance.end_time,
                    })
                }
            })
            .collect();

        optimize_timing(&mut segments, 1000);
        Ok(segments)
    }
}

#[tauri::command]
pub async fn start_transcription(
    app: AppHandle,
    request: TranscriptionRequest,
) -> Result<TranscriptionResult, String> {
    tauri::async_runtime::spawn_blocking(move || run_transcription(app, request))
        .await
        .map_err(|error| format!("转录任务执行失败: {error}"))?
}

#[tauri::command]
pub fn save_transcription_file(path: String, content: String) -> Result<(), String> {
    let output_path = PathBuf::from(path);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("无法创建字幕目录: {error}"))?;
    }

    fs::write(&output_path, content).map_err(|error| format!("无法保存字幕文件: {error}"))
}

fn run_transcription(
    app: AppHandle,
    request: TranscriptionRequest,
) -> Result<TranscriptionResult, String> {
    let input_path = PathBuf::from(&request.file_path);
    if !input_path.is_file() {
        return Err("视频文件不存在".to_string());
    }

    let output_format = normalize_subtitle_format(&request.output_format);
    let output_path = default_output_path(&input_path, output_format)?;
    let audio_file = NamedTempFile::new()
        .map_err(|error| format!("无法创建临时音频文件: {error}"))?
        .into_temp_path()
        .with_extension("wav");

    emit_progress(&app, 5, "转换音频中");
    convert_media_to_audio(&input_path, &audio_file)?;

    emit_progress(&app, 20, "语音转录中");

    let strategy = create_strategy(&request.model)?;
    let options = TranscriptionOptions {
        source_language: request.source_language.clone(),
    };
    let mut progress_callback = |progress: u8, message: &str| {
        let scaled = 20_u8.saturating_add(((progress as u16 * 80) / 100) as u8);
        emit_progress(&app, scaled.min(100), message);
    };

    let segments = strategy.transcribe(&audio_file, &options, &mut progress_callback)?;

    if segments.is_empty() {
        return Err("转录结果为空，请检查音频文件".to_string());
    }

    let subtitle_text = serialize_subtitle(&segments, output_format);
    fs::write(&output_path, &subtitle_text).map_err(|error| format!("无法保存字幕文件: {error}"))?;

    emit_progress(&app, 100, "转录完成");

    let _ = fs::remove_file(&audio_file);

    Ok(TranscriptionResult {
        segments,
        subtitle_text,
        output_path: output_path.to_string_lossy().to_string(),
        output_format: output_format.to_string(),
    })
}

fn create_strategy(model: &str) -> Result<Box<dyn TranscriptionStrategy + Send>, String> {
    match model {
        "bilibili" => Ok(Box::new(BilibiliTranscriptionStrategy::new()?)),
        _ => Err(format!("暂不支持该转录模型: {model}")),
    }
}

fn emit_progress(app: &AppHandle, progress: u8, message: &str) {
    let _ = app.emit(
        PROGRESS_EVENT,
        TranscriptionProgress {
            progress,
            message: message.to_string(),
        },
    );
}

fn convert_media_to_audio(input_path: &Path, output_path: &Path) -> Result<(), String> {
    run_ffmpeg_command(
        Command::new("ffmpeg")
            .arg("-i")
            .arg(input_path)
            .arg("-map")
            .arg("0:a:0")
            .arg("-acodec")
            .arg("pcm_s16le")
            .arg("-ar")
            .arg("16000")
            .arg("-ac")
            .arg("1")
            .arg("-y")
            .arg(output_path),
        "音频转换失败",
    )
}

fn export_audio_clip(input_path: &Path, start_ms: u64, end_ms: u64) -> Result<TempPath, String> {
    let output_file = tempfile::Builder::new()
        .suffix(".mp3")
        .tempfile()
        .map_err(|error| format!("无法创建分段临时文件: {error}"))?
        .into_temp_path();
    let output_path: &Path = output_file.as_ref();
    let start_seconds = format!("{:.3}", start_ms as f64 / 1000.0);
    let duration_seconds = format!("{:.3}", end_ms.saturating_sub(start_ms) as f64 / 1000.0);

    run_ffmpeg_command(
        Command::new("ffmpeg")
            .arg("-ss")
            .arg(start_seconds)
            .arg("-t")
            .arg(duration_seconds)
            .arg("-i")
            .arg(input_path)
            .arg("-acodec")
            .arg("libmp3lame")
            .arg("-ar")
            .arg("16000")
            .arg("-ac")
            .arg("1")
            .arg("-y")
            .arg(output_path),
        "分段音频导出失败",
    )?;

    Ok(output_file)
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

fn probe_duration_ms(path: &Path) -> Result<u64, String> {
    let mut command = Command::new("ffprobe");
    command
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }

    let output = command
        .output()
        .map_err(|error| format!("无法启动 ffprobe 获取音频时长: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("无法获取音频时长: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let seconds = stdout
        .trim()
        .parse::<f64>()
        .map_err(|error| format!("无法解析音频时长: {error}"))?;

    Ok((seconds * 1000.0).round() as u64)
}

fn build_audio_chunks(audio_path: &Path, duration_ms: u64, chunk_count: u64) -> Vec<AudioChunk> {
    let chunk_duration = duration_ms / chunk_count.max(1);
    let mut split_points = Vec::with_capacity(chunk_count as usize + 1);
    split_points.push(0);

    for index in 1..chunk_count {
        let target_ms = index * chunk_duration;
        split_points.push(find_smart_split_point(audio_path, duration_ms, target_ms));
    }

    split_points.push(duration_ms);
    split_points.sort_unstable();
    split_points.dedup();

    split_points
        .windows(2)
        .filter_map(|points| {
            let start_ms = points[0];
            let end_ms = points[1];
            (end_ms > start_ms).then_some(AudioChunk { start_ms, end_ms })
        })
        .collect()
}

fn find_smart_split_point(audio_path: &Path, duration_ms: u64, target_ms: u64) -> u64 {
    let window_start_ms = target_ms.saturating_sub(SMART_SPLIT_WINDOW_MS);
    let window_end_ms = (target_ms + SMART_SPLIT_WINDOW_MS).min(duration_ms);
    let window_duration_ms = window_end_ms.saturating_sub(window_start_ms);

    if window_duration_ms < 600 {
        return target_ms;
    }

    let silence_threshold_db =
        probe_mean_volume_db(audio_path, window_start_ms, window_duration_ms)
            .map(|mean_volume| {
                (mean_volume - SILENCE_OFFSET_DB)
                    .max(SILENCE_THRESH_MIN_DB)
                    .min(SILENCE_THRESH_MAX_DB)
            })
            .unwrap_or(-35.0);

    let mut command = Command::new("ffmpeg");
    command
        .arg("-hide_banner")
        .arg("-nostats")
        .arg("-ss")
        .arg(format!("{:.3}", window_start_ms as f64 / 1000.0))
        .arg("-t")
        .arg(format!("{:.3}", window_duration_ms as f64 / 1000.0))
        .arg("-i")
        .arg(audio_path)
        .arg("-af")
        .arg(format!("silencedetect=noise={silence_threshold_db:.1}dB:d=0.3"))
        .arg("-f")
        .arg("null")
        .arg("-");

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }

    let output = match command.stdout(Stdio::piped()).stderr(Stdio::piped()).output() {
        Ok(output) => output,
        Err(_) => return target_ms,
    };

    if !output.status.success() {
        return target_ms;
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let ranges = parse_silence_ranges(&stderr);
    if ranges.is_empty() {
        return target_ms;
    }

    let target_in_window = target_ms.saturating_sub(window_start_ms);
    ranges
        .into_iter()
        .map(|(start_ms, end_ms)| (start_ms + end_ms) / 2)
        .min_by_key(|mid_ms| mid_ms.abs_diff(target_in_window))
        .map(|mid_ms| window_start_ms + mid_ms)
        .unwrap_or(target_ms)
}

fn probe_mean_volume_db(audio_path: &Path, start_ms: u64, duration_ms: u64) -> Option<f64> {
    let mut command = Command::new("ffmpeg");
    command
        .arg("-hide_banner")
        .arg("-nostats")
        .arg("-ss")
        .arg(format!("{:.3}", start_ms as f64 / 1000.0))
        .arg("-t")
        .arg(format!("{:.3}", duration_ms as f64 / 1000.0))
        .arg("-i")
        .arg(audio_path)
        .arg("-af")
        .arg("volumedetect")
        .arg("-f")
        .arg("null")
        .arg("-");

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }

    let output = command.stdout(Stdio::piped()).stderr(Stdio::piped()).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    parse_ffmpeg_key_db(&stderr, "mean_volume:")
}

fn parse_silence_ranges(ffmpeg_stderr: &str) -> Vec<(u64, u64)> {
    let mut ranges = Vec::new();
    let mut current_start_ms: Option<u64> = None;

    for line in ffmpeg_stderr.lines() {
        if let Some(value) = parse_ffmpeg_key_seconds(line, "silence_start:") {
            current_start_ms = Some(value);
            continue;
        }

        if let Some(value) = parse_ffmpeg_key_seconds(line, "silence_end:") {
            if let Some(start_ms) = current_start_ms.take() {
                if value > start_ms {
                    ranges.push((start_ms, value));
                }
            }
        }
    }

    ranges
}

fn parse_ffmpeg_key_seconds(line: &str, key: &str) -> Option<u64> {
    let start_index = line.find(key)? + key.len();
    let value_text = line[start_index..].trim_start();
    let seconds_text = value_text
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_end_matches(',');
    let seconds = seconds_text.parse::<f64>().ok()?;

    Some((seconds * 1000.0).round() as u64)
}

fn parse_ffmpeg_key_db(text: &str, key: &str) -> Option<f64> {
    for line in text.lines() {
        let start_index = match line.find(key) {
            Some(index) => index + key.len(),
            None => continue,
        };
        let value_text = line[start_index..].trim_start();
        let db_text = value_text
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .trim_end_matches(',');

        if let Ok(value) = db_text.parse::<f64>() {
            return Some(value);
        }
    }

    None
}

fn offset_utterance(utterance: &mut BcutUtterance, offset_ms: u64) {
    utterance.start_time = utterance.start_time.saturating_add(offset_ms);
    utterance.end_time = utterance.end_time.saturating_add(offset_ms);
    for word in &mut utterance.words {
        word.start_time = word.start_time.saturating_add(offset_ms);
        word.end_time = word.end_time.saturating_add(offset_ms);
    }
}

fn optimize_timing(segments: &mut [TranscriptionSegment], threshold_ms: u64) {
    if segments.len() < 2 {
        return;
    }

    for index in 0..segments.len() - 1 {
        let current_end = segments[index].end_time;
        let next_start = segments[index + 1].start_time;

        if next_start < current_end {
            let mid_time = (current_end + next_start) / 2;
            segments[index].end_time = mid_time;
            segments[index + 1].start_time = mid_time;
            continue;
        }

        let gap = next_start - current_end;
        if gap < threshold_ms {
            let mid_time = (current_end + next_start) / 2 + gap / 4;
            segments[index].end_time = mid_time;
            segments[index + 1].start_time = mid_time;
        }
    }
}

fn default_output_path(input_path: &Path, format: SubtitleFormat) -> Result<PathBuf, String> {
    let file_stem = input_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "无法获取视频文件名".to_string())?;
    let parent = input_path.parent().unwrap_or_else(|| Path::new("."));
    Ok(parent.join(format!("{file_stem}.{}", format.as_extension())))
}

#[derive(Debug, Clone, Copy)]
enum SubtitleFormat {
    Srt,
    Vtt,
    Ass,
}

impl SubtitleFormat {
    fn as_extension(self) -> &'static str {
        match self {
            Self::Srt => "srt",
            Self::Vtt => "vtt",
            Self::Ass => "ass",
        }
    }
}

impl std::fmt::Display for SubtitleFormat {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_extension())
    }
}

fn normalize_subtitle_format(format: &str) -> SubtitleFormat {
    match format {
        "vtt" => SubtitleFormat::Vtt,
        "ass" => SubtitleFormat::Ass,
        _ => SubtitleFormat::Srt,
    }
}

fn serialize_subtitle(segments: &[TranscriptionSegment], format: SubtitleFormat) -> String {
    match format {
        SubtitleFormat::Srt => to_srt(segments),
        SubtitleFormat::Vtt => to_vtt(segments),
        SubtitleFormat::Ass => to_ass(segments),
    }
}

fn to_srt(segments: &[TranscriptionSegment]) -> String {
    segments
        .iter()
        .enumerate()
        .map(|(index, segment)| {
            format!(
                "{}\n{} --> {}\n{}\n",
                index + 1,
                ms_to_srt_time(segment.start_time),
                ms_to_srt_time(segment.end_time),
                segment.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn to_vtt(segments: &[TranscriptionSegment]) -> String {
    let body = segments
        .iter()
        .map(|segment| {
            format!(
                "{} --> {}\n{}\n",
                ms_to_vtt_time(segment.start_time),
                ms_to_vtt_time(segment.end_time),
                segment.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("WEBVTT\n\n{body}")
}

fn to_ass(segments: &[TranscriptionSegment]) -> String {
    let mut content = String::from(
        "[Script Info]\n\
         Author: Link\n\
         ScriptType: v4.00+\n\
         PlayResX: 1280\n\
         PlayResY: 720\n\n\
         [V4+ Styles]\n\
         Format: Name,Fontname,Fontsize,PrimaryColour,SecondaryColour,OutlineColour,BackColour,Bold,Italic,Underline,StrikeOut,ScaleX,ScaleY,Spacing,Angle,BorderStyle,Outline,Shadow,Alignment,MarginL,MarginR,MarginV,Encoding\n\
         Style: Default,Microsoft YaHei,40,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,-1,0,0,0,100,100,0,0,1,2,0,2,10,10,15,1\n\n\
         [Events]\n\
         Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );

    for segment in segments {
        content.push_str(&format!(
            "Dialogue: 0,{},{},Default,,0,0,0,,{}\n",
            ms_to_ass_time(segment.start_time),
            ms_to_ass_time(segment.end_time),
            escape_ass_text(&segment.text)
        ));
    }

    content
}

fn ms_to_srt_time(ms: u64) -> String {
    let milliseconds = ms % 1000;
    let total_seconds = ms / 1000;
    let seconds = total_seconds % 60;
    let total_minutes = total_seconds / 60;
    let minutes = total_minutes % 60;
    let hours = total_minutes / 60;

    format!("{hours:02}:{minutes:02}:{seconds:02},{milliseconds:03}")
}

fn ms_to_vtt_time(ms: u64) -> String {
    ms_to_srt_time(ms).replace(',', ".")
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

fn escape_ass_text(text: &str) -> String {
    text.replace('\n', "\\N")
        .replace('{', "")
        .replace('}', "")
}
