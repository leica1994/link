use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const DATA_DIR_NAME: &str = "data";
const DUBBING_DIR_NAME: &str = "dubbing";
const HTDEMUCS_DIR_NAME: &str = "htdemucs-libtorch";
const LOG_DIR_NAME: &str = "logs";
const MODELS_DIR_NAME: &str = "models";
const RNNOISE_DIR_NAME: &str = "arnndn";
const TEMP_DIR_NAME: &str = "temp";
const WEBVIEW_DIR_NAME: &str = "webview";
const EXPORTS_DIR_NAME: &str = "exports";
const YOUTUBE_TASKS_DIR_NAME: &str = "youtube_tasks";

pub fn app_data_dir() -> Result<PathBuf, String> {
    let install_dir = env::current_exe()
        .map_err(|error| format!("无法获取程序路径: {error}"))?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "无法获取程序安装目录".to_string())?;

    Ok(install_dir.join(DATA_DIR_NAME))
}

pub fn ensure_app_data_dir() -> Result<PathBuf, String> {
    let data_dir = app_data_dir()?;
    fs::create_dir_all(&data_dir).map_err(|error| format!("无法创建应用数据目录: {error}"))?;
    Ok(data_dir)
}

pub fn settings_database_path(file_name: &str) -> Result<PathBuf, String> {
    Ok(ensure_app_data_dir()?.join(file_name))
}

pub fn app_log_dir() -> Result<PathBuf, String> {
    let log_dir = ensure_app_data_dir()?.join(LOG_DIR_NAME);
    fs::create_dir_all(&log_dir).map_err(|error| format!("无法创建日志目录: {error}"))?;
    Ok(log_dir)
}

pub fn dubbing_dir() -> Result<PathBuf, String> {
    let dubbing_dir = ensure_app_data_dir()?.join(DUBBING_DIR_NAME);
    fs::create_dir_all(&dubbing_dir).map_err(|error| format!("无法创建配音素材目录: {error}"))?;
    Ok(dubbing_dir)
}

pub fn temp_dir() -> Result<PathBuf, String> {
    let temp_dir = ensure_app_data_dir()?.join(TEMP_DIR_NAME);
    fs::create_dir_all(&temp_dir).map_err(|error| format!("无法创建临时目录: {error}"))?;
    Ok(temp_dir)
}

pub fn htdemucs_model_dir() -> Result<PathBuf, String> {
    let cache_dir = ensure_app_data_dir()?
        .join(MODELS_DIR_NAME)
        .join(HTDEMUCS_DIR_NAME);
    fs::create_dir_all(&cache_dir)
        .map_err(|error| format!("无法创建 HTDemucs 模型缓存目录: {error}"))?;
    Ok(cache_dir)
}

pub fn rnnoise_model_dir() -> Result<PathBuf, String> {
    let cache_dir = ensure_app_data_dir()?
        .join(MODELS_DIR_NAME)
        .join(RNNOISE_DIR_NAME);
    fs::create_dir_all(&cache_dir)
        .map_err(|error| format!("无法创建 RNNoise 模型缓存目录: {error}"))?;
    Ok(cache_dir)
}

pub fn webview_data_dir() -> Result<PathBuf, String> {
    let webview_dir = ensure_app_data_dir()?.join(WEBVIEW_DIR_NAME);
    fs::create_dir_all(&webview_dir)
        .map_err(|error| format!("无法创建 WebView 缓存目录: {error}"))?;
    Ok(webview_dir)
}

pub fn exports_dir() -> Result<PathBuf, String> {
    let exports_dir = ensure_app_data_dir()?.join(EXPORTS_DIR_NAME);
    fs::create_dir_all(&exports_dir).map_err(|error| format!("无法创建导出目录: {error}"))?;
    Ok(exports_dir)
}

pub fn youtube_task_dir(task_id: &str) -> Result<PathBuf, String> {
    let tasks_dir = ensure_app_data_dir()?.join(YOUTUBE_TASKS_DIR_NAME);
    fs::create_dir_all(&tasks_dir).map_err(|error| format!("无法创建视频任务目录: {error}"))?;
    let task_dir = tasks_dir.join(task_id);
    fs::create_dir_all(&task_dir).map_err(|error| format!("无法创建视频任务目录: {error}"))?;
    Ok(task_dir)
}
