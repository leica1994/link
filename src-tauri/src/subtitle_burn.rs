use crate::app_log::{AppLogger, LogSession};
use crate::command_utils::create_command;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use tauri::{AppHandle, Emitter, Manager};
use tempfile::TempDir;

const PROGRESS_EVENT: &str = "subtitle-burn-progress";
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "mkv", "avi", "flv", "wmv", "webm", "m4v"];
const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "vtt", "ass"];

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleBurnRequest {
    pub video_path: String,
    pub subtitle_path: String,
    pub output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleBurnProgress {
    pub progress: u8,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleBurnResult {
    pub output_path: String,
    pub duration_ms: Option<u64>,
}

struct PreparedSubtitleFilterInput {
    temp_dir: TempDir,
    file_name: String,
}

impl PreparedSubtitleFilterInput {
    fn work_dir(&self) -> &Path {
        self.temp_dir.path()
    }

    fn filter_argument(&self) -> String {
        format!("subtitles={}", self.file_name)
    }
}

#[tauri::command]
pub async fn start_subtitle_burn(
    app: AppHandle,
    request: SubtitleBurnRequest,
) -> Result<SubtitleBurnResult, String> {
    tauri::async_runtime::spawn_blocking(move || start_subtitle_burn_blocking(app, request))
        .await
        .map_err(|error| format!("字幕烧录任务异常: {error}"))?
}

fn start_subtitle_burn_blocking(
    app: AppHandle,
    request: SubtitleBurnRequest,
) -> Result<SubtitleBurnResult, String> {
    let app_logger = app.state::<AppLogger>();
    let log_session = app_logger.start_session("subtitle_burn")?;
    log_session.info(
        "request_received",
        "收到字幕烧录请求",
        json!({
            "videoPath": request.video_path.trim(),
            "subtitlePath": request.subtitle_path.trim(),
            "outputPath": request.output_path.as_deref().unwrap_or("").trim(),
        }),
    );

    let result = start_subtitle_burn_inner(&app, request, &log_session);
    match result {
        Ok(result) => {
            log_session.info(
                "subtitle_burn_completed",
                "字幕烧录完成",
                json!({
                    "outputPath": &result.output_path,
                    "durationMs": result.duration_ms,
                    "logPath": log_session.path_string(),
                }),
            );
            Ok(result)
        }
        Err(error) => {
            log_session.error(
                "subtitle_burn_failed",
                "字幕烧录失败",
                json!({ "error": &error }),
            );
            Err(error)
        }
    }
}

fn start_subtitle_burn_inner(
    app: &AppHandle,
    request: SubtitleBurnRequest,
    log_session: &LogSession,
) -> Result<SubtitleBurnResult, String> {
    emit_progress(app, 0, "准备烧录", None);

    let video_path = PathBuf::from(request.video_path.trim());
    let subtitle_path = PathBuf::from(request.subtitle_path.trim());
    validate_input_file(&video_path, VIDEO_EXTENSIONS, "视频")?;
    validate_input_file(&subtitle_path, SUBTITLE_EXTENSIONS, "字幕")?;

    let output_path = normalize_output_path(request.output_path.as_deref(), &video_path)?;
    validate_output_path(&video_path, &output_path)?;
    let output_path_string = path_to_string(&output_path);

    emit_progress(app, 8, "检测视频时长", Some(output_path_string.clone()));
    let duration_ms = match probe_duration_ms(&video_path) {
        Ok(duration) if duration > 0 => {
            log_session.info(
                "video_duration_detected",
                "已检测视频时长",
                json!({ "durationMs": duration }),
            );
            Some(duration)
        }
        Ok(_) => {
            log_session.warn(
                "video_duration_empty",
                "视频时长检测结果为空，将继续烧录",
                json!({}),
            );
            None
        }
        Err(error) => {
            log_session.warn(
                "video_duration_probe_failed",
                "视频时长检测失败，将继续烧录",
                json!({ "error": &error }),
            );
            None
        }
    };

    emit_progress(app, 18, "启动 ffmpeg", Some(output_path_string.clone()));
    log_session.info(
        "ffmpeg_burn_start",
        "启动 ffmpeg 烧录字幕",
        json!({
            "videoPath": video_path.to_string_lossy(),
            "subtitlePath": subtitle_path.to_string_lossy(),
            "outputPath": output_path.to_string_lossy(),
        }),
    );
    run_ffmpeg_burn(app, &video_path, &subtitle_path, &output_path, duration_ms)?;
    log_session.info(
        "ffmpeg_burn_success",
        "ffmpeg 字幕烧录成功",
        json!({ "outputPath": output_path.to_string_lossy() }),
    );

    emit_progress(app, 100, "烧录完成", Some(output_path_string.clone()));
    Ok(SubtitleBurnResult {
        output_path: output_path_string,
        duration_ms,
    })
}

fn validate_input_file(
    path: &Path,
    allowed_extensions: &[&str],
    label: &str,
) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Err(format!("请选择{label}文件"));
    }

    if !path.exists() {
        return Err(format!("{label}文件不存在"));
    }

    if !path.is_file() {
        return Err(format!("请选择有效的{label}文件"));
    }

    if !has_allowed_extension(path, allowed_extensions) {
        return Err(format!("{label}文件格式不支持"));
    }

    Ok(())
}

fn normalize_output_path(output_path: Option<&str>, video_path: &Path) -> Result<PathBuf, String> {
    let output_path = output_path
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| default_output_path(video_path));

    let output_path = if output_path.extension().is_some() {
        output_path
    } else {
        output_path.with_extension("mp4")
    };

    if !has_allowed_extension(&output_path, &["mp4"]) {
        return Err("输出文件需要使用 MP4 扩展名".to_string());
    }

    Ok(output_path)
}

fn validate_output_path(video_path: &Path, output_path: &Path) -> Result<(), String> {
    if paths_refer_same_file(video_path, output_path) {
        return Err("输出文件不能覆盖源视频".to_string());
    }

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return Err("输出目录不存在".to_string());
        }
    }

    Ok(())
}

fn default_output_path(video_path: &Path) -> PathBuf {
    let parent = video_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let stem = video_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("video");
    unique_output_path(parent.join(format!("{stem}_burned.mp4")))
}

fn unique_output_path(base_path: PathBuf) -> PathBuf {
    if !base_path.exists() {
        return base_path;
    }

    let parent = base_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let stem = base_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("video_burned");

    for index in 1..1000 {
        let candidate = parent.join(format!("{stem}_{index}.mp4"));
        if !candidate.exists() {
            return candidate;
        }
    }

    parent.join(format!("{stem}_latest.mp4"))
}

fn prepare_subtitle_filter_input(
    subtitle_path: &Path,
) -> Result<PreparedSubtitleFilterInput, String> {
    let extension = subtitle_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_else(|| "ass".to_string());
    let file_name = format!("subtitle.{extension}");
    let temp_dir = tempfile::Builder::new()
        .prefix("link-subtitle-burn-")
        .tempdir()
        .map_err(|error| format!("无法创建字幕烧录临时目录: {error}"))?;
    let copied_path = temp_dir.path().join(&file_name);

    fs::copy(subtitle_path, &copied_path)
        .map_err(|error| format!("无法准备字幕烧录临时文件: {error}"))?;

    Ok(PreparedSubtitleFilterInput {
        temp_dir,
        file_name,
    })
}

fn run_ffmpeg_burn(
    app: &AppHandle,
    video_path: &Path,
    subtitle_path: &Path,
    output_path: &Path,
    duration_ms: Option<u64>,
) -> Result<(), String> {
    let subtitle_filter_input = prepare_subtitle_filter_input(subtitle_path)?;
    let subtitle_filter = subtitle_filter_input.filter_argument();
    let output_path_string = path_to_string(output_path);

    let mut command = create_command("ffmpeg");
    command
        .current_dir(subtitle_filter_input.work_dir())
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
        .arg(subtitle_filter)
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
    hide_command_window(&mut command);

    let mut child = command
        .spawn()
        .map_err(|error| format!("无法启动 ffmpeg: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取 ffmpeg 进度".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "无法读取 ffmpeg 错误输出".to_string())?;
    let stderr_handle = thread::spawn(move || read_stream_to_string(stderr));

    emit_progress(
        app,
        24,
        "ffmpeg 正在烧录字幕",
        Some(output_path_string.clone()),
    );
    let mut last_progress = 24_u8;
    let reader = BufReader::new(stdout);
    for line in reader.lines().map_while(Result::ok) {
        if let (Some(duration), Some(time_ms)) = (duration_ms, parse_progress_time_ms(&line)) {
            let scaled = 24_u8.saturating_add(((time_ms.min(duration) * 72) / duration) as u8);
            let progress = scaled.clamp(last_progress, 96);
            if progress > last_progress {
                last_progress = progress;
                emit_progress(
                    app,
                    progress,
                    "ffmpeg 正在烧录字幕",
                    Some(output_path_string.clone()),
                );
            }
        }
    }

    let status = child
        .wait()
        .map_err(|error| format!("等待 ffmpeg 结束失败: {error}"))?;
    let stderr = stderr_handle.join().unwrap_or_default();

    if status.success() {
        Ok(())
    } else {
        let _ = fs::remove_file(output_path);
        Err(format!("字幕烧录失败: {}", summarize_stderr(&stderr)))
    }
}

fn probe_duration_ms(path: &Path) -> Result<u64, String> {
    let mut command = create_command("ffprobe");
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
    hide_command_window(&mut command);

    let output = command
        .output()
        .map_err(|error| format!("无法启动 ffprobe: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("无法获取视频时长: {}", summarize_stderr(&stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let seconds = stdout
        .trim()
        .parse::<f64>()
        .map_err(|error| format!("无法解析视频时长: {error}"))?;
    Ok((seconds * 1000.0).round() as u64)
}

fn parse_progress_time_ms(line: &str) -> Option<u64> {
    let (key, value) = line.split_once('=')?;
    match key {
        "out_time_ms" | "out_time_us" => value.trim().parse::<u64>().ok().map(|value| value / 1000),
        "out_time" => parse_timestamp_ms(value),
        _ => None,
    }
}

fn parse_timestamp_ms(value: &str) -> Option<u64> {
    let mut parts = value.trim().split(':');
    let hours = parts.next()?.parse::<u64>().ok()?;
    let minutes = parts.next()?.parse::<u64>().ok()?;
    let seconds = parts.next()?.parse::<f64>().ok()?;
    if parts.next().is_some() {
        return None;
    }

    Some(hours * 3_600_000 + minutes * 60_000 + (seconds * 1000.0).round() as u64)
}

fn has_allowed_extension(path: &Path, allowed_extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .is_some_and(|extension| allowed_extensions.contains(&extension.as_str()))
}

fn paths_refer_same_file(left: &Path, right: &Path) -> bool {
    normalize_for_compare(left) == normalize_for_compare(right)
}

fn normalize_for_compare(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn summarize_stderr(stderr: &str) -> String {
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        return "ffmpeg 未返回错误详情".to_string();
    }

    let mut summary: String = trimmed.chars().take(1200).collect();
    if trimmed.chars().count() > summary.chars().count() {
        summary.push_str("...");
    }
    summary
}

fn read_stream_to_string(mut stream: impl Read) -> String {
    let mut output = String::new();
    let _ = stream.read_to_string(&mut output);
    output
}

fn emit_progress(app: &AppHandle, progress: u8, message: &str, output_path: Option<String>) {
    let _ = app.emit(
        PROGRESS_EVENT,
        SubtitleBurnProgress {
            progress: progress.min(100),
            message: message.to_string(),
            output_path,
        },
    );
}

fn hide_command_window(command: &mut Command) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepares_safe_relative_subtitle_filter_input_for_quoted_paths() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let subtitle_path = dir
            .path()
            .join("We Didn't Expect Kosovo's Fast Food to Be THIS Good.ass");
        fs::write(&subtitle_path, "subtitle content").expect("write subtitle");

        let prepared =
            prepare_subtitle_filter_input(&subtitle_path).expect("prepare subtitle filter input");
        let copied_path = prepared.work_dir().join(&prepared.file_name);

        assert_eq!(prepared.file_name, "subtitle.ass");
        assert_eq!(prepared.filter_argument(), "subtitles=subtitle.ass");
        assert_eq!(
            fs::read_to_string(copied_path).expect("read copied subtitle"),
            "subtitle content"
        );
        assert!(!prepared.file_name.contains('\''));
        assert!(!prepared.file_name.contains('\\'));
        assert!(!prepared.file_name.contains(':'));
    }
}
