use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const COMPOSE_VERSION: u32 = 1;
const COMPOSE_DIR_NAME: &str = "video_compose";
const FINAL_VIDEO_FILE_NAME: &str = "final_dubbed_video.mp4";
const TEMP_VIDEO_FILE_NAME: &str = "temp_final_dubbed_video.mp4";
const MANIFEST_FILE_NAME: &str = "manifest.json";
const AUDIO_SAMPLE_RATE: u32 = 44_100;
const AUDIO_CHANNELS: u32 = 2;
const AUDIO_BITRATE: &str = "192k";
const VIDEO_CRF: u8 = 23;
const VIDEO_PRESET: &str = "medium";

const COMPOSE_STEP_INPUT_VALIDATION: &str = "input-validation";
const COMPOSE_STEP_AUDIO_MIX: &str = "audio-mix";
const COMPOSE_STEP_VIDEO_MUX: &str = "video-mux";
const COMPOSE_STEP_OUTPUT_VALIDATION: &str = "output-validation";

#[derive(Debug, Clone)]
pub struct DubbingComposeInput {
    pub work_dir: PathBuf,
    pub aligned_video_path: PathBuf,
    pub aligned_audio_path: PathBuf,
    pub aligned_subtitle_path: PathBuf,
    pub aligned_background_music_path: Option<PathBuf>,
    pub alignment_manifest_path: PathBuf,
    pub background_music_enabled: bool,
    pub background_music_volume: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingComposeProgressStep {
    pub key: String,
    pub label: String,
    pub description: String,
    pub progress: u8,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingComposeProgress {
    pub progress: u8,
    pub message: String,
    pub active_step: String,
    pub steps: Vec<DubbingComposeProgressStep>,
    pub background_music_enabled: bool,
    pub background_music_volume: f64,
}

#[derive(Debug, Clone)]
pub struct DubbingComposeResult {
    pub final_video_path: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest: Value,
    pub stage_snapshot: Value,
    pub duration_ms: u64,
    pub file_size: u64,
    pub resolution: String,
    pub video_codec: String,
    pub audio_codec: String,
}

struct DubbingComposeProgressState {
    steps: Vec<DubbingComposeProgressStep>,
    background_music_enabled: bool,
    background_music_volume: f64,
}

#[derive(Default)]
struct VideoInfo {
    duration_ms: u64,
    width: u64,
    height: u64,
    video_codec: String,
    audio_codec: String,
}

impl DubbingComposeProgressState {
    fn new(background_music_enabled: bool, background_music_volume: f64) -> Self {
        Self {
            steps: vec![
                compose_progress_step(
                    COMPOSE_STEP_INPUT_VALIDATION,
                    "合成准备",
                    "确认对齐产物和输出位置",
                ),
                compose_progress_step(
                    COMPOSE_STEP_AUDIO_MIX,
                    "输出音轨",
                    if background_music_enabled {
                        "按音量生成最终输出音轨"
                    } else {
                        "写入对齐配音音轨"
                    },
                ),
                compose_progress_step(COMPOSE_STEP_VIDEO_MUX, "MP4 封装", "写入画面轨和输出音轨"),
                compose_progress_step(
                    COMPOSE_STEP_OUTPUT_VALIDATION,
                    "成片校验",
                    "校验最终文件、分辨率和清单",
                ),
            ],
            background_music_enabled,
            background_music_volume,
        }
    }

    fn set_step(&mut self, key: &str, progress: u8, status: &str) {
        if let Some(step) = self.steps.iter_mut().find(|step| step.key == key) {
            step.progress = progress.min(100);
            step.status = status.to_string();
        }
    }

    fn activate_step(&mut self, key: &str, progress: u8) {
        for step in &mut self.steps {
            if step.key == key {
                step.progress = progress.min(100);
                step.status = "active".to_string();
            } else if step.status == "active" {
                step.progress = 100;
                step.status = "done".to_string();
            }
        }
    }

    fn complete_step(&mut self, key: &str) {
        self.set_step(key, 100, "done");
    }

    fn snapshot(&self, progress: u8, message: &str, active_step: &str) -> DubbingComposeProgress {
        DubbingComposeProgress {
            progress: progress.min(100),
            message: message.to_string(),
            active_step: active_step.to_string(),
            steps: self.steps.clone(),
            background_music_enabled: self.background_music_enabled,
            background_music_volume: self.background_music_volume,
        }
    }
}

fn compose_progress_step(key: &str, label: &str, description: &str) -> DubbingComposeProgressStep {
    DubbingComposeProgressStep {
        key: key.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        progress: 0,
        status: "pending".to_string(),
    }
}

pub fn run_dubbing_compose<F>(
    input: DubbingComposeInput,
    mut emit_progress: F,
) -> Result<DubbingComposeResult, String>
where
    F: FnMut(&DubbingComposeProgress) -> Result<(), String>,
{
    let background_music_volume = normalize_background_music_volume(input.background_music_volume);
    let mut progress_state =
        DubbingComposeProgressState::new(input.background_music_enabled, background_music_volume);

    let output_dir = input.work_dir.join(COMPOSE_DIR_NAME);
    if output_dir.exists() {
        fs::remove_dir_all(&output_dir)
            .map_err(|error| format!("无法清理视频合成目录: {error}"))?;
    }
    fs::create_dir_all(&output_dir).map_err(|error| format!("无法创建视频合成目录: {error}"))?;

    progress_state.activate_step(COMPOSE_STEP_INPUT_VALIDATION, 20);
    emit_progress(&progress_state.snapshot(5, "读取合成输入", COMPOSE_STEP_INPUT_VALIDATION))?;

    ensure_non_empty_file(&input.aligned_video_path, "对齐视频不存在或为空")?;
    ensure_non_empty_file(&input.aligned_audio_path, "对齐 TTS 音频不存在或为空")?;
    ensure_non_empty_file(&input.aligned_subtitle_path, "对齐字幕不存在或为空")?;
    ensure_non_empty_file(&input.alignment_manifest_path, "音视频对齐清单不存在或为空")?;
    if let Some(path) = &input.aligned_background_music_path {
        ensure_non_empty_file(path, "对齐背景音乐不存在或为空")?;
    }
    if input.background_music_enabled && input.aligned_background_music_path.is_none() {
        return Err("背景音乐已开启，但缺少对齐背景音乐".to_string());
    }

    let aligned_video_duration_ms = probe_media_duration_ms(&input.aligned_video_path)?;
    let aligned_audio_duration_ms = probe_media_duration_ms(&input.aligned_audio_path)?;
    if aligned_video_duration_ms == 0 || aligned_audio_duration_ms == 0 {
        return Err("对齐视频或音频时长为 0，无法合成".to_string());
    }

    progress_state.complete_step(COMPOSE_STEP_INPUT_VALIDATION);
    if input.background_music_enabled {
        progress_state.activate_step(COMPOSE_STEP_AUDIO_MIX, 0);
        emit_progress(&progress_state.snapshot(22, "准备混合背景音乐", COMPOSE_STEP_AUDIO_MIX))?;
    } else {
        progress_state.complete_step(COMPOSE_STEP_AUDIO_MIX);
    }

    progress_state.activate_step(COMPOSE_STEP_VIDEO_MUX, 0);
    emit_progress(&progress_state.snapshot(35, "封装最终视频", COMPOSE_STEP_VIDEO_MUX))?;

    let final_video_path = output_dir.join(FINAL_VIDEO_FILE_NAME);
    let temp_video_path = output_dir.join(TEMP_VIDEO_FILE_NAME);
    compose_video(&input, &temp_video_path, background_music_volume)?;
    ensure_non_empty_file(&temp_video_path, "视频合成临时文件为空")?;
    replace_file(&temp_video_path, &final_video_path)?;
    ensure_non_empty_file(&final_video_path, "最终配音视频为空")?;

    progress_state.complete_step(COMPOSE_STEP_VIDEO_MUX);
    progress_state.activate_step(COMPOSE_STEP_OUTPUT_VALIDATION, 0);
    emit_progress(&progress_state.snapshot(88, "校验最终视频", COMPOSE_STEP_OUTPUT_VALIDATION))?;

    let video_info = probe_video_info(&final_video_path)?;
    let file_size = file_size(&final_video_path)?;
    let resolution = if video_info.width > 0 && video_info.height > 0 {
        format!("{}x{}", video_info.width, video_info.height)
    } else {
        String::new()
    };
    let alignment_manifest_hash = file_sha256(&input.alignment_manifest_path)?;
    let manifest_path = output_dir.join(MANIFEST_FILE_NAME);
    let manifest = json!({
        "composeVersion": COMPOSE_VERSION,
        "alignmentManifestPath": path_to_string(&input.alignment_manifest_path),
        "alignmentManifestHash": alignment_manifest_hash,
        "alignedVideoPath": path_to_string(&input.aligned_video_path),
        "alignedAudioPath": path_to_string(&input.aligned_audio_path),
        "alignedSubtitlePath": path_to_string(&input.aligned_subtitle_path),
        "alignedBackgroundMusicPath": input.aligned_background_music_path.as_ref().map(|path| path_to_string(path)),
        "outputPath": path_to_string(&final_video_path),
        "backgroundMusicEnabled": input.background_music_enabled,
        "backgroundMusicVolume": background_music_volume,
        "durationMs": video_info.duration_ms,
        "fileSize": file_size,
        "resolution": resolution,
        "videoCodec": video_info.video_codec,
        "audioCodec": video_info.audio_codec,
    });
    let manifest_text = serde_json::to_string_pretty(&manifest)
        .map_err(|error| format!("无法序列化视频合成清单: {error}"))?;
    fs::write(&manifest_path, manifest_text)
        .map_err(|error| format!("无法保存视频合成清单: {error}"))?;

    progress_state.complete_step(COMPOSE_STEP_OUTPUT_VALIDATION);
    let final_progress =
        progress_state.snapshot(99, "视频合成收尾", COMPOSE_STEP_OUTPUT_VALIDATION);
    let stage_snapshot = json!({
        "outputPath": path_to_string(&final_video_path),
        "manifestPath": path_to_string(&manifest_path),
        "durationMs": video_info.duration_ms,
        "fileSize": file_size,
        "resolution": resolution,
        "videoCodec": video_info.video_codec,
        "audioCodec": video_info.audio_codec,
        "backgroundMusicEnabled": input.background_music_enabled,
        "backgroundMusicVolume": background_music_volume,
        "steps": &final_progress.steps,
    });

    emit_progress(&final_progress)?;
    Ok(DubbingComposeResult {
        final_video_path,
        manifest_path,
        manifest,
        stage_snapshot,
        duration_ms: video_info.duration_ms,
        file_size,
        resolution,
        video_codec: video_info.video_codec,
        audio_codec: video_info.audio_codec,
    })
}

fn compose_video(
    input: &DubbingComposeInput,
    output_path: &Path,
    background_music_volume: f64,
) -> Result<(), String> {
    let mut command = Command::new("ffmpeg");
    command
        .arg("-hide_banner")
        .arg("-nostdin")
        .arg("-nostats")
        .arg("-i")
        .arg(&input.aligned_video_path)
        .arg("-i")
        .arg(&input.aligned_audio_path);

    if let Some(background_music_path) = &input.aligned_background_music_path {
        command
            .arg("-i")
            .arg(background_music_path)
            .arg("-filter_complex")
            .arg(format!(
                "[2:a]volume={background_music_volume:.4}[bgm];[1:a][bgm]amix=inputs=2:duration=first:dropout_transition=0:normalize=0,alimiter=limit=0.95[aout]"
            ))
            .arg("-map")
            .arg("0:v:0")
            .arg("-map")
            .arg("[aout]");
    } else {
        command.arg("-map").arg("0:v:0").arg("-map").arg("1:a:0");
    }

    command
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg(VIDEO_PRESET)
        .arg("-crf")
        .arg(VIDEO_CRF.to_string())
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg(AUDIO_BITRATE)
        .arg("-ar")
        .arg(AUDIO_SAMPLE_RATE.to_string())
        .arg("-ac")
        .arg(AUDIO_CHANNELS.to_string())
        .arg("-movflags")
        .arg("+faststart")
        .arg("-max_muxing_queue_size")
        .arg("9999")
        .arg("-y")
        .arg(output_path);

    run_command(&mut command, "视频合成失败")
}

fn normalize_background_music_volume(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

fn ensure_non_empty_file(path: &Path, message: &str) -> Result<(), String> {
    let metadata = fs::metadata(path).map_err(|error| format!("{message}: {error}"))?;
    if metadata.len() == 0 {
        Err(message.to_string())
    } else {
        Ok(())
    }
}

fn file_size(path: &Path) -> Result<u64, String> {
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .map_err(|error| format!("无法读取文件大小: {error}"))
}

fn file_sha256(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("无法读取文件 hash: {error}"))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn replace_file(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        fs::remove_file(destination).map_err(|error| format!("无法更新视频合成文件: {error}"))?;
    }
    fs::rename(source, destination).map_err(|error| format!("无法保存最终配音视频: {error}"))
}

fn probe_media_duration_ms(path: &Path) -> Result<u64, String> {
    let mut command = Command::new("ffprobe");
    command
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(path);
    let (stdout, _) = run_command_with_output(&mut command, "无法获取媒体时长")?;
    parse_duration_ms(stdout.trim())
}

fn probe_video_info(path: &Path) -> Result<VideoInfo, String> {
    let duration_ms = probe_media_duration_ms(path)?;
    let mut info = VideoInfo {
        duration_ms,
        ..VideoInfo::default()
    };

    let mut video_command = Command::new("ffprobe");
    video_command
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-show_entries")
        .arg("stream=codec_name,width,height")
        .arg("-of")
        .arg("default=noprint_wrappers=1")
        .arg(path);
    let (stdout, _) = run_command_with_output(&mut video_command, "无法获取视频信息")?;
    for line in stdout.lines() {
        if let Some(value) = line.strip_prefix("codec_name=") {
            info.video_codec = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("width=") {
            info.width = value.trim().parse::<u64>().unwrap_or_default();
        } else if let Some(value) = line.strip_prefix("height=") {
            info.height = value.trim().parse::<u64>().unwrap_or_default();
        }
    }

    let mut audio_command = Command::new("ffprobe");
    audio_command
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("a:0")
        .arg("-show_entries")
        .arg("stream=codec_name")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(path);
    let (stdout, _) = run_command_with_output(&mut audio_command, "无法获取音频信息")?;
    info.audio_codec = stdout.trim().to_string();

    if info.video_codec.is_empty() {
        return Err("最终视频缺少视频轨道".to_string());
    }
    if info.audio_codec.is_empty() {
        return Err("最终视频缺少音频轨道".to_string());
    }

    Ok(info)
}

fn parse_duration_ms(value: &str) -> Result<u64, String> {
    let seconds = value
        .parse::<f64>()
        .map_err(|error| format!("无法解析媒体时长: {error}"))?;
    if !seconds.is_finite() || seconds <= 0.0 {
        return Ok(0);
    }
    Ok((seconds * 1000.0).round() as u64)
}

fn run_command(command: &mut Command, failure_message: &str) -> Result<(), String> {
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

fn run_command_with_output(
    command: &mut Command,
    failure_message: &str,
) -> Result<(String, String), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }

    let output = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("{failure_message}: 无法启动 ffprobe: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if output.status.success() {
        Ok((stdout, stderr))
    } else {
        Err(format!("{failure_message}: {}", stderr.trim()))
    }
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
