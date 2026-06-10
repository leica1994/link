use reqwest::blocking::Client;
use sha2::{Digest, Sha256};
#[cfg(target_os = "windows")]
use std::ffi::c_void;
use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
#[cfg(target_os = "windows")]
use std::sync::OnceLock;
use std::time::Duration;
use tch::{no_grad, CModule, Cuda, Device, IndexOp, Kind, Tensor};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::app_paths;

pub const MODEL_ID: &str = "htdemucs_libtorch_ft_cuda_v3_context_norm";

const MODEL_FILE_NAME: &str = "htdemucs_ft_cuda.pt";
const SOURCE_MODEL_FILE_NAME: &str = "htdemucs_ft.pt";
const SOURCE_MODEL_SIZE_BYTES: u64 = 168_832_923;
const SOURCE_MODEL_SHA256: &str =
    "2bccb2bccb8310d1369440361f8541eee34a339eb55e436d19e7e0f45978289b";
const MIN_CUDA_MODEL_SIZE_BYTES: u64 = 80 * 1024 * 1024;
const MODEL_URLS: &[&str] = &[
    "https://hf-mirror.com/CrazeDigger/htdemucs/resolve/main/htdemucs_ft.pt",
    "https://huggingface.co/CrazeDigger/htdemucs/resolve/main/htdemucs_ft.pt",
];
const SAMPLE_RATE: u32 = 44_100;
const CHANNELS: u16 = 2;
const WINDOW_SECONDS: usize = 8;
const CONTEXT_SECONDS: usize = 2;
const WINDOW_SAMPLES: usize = SAMPLE_RATE as usize * WINDOW_SECONDS;
const CONTEXT_SAMPLES: usize = SAMPLE_RATE as usize * CONTEXT_SECONDS;
const HOP_SAMPLES: usize = WINDOW_SAMPLES - CONTEXT_SAMPLES * 2;
const STEM_COUNT: usize = 4;
const STEM_NAMES: [&str; STEM_COUNT] = ["drums", "bass", "other", "vocals"];

#[derive(Debug, Clone)]
pub struct StemPaths {
    pub drums_path: PathBuf,
    pub bass_path: PathBuf,
    pub other_path: PathBuf,
    pub vocals_path: PathBuf,
}

#[derive(Debug, Clone)]
pub enum HtDemucsProgress {
    CheckingModel,
    DownloadingModel { downloaded: u64, total: u64 },
    VerifyingModel,
    ModelReady,
    LoadingModel { device: String },
    ReadingAudio,
    Inferencing { percent: f64 },
    Finished,
}

struct StereoAudio {
    left: Vec<f32>,
    right: Vec<f32>,
}

struct StemWriter {
    name: &'static str,
    writer: hound::WavWriter<BufWriter<File>>,
}

pub fn split_file<F>(
    input_path: &Path,
    output_dir: &Path,
    mut progress: F,
) -> Result<StemPaths, String>
where
    F: FnMut(HtDemucsProgress) -> Result<(), String>,
{
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("无法创建 HTDemucs 输出目录: {error}"))?;

    let model_path = ensure_model(&mut progress)?;
    let device = cuda_device()?;
    progress(HtDemucsProgress::LoadingModel {
        device: device_label(device),
    })?;
    let mut model = CModule::load_on_device(&model_path, device).map_err(|error| {
        format!(
            "无法加载 HTDemucs TorchScript 模型: {}",
            compact_torch_error(error)
        )
    })?;
    model.set_eval();

    progress(HtDemucsProgress::ReadingAudio)?;
    let audio = read_stereo_pcm16_wav(input_path)?;
    if audio.left.is_empty() {
        return Err("源音频为空，无法分离背景音乐".to_string());
    }

    let paths = stem_paths(output_dir);
    let mut writers = create_stem_writers(&paths)?;
    let chunk_count = audio.left.len().div_ceil(HOP_SAMPLES);

    progress(HtDemucsProgress::Inferencing { percent: 0.0 })?;
    for chunk_index in 0..chunk_count {
        let valid_start = chunk_index * HOP_SAMPLES;
        let take = (audio.left.len() - valid_start).min(HOP_SAMPLES);
        let input_start = valid_start.saturating_sub(CONTEXT_SAMPLES);
        let output_start = valid_start - input_start;
        let input = window_tensor(&audio, input_start, device);
        let (input, input_mean, input_std) = normalize_input_tensor(input);
        let output = no_grad(|| model.forward_ts(&[input]))
            .map_err(|error| format!("HTDemucs CUDA 推理失败: {}", compact_torch_error(error)))?;
        let output = denormalize_output_tensor(output, &input_mean, &input_std);
        let output = normalize_output_tensor(output, output_start + take)?;
        write_chunk(&mut writers, &output, output_start, take)?;

        let percent = ((chunk_index + 1) as f64 / chunk_count as f64) * 100.0;
        progress(HtDemucsProgress::Inferencing { percent })?;
    }

    for writer in writers {
        writer
            .writer
            .finalize()
            .map_err(|error| format!("无法写入 {} 音轨: {error}", writer.name))?;
    }

    progress(HtDemucsProgress::Finished)?;
    Ok(paths)
}

fn cuda_device() -> Result<Device, String> {
    load_torch_cuda_backend()?;
    if Cuda::is_available() {
        Ok(Device::Cuda(0))
    } else {
        Err(
            "未检测到 CUDA 版 LibTorch，已拒绝使用 CPU 推理。请使用 CUDA 版 LibTorch 构建运行，或清理旧 CPU 构建缓存后重新构建。"
                .to_string(),
        )
    }
}

#[cfg(target_os = "windows")]
fn load_torch_cuda_backend() -> Result<(), String> {
    static TORCH_CUDA: OnceLock<Result<(), String>> = OnceLock::new();
    TORCH_CUDA
        .get_or_init(|| {
            load_library("c10_cuda.dll")?;
            load_library("torch_cuda.dll")
        })
        .clone()
}

#[cfg(not(target_os = "windows"))]
fn load_torch_cuda_backend() -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "windows")]
fn load_library(name: &str) -> Result<(), String> {
    let wide = name
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
    if handle.is_null() {
        Err(format!(
            "无法加载 {name}，请确认 CUDA 版 LibTorch DLL 已复制到程序目录"
        ))
    } else {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(lp_lib_file_name: *const u16) -> *mut c_void;
}

fn ensure_model<F>(progress: &mut F) -> Result<PathBuf, String>
where
    F: FnMut(HtDemucsProgress) -> Result<(), String>,
{
    progress(HtDemucsProgress::CheckingModel)?;
    let model_dir = app_paths::htdemucs_model_dir()?;
    let model_path = model_dir.join(MODEL_FILE_NAME);
    let source_model_path = model_dir.join(SOURCE_MODEL_FILE_NAME);

    if cuda_model_cache_is_valid(&model_path)? {
        progress(HtDemucsProgress::ModelReady)?;
        return Ok(model_path);
    }

    remove_file_if_exists(&model_path, "无法更新 HTDemucs CUDA 模型缓存")?;

    progress(HtDemucsProgress::VerifyingModel)?;
    let mut last_error = String::new();
    if source_model_cache_is_valid(&source_model_path)? {
        match build_cuda_model_from_source(&source_model_path, &model_path) {
            Ok(()) => {
                progress(HtDemucsProgress::ModelReady)?;
                return Ok(model_path);
            }
            Err(error) => {
                last_error = error;
                let _ = fs::remove_file(&model_path);
            }
        }
    } else {
        remove_file_if_exists(&source_model_path, "无法清理无效的 HTDemucs 源模型缓存")?;
    }

    for url in MODEL_URLS {
        match download_model(url, &source_model_path, progress) {
            Ok(()) => {
                progress(HtDemucsProgress::VerifyingModel)?;
                if !source_model_cache_is_valid(&source_model_path)? {
                    last_error = "HTDemucs 源模型校验失败".to_string();
                    let _ = fs::remove_file(&source_model_path);
                    continue;
                }

                match build_cuda_model_from_source(&source_model_path, &model_path) {
                    Ok(()) => {
                        progress(HtDemucsProgress::ModelReady)?;
                        return Ok(model_path);
                    }
                    Err(error) => {
                        last_error = error;
                        let _ = fs::remove_file(&model_path);
                    }
                }
            }
            Err(error) => {
                last_error = error;
                let _ = fs::remove_file(source_model_path.with_extension("part"));
            }
        }
    }

    Err(format!("无法准备 HTDemucs CUDA 模型: {last_error}"))
}

fn source_model_cache_is_valid(path: &Path) -> Result<bool, String> {
    if !path.is_file() {
        return Ok(false);
    }

    let metadata =
        fs::metadata(path).map_err(|error| format!("无法读取 HTDemucs 模型缓存: {error}"))?;
    if metadata.len() != SOURCE_MODEL_SIZE_BYTES {
        return Ok(false);
    }

    Ok(file_sha256_hex(path)? == SOURCE_MODEL_SHA256)
}

fn cuda_model_cache_is_valid(path: &Path) -> Result<bool, String> {
    if !path.is_file() {
        return Ok(false);
    }

    let metadata =
        fs::metadata(path).map_err(|error| format!("无法读取 HTDemucs CUDA 模型缓存: {error}"))?;
    if metadata.len() < MIN_CUDA_MODEL_SIZE_BYTES {
        return Ok(false);
    }

    let file =
        File::open(path).map_err(|error| format!("无法读取 HTDemucs CUDA 模型缓存: {error}"))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| format!("无法读取 HTDemucs CUDA 模型归档: {error}"))?;
    let mut has_model_code = false;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("无法读取 HTDemucs CUDA 模型归档条目: {error}"))?;
        if entry.is_dir() {
            continue;
        }

        let name = entry.name().replace('\\', "/");
        if name.ends_with("code/__torch__/demucs/htdemucs.py") {
            has_model_code = true;
        }
        if should_scan_model_entry(&name) {
            let mut bytes = Vec::new();
            entry
                .read_to_end(&mut bytes)
                .map_err(|error| format!("无法读取 HTDemucs CUDA 模型归档条目: {error}"))?;
            if contains_bytes(&bytes, b"mps") {
                return Ok(false);
            }
        }
    }

    Ok(has_model_code)
}

fn build_cuda_model_from_source(source_path: &Path, model_path: &Path) -> Result<(), String> {
    let temp_path = model_path.with_extension("part");
    remove_file_if_exists(&temp_path, "无法清理 HTDemucs CUDA 模型临时文件")?;

    normalize_torchscript_archive_for_cuda(source_path, &temp_path)?;
    if !cuda_model_cache_is_valid(&temp_path)? {
        let _ = fs::remove_file(&temp_path);
        return Err("HTDemucs CUDA 模型生成后校验失败".to_string());
    }

    remove_file_if_exists(model_path, "无法更新 HTDemucs CUDA 模型缓存")?;
    fs::rename(&temp_path, model_path)
        .map_err(|error| format!("无法更新 HTDemucs CUDA 模型缓存: {error}"))
}

fn normalize_torchscript_archive_for_cuda(
    source_path: &Path,
    output_path: &Path,
) -> Result<(), String> {
    let source_file =
        File::open(source_path).map_err(|error| format!("无法读取 HTDemucs 源模型: {error}"))?;
    let mut source = ZipArchive::new(source_file)
        .map_err(|error| format!("无法读取 HTDemucs 源模型归档: {error}"))?;
    let output_file = File::create(output_path)
        .map_err(|error| format!("无法写入 HTDemucs CUDA 模型缓存: {error}"))?;
    let mut output = ZipWriter::new(output_file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut replacement_count = 0_usize;

    for index in 0..source.len() {
        let mut entry = source
            .by_index(index)
            .map_err(|error| format!("无法读取 HTDemucs 源模型归档条目: {error}"))?;
        let name = entry.name().to_string();

        if entry.is_dir() {
            output
                .add_directory(name, options)
                .map_err(|error| format!("无法写入 HTDemucs CUDA 模型目录条目: {error}"))?;
            continue;
        }

        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|error| format!("无法读取 HTDemucs 源模型归档条目: {error}"))?;

        if name.ends_with(".py") {
            let (patched, count) = patch_torchscript_source(&bytes)?;
            bytes = patched;
            replacement_count += count;
        } else if name.ends_with(".pkl") || name.ends_with(".debug_pkl") {
            let (patched, count) = patch_pickle_devices(&bytes);
            bytes = patched;
            replacement_count += count;
        }

        output
            .start_file(name, options)
            .map_err(|error| format!("无法写入 HTDemucs CUDA 模型归档条目: {error}"))?;
        output
            .write_all(&bytes)
            .map_err(|error| format!("无法写入 HTDemucs CUDA 模型归档数据: {error}"))?;
    }

    output
        .finish()
        .map_err(|error| format!("无法完成 HTDemucs CUDA 模型归档写入: {error}"))?;

    if replacement_count == 0 {
        return Err("HTDemucs 源模型中未找到需要转换的 MPS 设备标记".to_string());
    }

    Ok(())
}

fn patch_torchscript_source(bytes: &[u8]) -> Result<(Vec<u8>, usize), String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| format!("HTDemucs TorchScript 源码不是有效 UTF-8: {error}"))?;
    let replacement_count = text.matches("torch.device(\"mps:0\")").count()
        + text.matches("torch.device(\"mps\")").count();
    let text = text
        .replace("torch.device(\"mps:0\")", "torch.device(\"cuda\")")
        .replace("torch.device(\"mps\")", "torch.device(\"cuda\")");

    Ok((text.into_bytes(), replacement_count))
}

fn patch_pickle_devices(bytes: &[u8]) -> (Vec<u8>, usize) {
    let replacements: &[(&[u8], &[u8])] = &[
        (
            &[0x58, 0x05, 0x00, 0x00, 0x00, b'm', b'p', b's', b':', b'0'],
            &[0x58, 0x04, 0x00, 0x00, 0x00, b'c', b'u', b'd', b'a'],
        ),
        (
            &[0x58, 0x03, 0x00, 0x00, 0x00, b'm', b'p', b's'],
            &[0x58, 0x04, 0x00, 0x00, 0x00, b'c', b'u', b'd', b'a'],
        ),
        (
            &[0x8c, 0x05, b'm', b'p', b's', b':', b'0'],
            &[0x8c, 0x04, b'c', b'u', b'd', b'a'],
        ),
        (
            &[0x8c, 0x03, b'm', b'p', b's'],
            &[0x8c, 0x04, b'c', b'u', b'd', b'a'],
        ),
    ];
    let mut output = bytes.to_vec();
    let mut replacement_count = 0_usize;

    for (from, to) in replacements {
        let (patched, count) = replace_all_bytes(&output, from, to);
        output = patched;
        replacement_count += count;
    }

    (output, replacement_count)
}

fn replace_all_bytes(input: &[u8], from: &[u8], to: &[u8]) -> (Vec<u8>, usize) {
    let mut output = Vec::with_capacity(input.len());
    let mut index = 0_usize;
    let mut replacement_count = 0_usize;

    while index < input.len() {
        if index + from.len() <= input.len() && &input[index..index + from.len()] == from {
            output.extend_from_slice(to);
            index += from.len();
            replacement_count += 1;
        } else {
            output.push(input[index]);
            index += 1;
        }
    }

    (output, replacement_count)
}

fn should_scan_model_entry(name: &str) -> bool {
    name.ends_with(".py") || name.ends_with(".pkl") || name.ends_with(".debug_pkl")
}

fn contains_bytes(bytes: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && bytes.windows(needle.len()).any(|window| window == needle)
}

fn remove_file_if_exists(path: &Path, context: &str) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("{context}: {error}")),
    }
}

fn download_model<F>(url: &str, output_path: &Path, progress: &mut F) -> Result<(), String>
where
    F: FnMut(HtDemucsProgress) -> Result<(), String>,
{
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("无法创建 HTDemucs 模型目录: {error}"))?;
    }

    let temp_path = output_path.with_extension("part");
    if temp_path.exists() {
        fs::remove_file(&temp_path)
            .map_err(|error| format!("无法清理 HTDemucs 模型临时文件: {error}"))?;
    }

    let client = Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(60 * 60))
        .build()
        .map_err(|error| format!("无法创建 HTDemucs 模型下载客户端: {error}"))?;
    let mut response = client
        .get(url)
        .send()
        .map_err(|error| format!("HTDemucs 模型下载失败: {error}"))?
        .error_for_status()
        .map_err(|error| format!("HTDemucs 模型下载失败: {error}"))?;
    let total = response.content_length().unwrap_or(SOURCE_MODEL_SIZE_BYTES);
    let mut file =
        File::create(&temp_path).map_err(|error| format!("无法写入 HTDemucs 模型缓存: {error}"))?;
    let mut downloaded = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];

    progress(HtDemucsProgress::DownloadingModel { downloaded, total })?;
    loop {
        let bytes_read = response
            .read(&mut buffer)
            .map_err(|error| format!("无法读取 HTDemucs 模型下载数据: {error}"))?;
        if bytes_read == 0 {
            break;
        }
        file.write_all(&buffer[..bytes_read])
            .map_err(|error| format!("无法保存 HTDemucs 模型缓存: {error}"))?;
        downloaded += bytes_read as u64;
        progress(HtDemucsProgress::DownloadingModel { downloaded, total })?;
    }
    file.flush()
        .map_err(|error| format!("无法保存 HTDemucs 模型缓存: {error}"))?;

    if downloaded != SOURCE_MODEL_SIZE_BYTES {
        let _ = fs::remove_file(&temp_path);
        return Err(format!(
            "HTDemucs 模型下载不完整，期望 {} 字节，实际 {} 字节",
            SOURCE_MODEL_SIZE_BYTES, downloaded
        ));
    }

    fs::rename(&temp_path, output_path)
        .map_err(|error| format!("无法更新 HTDemucs 模型缓存: {error}"))
}

fn file_sha256_hex(path: &Path) -> Result<String, String> {
    let mut file =
        File::open(path).map_err(|error| format!("无法读取 HTDemucs 模型缓存: {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let bytes_read = file
            .read(&mut buffer)
            .map_err(|error| format!("无法校验 HTDemucs 模型缓存: {error}"))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn read_stereo_pcm16_wav(path: &Path) -> Result<StereoAudio, String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|error| format!("无法读取源音频: {error}"))?;
    let spec = reader.spec();
    if spec.sample_rate != SAMPLE_RATE {
        return Err(format!(
            "源音频采样率必须为 {} Hz，实际为 {} Hz",
            SAMPLE_RATE, spec.sample_rate
        ));
    }
    if spec.channels != CHANNELS {
        return Err(format!(
            "源音频必须为 {} 声道，实际为 {} 声道",
            CHANNELS, spec.channels
        ));
    }
    if spec.sample_format != hound::SampleFormat::Int || spec.bits_per_sample != 16 {
        return Err("源音频必须为 16-bit PCM WAV".to_string());
    }

    let samples = reader
        .samples::<i16>()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法读取源音频采样: {error}"))?;
    if samples.len() % CHANNELS as usize != 0 {
        return Err("源音频采样数量异常".to_string());
    }

    let frame_count = samples.len() / CHANNELS as usize;
    let mut left = Vec::with_capacity(frame_count);
    let mut right = Vec::with_capacity(frame_count);
    for frame in samples.chunks_exact(CHANNELS as usize) {
        left.push(pcm16_to_f32(frame[0]));
        right.push(pcm16_to_f32(frame[1]));
    }

    Ok(StereoAudio { left, right })
}

fn stem_paths(output_dir: &Path) -> StemPaths {
    StemPaths {
        drums_path: output_dir.join("drums.wav"),
        bass_path: output_dir.join("bass.wav"),
        other_path: output_dir.join("other.wav"),
        vocals_path: output_dir.join("vocals.wav"),
    }
}

fn create_stem_writers(paths: &StemPaths) -> Result<Vec<StemWriter>, String> {
    let spec = hound::WavSpec {
        channels: CHANNELS,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let entries = [
        ("drums", &paths.drums_path),
        ("bass", &paths.bass_path),
        ("other", &paths.other_path),
        ("vocals", &paths.vocals_path),
    ];
    let mut writers = Vec::with_capacity(entries.len());

    for (name, path) in entries {
        let writer = hound::WavWriter::create(path, spec)
            .map_err(|error| format!("无法创建 {name} 音轨: {error}"))?;
        writers.push(StemWriter { name, writer });
    }

    Ok(writers)
}

fn window_tensor(audio: &StereoAudio, start: usize, device: Device) -> Tensor {
    let mut samples = vec![0.0_f32; CHANNELS as usize * WINDOW_SAMPLES];
    let take = audio.left.len().saturating_sub(start).min(WINDOW_SAMPLES);

    for index in 0..take {
        samples[index] = audio.left[start + index];
        samples[WINDOW_SAMPLES + index] = audio.right[start + index];
    }

    Tensor::from_slice(&samples)
        .reshape([1, CHANNELS as i64, WINDOW_SAMPLES as i64])
        .to_device(device)
}

fn normalize_input_tensor(input: Tensor) -> (Tensor, Tensor, Tensor) {
    let mono = input.mean_dim(1i64, true, Kind::Float);
    let mean = mono.mean_dim(2i64, true, Kind::Float);
    let std = mono.std_dim(2i64, false, true);
    let normalized = (&input - &mean) / (&std + 1e-5);

    (normalized, mean, std)
}

fn denormalize_output_tensor(output: Tensor, mean: &Tensor, std: &Tensor) -> Tensor {
    output * std + mean
}

fn normalize_output_tensor(output: Tensor, required_frames: usize) -> Result<Tensor, String> {
    let output = output.to_device(Device::Cpu).to_kind(Kind::Float);
    let shape = output.size();
    let output = match shape.as_slice() {
        [1, sources, channels, frames] => {
            validate_output_shape(*sources, *channels, *frames, required_frames)?;
            output.i(0)
        }
        [sources, channels, frames] => {
            validate_output_shape(*sources, *channels, *frames, required_frames)?;
            output
        }
        _ => {
            return Err(format!(
                "HTDemucs 输出形状异常，期望 [1, stems, 2, samples]，实际 {:?}",
                shape
            ));
        }
    };

    Ok(output)
}

fn validate_output_shape(
    sources: i64,
    channels: i64,
    frames: i64,
    required_frames: usize,
) -> Result<(), String> {
    if sources < STEM_COUNT as i64 {
        return Err(format!("HTDemucs 输出音轨数量不足: {sources}"));
    }
    if channels != CHANNELS as i64 {
        return Err(format!("HTDemucs 输出声道数量异常: {channels}"));
    }
    if frames < required_frames as i64 {
        return Err(format!(
            "HTDemucs 输出长度不足，期望至少 {}，实际 {}",
            required_frames, frames
        ));
    }

    Ok(())
}

fn write_chunk(
    writers: &mut [StemWriter],
    output: &Tensor,
    output_start: usize,
    take: usize,
) -> Result<(), String> {
    for source_index in 0..STEM_COUNT {
        let mut left = vec![0.0_f32; take];
        let mut right = vec![0.0_f32; take];
        output
            .i((source_index as i64, 0))
            .narrow(0, output_start as i64, take as i64)
            .contiguous()
            .copy_data(&mut left, take);
        output
            .i((source_index as i64, 1))
            .narrow(0, output_start as i64, take as i64)
            .contiguous()
            .copy_data(&mut right, take);

        let writer = writers
            .get_mut(source_index)
            .ok_or_else(|| "HTDemucs 输出音轨数量异常".to_string())?;
        for sample_index in 0..take {
            writer
                .writer
                .write_sample(f32_to_pcm16(left[sample_index]))
                .map_err(|error| {
                    format!("无法写入 {} 左声道: {error}", STEM_NAMES[source_index])
                })?;
            writer
                .writer
                .write_sample(f32_to_pcm16(right[sample_index]))
                .map_err(|error| {
                    format!("无法写入 {} 右声道: {error}", STEM_NAMES[source_index])
                })?;
        }
    }

    Ok(())
}

fn pcm16_to_f32(sample: i16) -> f32 {
    sample as f32 / i16::MAX as f32
}

fn f32_to_pcm16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
}

fn compact_torch_error(error: impl std::fmt::Display) -> String {
    const MAX_LEN: usize = 360;
    let text = error.to_string().replace('\r', " ").replace('\n', " ");
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= MAX_LEN {
        return compact;
    }

    format!("{}...", compact.chars().take(MAX_LEN).collect::<String>())
}

fn device_label(device: Device) -> String {
    match device {
        Device::Cpu => "CPU".to_string(),
        Device::Cuda(index) => format!("CUDA:{index}"),
        Device::Mps => "MPS".to_string(),
        Device::Vulkan => "Vulkan".to_string(),
    }
}
