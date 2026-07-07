use crate::command_utils::create_command;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const ALIGNMENT_VERSION: u32 = 1;
const AUDIO_SAMPLE_RATE: u32 = 44_100;
const AUDIO_CHANNELS: u32 = 2;
const MIN_CLIP_DURATION_MS: u64 = 50;
const TOLERANCE_MS: u64 = 60;
const MAX_SLOWDOWN_PTS: f64 = 2.0;
const TTS_FADE_IN_MS: u64 = 25;
const TTS_FADE_OUT_MS: u64 = 60;
const ALIGNMENT_STEP_INPUT_VALIDATION: &str = "input-validation";
const ALIGNMENT_STEP_TIMELINE_PLANNING: &str = "timeline-planning";
const ALIGNMENT_STEP_VIDEO_ALIGNMENT: &str = "video-alignment";
const ALIGNMENT_STEP_MAIN_AUDIO_REBUILD: &str = "main-audio-rebuild";
const ALIGNMENT_STEP_BACKGROUND_MUSIC_SYNC: &str = "background-music-sync";
const ALIGNMENT_STEP_OUTPUT_FINALIZE: &str = "output-finalize";

#[derive(Clone)]
pub struct DubbingAlignmentInput {
    pub work_dir: PathBuf,
    pub muted_video_path: PathBuf,
    pub background_music_path: Option<PathBuf>,
    pub tts_interval_ms: u32,
    pub segments: Vec<DubbingAlignmentSegmentInput>,
}

#[derive(Clone)]
pub struct DubbingAlignmentSegmentInput {
    pub index: usize,
    pub uid: String,
    pub text: String,
    pub start_time_ms: u64,
    pub end_time_ms: u64,
    pub tts_path: PathBuf,
    pub tts_duration_ms: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingAlignmentSegmentResult {
    pub index: usize,
    pub uid: String,
    pub text: String,
    pub source_start_ms: u64,
    pub source_end_ms: u64,
    pub tts_duration_ms: u64,
    pub pause_duration_ms: u64,
    pub aligned_start_ms: u64,
    pub aligned_end_ms: u64,
    pub block_duration_ms: u64,
    pub video_mode: String,
    pub pts: f64,
    pub freeze_tail_ms: u64,
    pub warning: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingAlignmentProgressStep {
    pub key: String,
    pub label: String,
    pub description: String,
    pub progress: u8,
    pub status: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingAlignmentProgress {
    pub progress: u8,
    pub message: String,
    pub active_step: String,
    pub steps: Vec<DubbingAlignmentProgressStep>,
    pub segment_count: usize,
    pub processed_video_clips: usize,
    pub processed_audio_clips: usize,
    pub processed_background_clips: usize,
    pub background_music_enabled: bool,
    pub mode_counts: BTreeMap<String, usize>,
    pub segments: Vec<DubbingAlignmentSegmentResult>,
}

pub struct DubbingAlignmentResult {
    pub aligned_video_path: PathBuf,
    pub aligned_audio_path: PathBuf,
    pub aligned_subtitle_path: PathBuf,
    pub aligned_background_music_path: Option<PathBuf>,
    pub manifest_path: PathBuf,
    pub manifest: Value,
    pub stage_snapshot: Value,
    pub segments: Vec<DubbingAlignmentSegmentResult>,
    pub warnings: Vec<String>,
}

#[derive(Clone)]
enum AlignmentStepKind {
    Gap,
    Subtitle(usize),
}

#[derive(Clone)]
struct AlignmentStep {
    kind: AlignmentStepKind,
    source_start_ms: u64,
    source_end_ms: u64,
    target_duration_ms: u64,
    real_duration_ms: u64,
    pts: f64,
    freeze_tail_ms: u64,
    mode: String,
    video_path: PathBuf,
    audio_path: PathBuf,
    background_path: Option<PathBuf>,
}

struct DubbingAlignmentProgressState {
    steps: Vec<DubbingAlignmentProgressStep>,
    segment_count: usize,
    processed_video_clips: usize,
    processed_audio_clips: usize,
    processed_background_clips: usize,
    background_music_enabled: bool,
    segments: Vec<DubbingAlignmentSegmentResult>,
}

impl DubbingAlignmentProgressState {
    fn new(segment_count: usize, background_music_enabled: bool) -> Self {
        let mut steps = vec![
            alignment_progress_step(
                ALIGNMENT_STEP_INPUT_VALIDATION,
                "输入校验",
                "读取无声视频、TTS 清单、背景音乐",
            ),
            alignment_progress_step(
                ALIGNMENT_STEP_TIMELINE_PLANNING,
                "时间轴规划",
                "计算字幕块、间隙、慢放/冻结策略",
            ),
            alignment_progress_step(
                ALIGNMENT_STEP_VIDEO_ALIGNMENT,
                "视频对齐",
                "裁切、慢放、冻结尾帧",
            ),
            alignment_progress_step(
                ALIGNMENT_STEP_MAIN_AUDIO_REBUILD,
                "主音频重建",
                "静音画布、TTS 叠加、拼接",
            ),
            alignment_progress_step(
                ALIGNMENT_STEP_BACKGROUND_MUSIC_SYNC,
                "背景音乐同步",
                if background_music_enabled {
                    "背景音乐按对齐时间轴同步"
                } else {
                    "背景音乐已关闭，跳过同步"
                },
            ),
            alignment_progress_step(
                ALIGNMENT_STEP_OUTPUT_FINALIZE,
                "产物收尾",
                "拼接、时长纠偏、字幕和清单输出",
            ),
        ];
        if !background_music_enabled {
            if let Some(step) = steps
                .iter_mut()
                .find(|step| step.key == ALIGNMENT_STEP_BACKGROUND_MUSIC_SYNC)
            {
                step.progress = 100;
                step.status = "done".to_string();
            }
        }

        Self {
            steps,
            segment_count,
            processed_video_clips: 0,
            processed_audio_clips: 0,
            processed_background_clips: 0,
            background_music_enabled,
            segments: Vec::new(),
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

    fn snapshot(&self, progress: u8, message: &str, active_step: &str) -> DubbingAlignmentProgress {
        DubbingAlignmentProgress {
            progress: progress.min(100),
            message: message.to_string(),
            active_step: active_step.to_string(),
            steps: self.steps.clone(),
            segment_count: self.segment_count,
            processed_video_clips: self.processed_video_clips,
            processed_audio_clips: self.processed_audio_clips,
            processed_background_clips: self.processed_background_clips,
            background_music_enabled: self.background_music_enabled,
            mode_counts: alignment_mode_counts(&self.segments),
            segments: self.segments.clone(),
        }
    }
}

fn alignment_progress_step(
    key: &str,
    label: &str,
    description: &str,
) -> DubbingAlignmentProgressStep {
    DubbingAlignmentProgressStep {
        key: key.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        progress: 0,
        status: "pending".to_string(),
    }
}

pub fn run_dubbing_alignment<F>(
    input: DubbingAlignmentInput,
    mut emit_progress: F,
) -> Result<DubbingAlignmentResult, String>
where
    F: FnMut(&DubbingAlignmentProgress) -> Result<(), String>,
{
    if input.segments.is_empty() {
        return Err("没有可对齐的字幕".to_string());
    }
    ensure_non_empty_file(&input.muted_video_path, "无声视频不存在或为空")?;
    for segment in &input.segments {
        if segment.end_time_ms <= segment.start_time_ms {
            return Err(format!("第 {} 条字幕时间轴无效", segment.index + 1));
        }
        if segment.tts_duration_ms == 0 {
            return Err(format!("第 {} 条字幕 TTS 时长为 0", segment.index + 1));
        }
        ensure_non_empty_file(
            &segment.tts_path,
            &format!("第 {} 条字幕 TTS 音频不存在或为空", segment.index + 1),
        )?;
    }

    let output_dir = input.work_dir.join("audio_video_alignment");
    if output_dir.exists() {
        fs::remove_dir_all(&output_dir)
            .map_err(|error| format!("无法清理音视频对齐目录: {error}"))?;
    }
    let video_dir = output_dir.join("video_clips");
    let audio_dir = output_dir.join("audio_clips");
    let background_dir = output_dir.join("background_clips");
    fs::create_dir_all(&video_dir).map_err(|error| format!("无法创建视频对齐目录: {error}"))?;
    fs::create_dir_all(&audio_dir).map_err(|error| format!("无法创建音频对齐目录: {error}"))?;
    if input.background_music_path.is_some() {
        fs::create_dir_all(&background_dir)
            .map_err(|error| format!("无法创建背景音乐对齐目录: {error}"))?;
    }

    let mut progress_state = DubbingAlignmentProgressState::new(
        input.segments.len(),
        input.background_music_path.is_some(),
    );

    progress_state.activate_step(ALIGNMENT_STEP_INPUT_VALIDATION, 20);
    emit_progress(&progress_state.snapshot(5, "读取音视频信息", ALIGNMENT_STEP_INPUT_VALIDATION))?;
    let video_duration_ms = probe_media_duration_ms(&input.muted_video_path)?;
    if video_duration_ms == 0 {
        return Err("无声视频时长为 0，无法对齐".to_string());
    }

    progress_state.complete_step(ALIGNMENT_STEP_INPUT_VALIDATION);
    progress_state.activate_step(ALIGNMENT_STEP_TIMELINE_PLANNING, 20);
    emit_progress(&progress_state.snapshot(
        12,
        "生成音视频对齐计划",
        ALIGNMENT_STEP_TIMELINE_PLANNING,
    ))?;
    let mut steps = build_alignment_steps(
        &input,
        video_duration_ms,
        &video_dir,
        &audio_dir,
        input
            .background_music_path
            .as_ref()
            .map(|_| background_dir.as_path()),
    )?;
    progress_state.segments = build_segment_results(&input.segments, &steps);
    progress_state.complete_step(ALIGNMENT_STEP_TIMELINE_PLANNING);

    progress_state.activate_step(ALIGNMENT_STEP_VIDEO_ALIGNMENT, 0);
    emit_progress(&progress_state.snapshot(
        22,
        "生成对齐视频片段",
        ALIGNMENT_STEP_VIDEO_ALIGNMENT,
    ))?;
    let step_count = steps.len();
    for (index, step) in steps.iter_mut().enumerate() {
        create_video_clip(&input.muted_video_path, step)?;
        progress_state.processed_video_clips = index + 1;
        progress_state.set_step(
            ALIGNMENT_STEP_VIDEO_ALIGNMENT,
            (((index + 1) as f64 / step_count as f64) * 100.0).round() as u8,
            "active",
        );
        let progress = 22 + (((index + 1) as f64 / step_count as f64) * 33.0).round() as u8;
        emit_progress(&progress_state.snapshot(
            progress.min(55),
            "生成对齐视频片段",
            ALIGNMENT_STEP_VIDEO_ALIGNMENT,
        ))?;
    }

    let segment_results = build_segment_results(&input.segments, &steps);
    progress_state.segments = segment_results.clone();
    progress_state.complete_step(ALIGNMENT_STEP_VIDEO_ALIGNMENT);
    progress_state.activate_step(ALIGNMENT_STEP_MAIN_AUDIO_REBUILD, 0);
    emit_progress(&progress_state.snapshot(
        58,
        "生成对齐主音频",
        ALIGNMENT_STEP_MAIN_AUDIO_REBUILD,
    ))?;
    for (index, step) in steps.iter().enumerate() {
        create_audio_clip(&input.segments, step)?;
        progress_state.processed_audio_clips = index + 1;
        progress_state.set_step(
            ALIGNMENT_STEP_MAIN_AUDIO_REBUILD,
            (((index + 1) as f64 / steps.len() as f64) * 100.0).round() as u8,
            "active",
        );
        let progress = 58 + (((index + 1) as f64 / steps.len() as f64) * 17.0).round() as u8;
        emit_progress(&progress_state.snapshot(
            progress.min(75),
            "生成对齐主音频",
            ALIGNMENT_STEP_MAIN_AUDIO_REBUILD,
        ))?;
    }
    progress_state.complete_step(ALIGNMENT_STEP_MAIN_AUDIO_REBUILD);

    if let Some(background_music_path) = &input.background_music_path {
        ensure_non_empty_file(background_music_path, "背景音乐文件不存在或为空")?;
        progress_state.activate_step(ALIGNMENT_STEP_BACKGROUND_MUSIC_SYNC, 0);
        emit_progress(&progress_state.snapshot(
            78,
            "同步背景音乐",
            ALIGNMENT_STEP_BACKGROUND_MUSIC_SYNC,
        ))?;
        for (index, step) in steps.iter().enumerate() {
            create_background_clip(background_music_path, step)?;
            progress_state.processed_background_clips = index + 1;
            progress_state.set_step(
                ALIGNMENT_STEP_BACKGROUND_MUSIC_SYNC,
                (((index + 1) as f64 / steps.len() as f64) * 100.0).round() as u8,
                "active",
            );
            let progress = 78 + (((index + 1) as f64 / steps.len() as f64) * 7.0).round() as u8;
            emit_progress(&progress_state.snapshot(
                progress.min(85),
                "同步背景音乐",
                ALIGNMENT_STEP_BACKGROUND_MUSIC_SYNC,
            ))?;
        }
        progress_state.complete_step(ALIGNMENT_STEP_BACKGROUND_MUSIC_SYNC);
    }

    progress_state.activate_step(ALIGNMENT_STEP_OUTPUT_FINALIZE, 0);
    emit_progress(&progress_state.snapshot(88, "拼接对齐产物", ALIGNMENT_STEP_OUTPUT_FINALIZE))?;
    let aligned_video_path = output_dir.join("aligned_muted_video.mp4");
    let aligned_audio_path = output_dir.join("aligned_tts_audio.wav");
    concat_media_files(
        &steps
            .iter()
            .map(|step| step.video_path.clone())
            .collect::<Vec<_>>(),
        &aligned_video_path,
        true,
    )?;
    concat_media_files(
        &steps
            .iter()
            .map(|step| step.audio_path.clone())
            .collect::<Vec<_>>(),
        &aligned_audio_path,
        false,
    )?;
    harmonize_audio_to_video(&aligned_audio_path, &aligned_video_path)?;

    let aligned_background_music_path = if input.background_music_path.is_some() {
        let output_path = output_dir.join("aligned_background_music.wav");
        let background_files = steps
            .iter()
            .filter_map(|step| step.background_path.clone())
            .collect::<Vec<_>>();
        concat_media_files(&background_files, &output_path, false)?;
        harmonize_audio_to_video(&output_path, &aligned_video_path)?;
        Some(output_path)
    } else {
        None
    };

    progress_state.set_step(ALIGNMENT_STEP_OUTPUT_FINALIZE, 62, "active");
    emit_progress(&progress_state.snapshot(94, "生成对齐字幕", ALIGNMENT_STEP_OUTPUT_FINALIZE))?;
    let aligned_subtitle_path = output_dir.join("aligned_subtitle.srt");
    write_aligned_subtitle(&aligned_subtitle_path, &segment_results)?;

    let warnings = segment_results
        .iter()
        .filter_map(|segment| segment.warning.clone())
        .collect::<Vec<_>>();
    let manifest_path = output_dir.join("manifest.json");
    let manifest = json!({
        "alignmentVersion": ALIGNMENT_VERSION,
        "ttsIntervalMs": input.tts_interval_ms,
        "mutedVideoPath": path_to_string(&input.muted_video_path),
        "backgroundMusicPath": input.background_music_path.as_ref().map(|path| path_to_string(path)),
        "alignedVideoPath": path_to_string(&aligned_video_path),
        "alignedAudioPath": path_to_string(&aligned_audio_path),
        "alignedSubtitlePath": path_to_string(&aligned_subtitle_path),
        "alignedBackgroundMusicPath": aligned_background_music_path.as_ref().map(|path| path_to_string(path)),
        "segmentCount": segment_results.len(),
        "processedVideoClips": progress_state.processed_video_clips,
        "processedAudioClips": progress_state.processed_audio_clips,
        "processedBackgroundClips": progress_state.processed_background_clips,
        "backgroundMusicEnabled": progress_state.background_music_enabled,
        "modeCounts": alignment_mode_counts(&segment_results),
        "segments": &segment_results,
        "warnings": &warnings,
    });
    let manifest_text = serde_json::to_string_pretty(&manifest)
        .map_err(|error| format!("无法序列化音视频对齐清单: {error}"))?;
    fs::write(&manifest_path, manifest_text)
        .map_err(|error| format!("无法保存音视频对齐清单: {error}"))?;

    progress_state.segments = segment_results.clone();
    progress_state.complete_step(ALIGNMENT_STEP_OUTPUT_FINALIZE);
    let final_progress =
        progress_state.snapshot(99, "音视频对齐收尾", ALIGNMENT_STEP_OUTPUT_FINALIZE);
    let stage_snapshot = json!({
        "segmentCount": segment_results.len(),
        "alignedVideoPath": path_to_string(&aligned_video_path),
        "alignedAudioPath": path_to_string(&aligned_audio_path),
        "alignedSubtitlePath": path_to_string(&aligned_subtitle_path),
        "alignedBackgroundMusicPath": aligned_background_music_path.as_ref().map(|path| path_to_string(path)),
        "manifestPath": path_to_string(&manifest_path),
        "steps": &final_progress.steps,
        "processedVideoClips": final_progress.processed_video_clips,
        "processedAudioClips": final_progress.processed_audio_clips,
        "processedBackgroundClips": final_progress.processed_background_clips,
        "backgroundMusicEnabled": final_progress.background_music_enabled,
        "modeCounts": &final_progress.mode_counts,
        "segments": &segment_results,
        "warnings": &warnings,
    });

    emit_progress(&final_progress)?;
    Ok(DubbingAlignmentResult {
        aligned_video_path,
        aligned_audio_path,
        aligned_subtitle_path,
        aligned_background_music_path,
        manifest_path,
        manifest,
        stage_snapshot,
        segments: segment_results,
        warnings,
    })
}

fn build_alignment_steps(
    input: &DubbingAlignmentInput,
    video_duration_ms: u64,
    video_dir: &Path,
    audio_dir: &Path,
    background_dir: Option<&Path>,
) -> Result<Vec<AlignmentStep>, String> {
    let mut steps = Vec::new();
    let mut consumed_source_ms = 0_u64;

    for (position, segment) in input.segments.iter().enumerate() {
        if segment.start_time_ms < consumed_source_ms {
            return Err(format!(
                "第 {} 条字幕与前一段对齐区间重叠",
                segment.index + 1
            ));
        }

        if segment.start_time_ms > consumed_source_ms {
            let duration = segment.start_time_ms - consumed_source_ms;
            if duration >= MIN_CLIP_DURATION_MS {
                push_step(
                    &mut steps,
                    AlignmentStepKind::Gap,
                    consumed_source_ms,
                    segment.start_time_ms,
                    duration,
                    1.0,
                    0,
                    "gap",
                    video_dir,
                    audio_dir,
                    background_dir,
                );
            }
        }

        let source_duration_ms = segment.end_time_ms - segment.start_time_ms;
        let pause_duration_ms = if position + 1 < input.segments.len() {
            u64::from(input.tts_interval_ms)
        } else {
            0
        };
        let desired_duration_ms =
            source_duration_ms.max(segment.tts_duration_ms + pause_duration_ms);
        let next_start_ms = input
            .segments
            .get(position + 1)
            .map(|next| next.start_time_ms)
            .unwrap_or(video_duration_ms)
            .min(video_duration_ms);
        let available_end_ms = next_start_ms
            .max(segment.end_time_ms)
            .min(video_duration_ms);
        let available_duration_ms = available_end_ms.saturating_sub(segment.start_time_ms);

        let (source_end_ms, target_duration_ms, pts, freeze_tail_ms, mode) =
            if desired_duration_ms <= source_duration_ms {
                (
                    segment.end_time_ms.min(video_duration_ms),
                    source_duration_ms,
                    1.0,
                    0,
                    "normal",
                )
            } else if desired_duration_ms <= available_duration_ms {
                (
                    segment.start_time_ms + desired_duration_ms,
                    desired_duration_ms,
                    1.0,
                    0,
                    "consume-gap",
                )
            } else {
                let source_end_ms = available_end_ms;
                let source_duration_ms = source_end_ms.saturating_sub(segment.start_time_ms).max(1);
                let required_pts = desired_duration_ms as f64 / source_duration_ms as f64;
                let pts = required_pts.min(MAX_SLOWDOWN_PTS).max(1.0);
                let slowed_duration_ms = ((source_duration_ms as f64) * pts).round() as u64;
                let freeze_tail_ms = desired_duration_ms.saturating_sub(slowed_duration_ms);
                let mode = if freeze_tail_ms > 0 {
                    "freeze-tail"
                } else {
                    "slowdown"
                };
                (
                    source_end_ms,
                    desired_duration_ms,
                    pts,
                    freeze_tail_ms,
                    mode,
                )
            };

        push_step(
            &mut steps,
            AlignmentStepKind::Subtitle(segment.index),
            segment.start_time_ms,
            source_end_ms,
            target_duration_ms,
            pts,
            freeze_tail_ms,
            mode,
            video_dir,
            audio_dir,
            background_dir,
        );
        consumed_source_ms = source_end_ms;
    }

    if video_duration_ms > consumed_source_ms {
        let duration = video_duration_ms - consumed_source_ms;
        if duration >= MIN_CLIP_DURATION_MS {
            push_step(
                &mut steps,
                AlignmentStepKind::Gap,
                consumed_source_ms,
                video_duration_ms,
                duration,
                1.0,
                0,
                "gap",
                video_dir,
                audio_dir,
                background_dir,
            );
        }
    }

    if steps.is_empty() {
        return Err("没有生成有效的音视频对齐片段".to_string());
    }

    Ok(steps)
}

#[allow(clippy::too_many_arguments)]
fn push_step(
    steps: &mut Vec<AlignmentStep>,
    kind: AlignmentStepKind,
    source_start_ms: u64,
    source_end_ms: u64,
    target_duration_ms: u64,
    pts: f64,
    freeze_tail_ms: u64,
    mode: &str,
    video_dir: &Path,
    audio_dir: &Path,
    background_dir: Option<&Path>,
) {
    let index = steps.len();
    steps.push(AlignmentStep {
        kind,
        source_start_ms,
        source_end_ms,
        target_duration_ms,
        real_duration_ms: target_duration_ms,
        pts,
        freeze_tail_ms,
        mode: mode.to_string(),
        video_path: video_dir.join(format!("clip_{index:05}.mp4")),
        audio_path: audio_dir.join(format!("clip_{index:05}.wav")),
        background_path: background_dir.map(|dir| dir.join(format!("clip_{index:05}.wav"))),
    });
}

fn create_video_clip(input_path: &Path, step: &mut AlignmentStep) -> Result<(), String> {
    let source_duration_ms = step.source_end_ms.saturating_sub(step.source_start_ms);
    if source_duration_ms == 0 {
        return Err("视频片段时长为 0，无法生成对齐视频".to_string());
    }

    let mut command = create_command("ffmpeg");
    command
        .arg("-hide_banner")
        .arg("-nostdin")
        .arg("-nostats")
        .arg("-ss")
        .arg(seconds_arg(step.source_start_ms))
        .arg("-t")
        .arg(seconds_arg(source_duration_ms))
        .arg("-i")
        .arg(input_path)
        .arg("-map")
        .arg("0:v:0")
        .arg("-an");
    if (step.pts - 1.0).abs() > 0.01 {
        command
            .arg("-vf")
            .arg(format!("setpts={:.6}*PTS", step.pts));
    }
    command
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("veryfast")
        .arg("-crf")
        .arg("18")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-y")
        .arg(&step.video_path);
    run_command(&mut command, "视频对齐片段生成失败")?;
    ensure_non_empty_file(&step.video_path, "视频对齐片段为空")?;

    if step.freeze_tail_ms > 0 {
        extend_video_tail(&step.video_path, step.freeze_tail_ms)?;
    }

    let mut real_duration_ms = probe_media_duration_ms(&step.video_path)?;
    if real_duration_ms + TOLERANCE_MS < step.target_duration_ms {
        let missing_ms = step.target_duration_ms - real_duration_ms;
        extend_video_tail(&step.video_path, missing_ms)?;
        real_duration_ms = probe_media_duration_ms(&step.video_path)?;
        step.freeze_tail_ms += missing_ms;
    }
    step.real_duration_ms = real_duration_ms.max(1);
    Ok(())
}

fn extend_video_tail(path: &Path, duration_ms: u64) -> Result<(), String> {
    if duration_ms == 0 {
        return Ok(());
    }

    let temp_path = path.with_file_name(format!(
        "{}.tail.mp4",
        path.file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("clip")
    ));
    let mut command = create_command("ffmpeg");
    command
        .arg("-hide_banner")
        .arg("-nostdin")
        .arg("-nostats")
        .arg("-i")
        .arg(path)
        .arg("-vf")
        .arg(format!(
            "tpad=stop_mode=clone:stop_duration={:.3}",
            duration_ms as f64 / 1000.0
        ))
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("veryfast")
        .arg("-crf")
        .arg("18")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-an")
        .arg("-y")
        .arg(&temp_path);
    run_command(&mut command, "视频尾帧补齐失败")?;
    replace_file(&temp_path, path)?;
    Ok(())
}

fn create_audio_clip(
    segments: &[DubbingAlignmentSegmentInput],
    step: &AlignmentStep,
) -> Result<(), String> {
    match step.kind {
        AlignmentStepKind::Gap => create_silent_audio(step.real_duration_ms, &step.audio_path),
        AlignmentStepKind::Subtitle(segment_index) => {
            let segment = segments
                .iter()
                .find(|segment| segment.index == segment_index)
                .ok_or_else(|| format!("找不到第 {} 条字幕", segment_index + 1))?;
            create_tts_audio_block(segment, step.real_duration_ms, &step.audio_path)
        }
    }
}

fn create_silent_audio(duration_ms: u64, output_path: &Path) -> Result<(), String> {
    let mut command = create_command("ffmpeg");
    command
        .arg("-hide_banner")
        .arg("-nostdin")
        .arg("-nostats")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg(format!("anullsrc=r={AUDIO_SAMPLE_RATE}:cl=stereo"))
        .arg("-t")
        .arg(seconds_arg(duration_ms))
        .arg("-acodec")
        .arg("pcm_s16le")
        .arg("-ar")
        .arg(AUDIO_SAMPLE_RATE.to_string())
        .arg("-ac")
        .arg(AUDIO_CHANNELS.to_string())
        .arg("-y")
        .arg(output_path);
    run_command(&mut command, "静音音频片段生成失败")?;
    ensure_non_empty_file(output_path, "静音音频片段为空")
}

fn create_tts_audio_block(
    segment: &DubbingAlignmentSegmentInput,
    duration_ms: u64,
    output_path: &Path,
) -> Result<(), String> {
    let fade_in_s = (TTS_FADE_IN_MS.min(segment.tts_duration_ms / 2)) as f64 / 1000.0;
    let fade_out_ms = TTS_FADE_OUT_MS.min(segment.tts_duration_ms / 2);
    let fade_out_s = fade_out_ms as f64 / 1000.0;
    let fade_out_start_s = segment.tts_duration_ms.saturating_sub(fade_out_ms) as f64 / 1000.0;
    let tts_filter = if fade_out_ms > 0 {
        format!(
            "aformat=sample_rates={AUDIO_SAMPLE_RATE}:channel_layouts=stereo,afade=t=in:d={fade_in_s:.3},afade=t=out:st={fade_out_start_s:.3}:d={fade_out_s:.3}[tts]"
        )
    } else {
        format!("aformat=sample_rates={AUDIO_SAMPLE_RATE}:channel_layouts=stereo[tts]")
    };

    let filter = format!(
        "[0:a]{tts_filter};[1:a][tts]amix=inputs=2:duration=first:dropout_transition=0:normalize=0,alimiter=limit=0.95,atrim=0:{:.3},asetpts=N/SR/TB[aout]",
        duration_ms as f64 / 1000.0
    );
    let mut command = create_command("ffmpeg");
    command
        .arg("-hide_banner")
        .arg("-nostdin")
        .arg("-nostats")
        .arg("-i")
        .arg(&segment.tts_path)
        .arg("-f")
        .arg("lavfi")
        .arg("-t")
        .arg(seconds_arg(duration_ms))
        .arg("-i")
        .arg(format!("anullsrc=r={AUDIO_SAMPLE_RATE}:cl=stereo"))
        .arg("-filter_complex")
        .arg(filter)
        .arg("-map")
        .arg("[aout]")
        .arg("-acodec")
        .arg("pcm_s16le")
        .arg("-ar")
        .arg(AUDIO_SAMPLE_RATE.to_string())
        .arg("-ac")
        .arg(AUDIO_CHANNELS.to_string())
        .arg("-y")
        .arg(output_path);
    run_command(&mut command, "TTS 对齐音频片段生成失败")?;
    ensure_non_empty_file(output_path, "TTS 对齐音频片段为空")
}

fn create_background_clip(
    background_music_path: &Path,
    step: &AlignmentStep,
) -> Result<(), String> {
    let Some(output_path) = &step.background_path else {
        return Ok(());
    };
    let source_duration_ms = step
        .source_end_ms
        .saturating_sub(step.source_start_ms)
        .max(1);
    let target_duration_ms = step.real_duration_ms.max(1);
    let tempo = source_duration_ms as f64 / target_duration_ms as f64;
    let mut filters = build_atempo_filters(tempo);
    filters.push("apad".to_string());
    filters.push(format!("atrim=0:{:.3}", target_duration_ms as f64 / 1000.0));
    filters.push("asetpts=N/SR/TB".to_string());

    let mut command = create_command("ffmpeg");
    command
        .arg("-hide_banner")
        .arg("-nostdin")
        .arg("-nostats")
        .arg("-ss")
        .arg(seconds_arg(step.source_start_ms))
        .arg("-t")
        .arg(seconds_arg(source_duration_ms))
        .arg("-i")
        .arg(background_music_path)
        .arg("-af")
        .arg(filters.join(","))
        .arg("-acodec")
        .arg("pcm_s16le")
        .arg("-ar")
        .arg(AUDIO_SAMPLE_RATE.to_string())
        .arg("-ac")
        .arg(AUDIO_CHANNELS.to_string())
        .arg("-y")
        .arg(output_path);
    run_command(&mut command, "背景音乐对齐片段生成失败")?;
    ensure_non_empty_file(output_path, "背景音乐对齐片段为空")
}

fn build_segment_results(
    segments: &[DubbingAlignmentSegmentInput],
    steps: &[AlignmentStep],
) -> Vec<DubbingAlignmentSegmentResult> {
    let mut results = Vec::new();
    let mut timeline_ms = 0_u64;

    for step in steps {
        match step.kind {
            AlignmentStepKind::Gap => {
                timeline_ms += step.real_duration_ms;
            }
            AlignmentStepKind::Subtitle(segment_index) => {
                let Some(segment) = segments
                    .iter()
                    .find(|segment| segment.index == segment_index)
                else {
                    timeline_ms += step.real_duration_ms;
                    continue;
                };
                let pause_duration_ms = step
                    .real_duration_ms
                    .saturating_sub(segment.tts_duration_ms);
                let warning = if step.real_duration_ms + TOLERANCE_MS < segment.tts_duration_ms {
                    Some(format!(
                        "第 {} 条字幕的视频块短于 TTS 音频，可能发生截断",
                        segment.index + 1
                    ))
                } else {
                    None
                };
                results.push(DubbingAlignmentSegmentResult {
                    index: segment.index,
                    uid: segment.uid.clone(),
                    text: segment.text.clone(),
                    source_start_ms: step.source_start_ms,
                    source_end_ms: step.source_end_ms,
                    tts_duration_ms: segment.tts_duration_ms,
                    pause_duration_ms,
                    aligned_start_ms: timeline_ms,
                    aligned_end_ms: timeline_ms + segment.tts_duration_ms,
                    block_duration_ms: step.real_duration_ms,
                    video_mode: step.mode.clone(),
                    pts: step.pts,
                    freeze_tail_ms: step.freeze_tail_ms,
                    warning,
                });
                timeline_ms += step.real_duration_ms;
            }
        }
    }

    results
}

fn alignment_mode_counts(segments: &[DubbingAlignmentSegmentResult]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for segment in segments {
        *counts.entry(segment.video_mode.clone()).or_insert(0) += 1;
    }
    counts
}

fn concat_media_files(files: &[PathBuf], output_path: &Path, video: bool) -> Result<(), String> {
    if files.is_empty() {
        return Err("没有可拼接的对齐片段".to_string());
    }
    for file in files {
        ensure_non_empty_file(file, "对齐片段不存在或为空")?;
    }

    let concat_path = output_path.with_file_name(format!(
        "{}_concat.txt",
        output_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("alignment")
    ));
    write_concat_file(files, &concat_path)?;

    let mut command = create_command("ffmpeg");
    command
        .arg("-hide_banner")
        .arg("-nostdin")
        .arg("-nostats")
        .arg("-f")
        .arg("concat")
        .arg("-safe")
        .arg("0")
        .arg("-i")
        .arg(&concat_path);
    if video {
        command
            .arg("-c:v")
            .arg("libx264")
            .arg("-preset")
            .arg("veryfast")
            .arg("-crf")
            .arg("18")
            .arg("-pix_fmt")
            .arg("yuv420p")
            .arg("-an");
    } else {
        command
            .arg("-acodec")
            .arg("pcm_s16le")
            .arg("-ar")
            .arg(AUDIO_SAMPLE_RATE.to_string())
            .arg("-ac")
            .arg(AUDIO_CHANNELS.to_string());
    }
    command.arg("-y").arg(output_path);

    let result = run_command(
        &mut command,
        if video {
            "对齐视频拼接失败"
        } else {
            "对齐音频拼接失败"
        },
    );
    let _ = fs::remove_file(&concat_path);
    result?;
    ensure_non_empty_file(output_path, "对齐拼接结果为空")
}

fn harmonize_audio_to_video(audio_path: &Path, video_path: &Path) -> Result<(), String> {
    let audio_duration_ms = probe_media_duration_ms(audio_path)?;
    let video_duration_ms = probe_media_duration_ms(video_path)?;
    let diff_ms = video_duration_ms as i64 - audio_duration_ms as i64;
    if diff_ms.unsigned_abs() <= TOLERANCE_MS {
        return Ok(());
    }

    let temp_path = audio_path.with_file_name(format!(
        "{}.durfix.wav",
        audio_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("audio")
    ));
    let mut command = create_command("ffmpeg");
    command
        .arg("-hide_banner")
        .arg("-nostdin")
        .arg("-nostats")
        .arg("-i")
        .arg(audio_path);

    if diff_ms > 0 {
        command.arg("-af").arg("apad");
    } else if diff_ms.unsigned_abs() > 250 {
        return Err(format!(
            "对齐音频比视频长 {}ms，超过可裁剪范围",
            diff_ms.unsigned_abs()
        ));
    }

    command
        .arg("-t")
        .arg(seconds_arg(video_duration_ms))
        .arg("-acodec")
        .arg("pcm_s16le")
        .arg("-ar")
        .arg(AUDIO_SAMPLE_RATE.to_string())
        .arg("-ac")
        .arg(AUDIO_CHANNELS.to_string())
        .arg("-y")
        .arg(&temp_path);
    run_command(&mut command, "音视频时长纠偏失败")?;
    replace_file(&temp_path, audio_path)?;
    Ok(())
}

fn write_aligned_subtitle(
    output_path: &Path,
    segments: &[DubbingAlignmentSegmentResult],
) -> Result<(), String> {
    let mut text = String::new();
    let mut last_end_ms = 0_u64;
    for (position, segment) in segments.iter().enumerate() {
        let start_ms = segment.aligned_start_ms.max(last_end_ms);
        let end_ms = segment.aligned_end_ms.max(start_ms + 100);
        text.push_str(&(position + 1).to_string());
        text.push('\n');
        text.push_str(&format!(
            "{} --> {}\n",
            format_srt_time(start_ms),
            format_srt_time(end_ms)
        ));
        text.push_str(segment.text.trim());
        text.push_str("\n\n");
        last_end_ms = end_ms;
    }
    fs::write(output_path, text).map_err(|error| format!("无法保存对齐字幕: {error}"))?;
    ensure_non_empty_file(output_path, "对齐字幕为空")
}

fn build_atempo_filters(tempo: f64) -> Vec<String> {
    if !tempo.is_finite() || tempo <= 0.0 || (tempo - 1.0).abs() <= 0.001 {
        return Vec::new();
    }

    let mut filters = Vec::new();
    let mut remaining = tempo;
    while remaining < 0.5 {
        filters.push("atempo=0.5".to_string());
        remaining /= 0.5;
    }
    while remaining > 2.0 {
        filters.push("atempo=2.0".to_string());
        remaining /= 2.0;
    }
    if (remaining - 1.0).abs() > 0.001 {
        filters.push(format!("atempo={remaining:.6}"));
    }
    filters
}

fn write_concat_file(files: &[PathBuf], concat_path: &Path) -> Result<(), String> {
    let mut text = String::new();
    for file in files {
        let path = file
            .canonicalize()
            .unwrap_or_else(|_| file.to_path_buf())
            .to_string_lossy()
            .replace('\\', "/")
            .replace('\'', "\\'");
        text.push_str(&format!("file '{path}'\n"));
    }
    fs::write(concat_path, text).map_err(|error| format!("无法保存拼接列表: {error}"))
}

fn probe_media_duration_ms(path: &Path) -> Result<u64, String> {
    let mut command = create_command("ffprobe");
    command
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(path);
    let output = run_command_with_output(&mut command, "无法获取媒体时长")?;
    let seconds = output
        .trim()
        .parse::<f64>()
        .map_err(|error| format!("无法解析媒体时长: {error}"))?;
    Ok((seconds * 1000.0).round().max(0.0) as u64)
}

fn ensure_non_empty_file(path: &Path, message: &str) -> Result<(), String> {
    let metadata = fs::metadata(path).map_err(|error| format!("{message}: {error}"))?;
    if metadata.len() == 0 {
        Err(message.to_string())
    } else {
        Ok(())
    }
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
        .map_err(|error| format!("{failure_message}: 无法启动进程: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{failure_message}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn run_command_with_output(command: &mut Command, failure_message: &str) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }

    let output = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("{failure_message}: 无法启动进程: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(format!(
            "{failure_message}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn replace_file(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        fs::remove_file(destination).map_err(|error| format!("无法替换文件: {error}"))?;
    }
    fs::rename(source, destination).map_err(|error| format!("无法移动临时文件: {error}"))
}

fn seconds_arg(milliseconds: u64) -> String {
    format!("{:.3}", milliseconds as f64 / 1000.0)
}

fn format_srt_time(milliseconds: u64) -> String {
    let hours = milliseconds / 3_600_000;
    let minutes = (milliseconds % 3_600_000) / 60_000;
    let seconds = (milliseconds % 60_000) / 1000;
    let millis = milliseconds % 1000;
    format!("{hours:02}:{minutes:02}:{seconds:02},{millis:03}")
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
