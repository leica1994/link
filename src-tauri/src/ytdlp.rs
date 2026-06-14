use serde::Serialize;
use serde_json::json;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::app_log::{AppLogger, LogSession};
use crate::app_paths;
use crate::command_utils::create_command;
use tauri::{AppHandle, Manager};

const YTDLP_COMMAND: &str = "yt-dlp";
const CONFIG_POLICY_IGNORE: &str = "ignoreConfig";
const YTDLP_SOCKET_TIMEOUT_SECONDS: &str = "30";
const YOUTUBE_ACCEPT_LANGUAGE: &str = "Accept-Language: zh-CN,zh;q=0.9,en;q=0.8";
const YTDLP_MERGE_OUTPUT_FORMATS: &str = "mp4/mkv";

#[derive(Debug, Clone, Copy)]
pub struct YoutubeClientStrategy {
    pub label: &'static str,
    pub extractor_args: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct YoutubeVideoFormatStrategy {
    pub label: &'static str,
    pub selector: Option<&'static str>,
    pub check_formats: bool,
    pub merge_output_format: Option<&'static str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YtdlpStatus {
    pub is_available: bool,
    pub version: String,
    pub message: String,
    pub resolved_path: String,
    pub config_policy: String,
}

#[derive(Debug, Clone, Default)]
pub struct YtdlpConfig {
    pub proxy: String,
    pub cookies_path: String,
}

impl YtdlpConfig {
    pub fn new(proxy: impl Into<String>, cookies_path: impl Into<String>) -> Self {
        Self {
            proxy: proxy.into(),
            cookies_path: cookies_path.into(),
        }
    }
}

const YOUTUBE_CLIENT_STRATEGIES: [YoutubeClientStrategy; 5] = [
    YoutubeClientStrategy {
        label: "default",
        extractor_args: "youtube:lang=zh-CN",
    },
    YoutubeClientStrategy {
        label: "web+ios",
        extractor_args: "youtube:lang=zh-CN;player-client=web,ios",
    },
    YoutubeClientStrategy {
        label: "web_safari+ios",
        extractor_args: "youtube:lang=zh-CN;player-client=web_safari,ios",
    },
    YoutubeClientStrategy {
        label: "ios+web",
        extractor_args: "youtube:lang=zh-CN;player-client=ios,web",
    },
    YoutubeClientStrategy {
        label: "mweb+web",
        extractor_args: "youtube:lang=zh-CN;player-client=mweb,web",
    },
];

const YOUTUBE_VIDEO_FORMAT_STRATEGIES: [YoutubeVideoFormatStrategy; 4] = [
    YoutubeVideoFormatStrategy {
        label: "preferred_mp4",
        selector: Some("bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]/best[ext=mp4]"),
        check_formats: true,
        merge_output_format: Some(YTDLP_MERGE_OUTPUT_FORMATS),
    },
    YoutubeVideoFormatStrategy {
        label: "flexible_best",
        selector: Some("bv*+ba/bestvideo*+bestaudio/best"),
        check_formats: false,
        merge_output_format: Some(YTDLP_MERGE_OUTPUT_FORMATS),
    },
    YoutubeVideoFormatStrategy {
        label: "single_best",
        selector: Some("best/b"),
        check_formats: false,
        merge_output_format: Some(YTDLP_MERGE_OUTPUT_FORMATS),
    },
    YoutubeVideoFormatStrategy {
        label: "yt_dlp_default",
        selector: None,
        check_formats: false,
        merge_output_format: Some(YTDLP_MERGE_OUTPUT_FORMATS),
    },
];

pub fn youtube_client_strategies() -> &'static [YoutubeClientStrategy] {
    &YOUTUBE_CLIENT_STRATEGIES
}

pub fn youtube_video_format_strategies() -> &'static [YoutubeVideoFormatStrategy] {
    &YOUTUBE_VIDEO_FORMAT_STRATEGIES
}

pub fn add_youtube_extractor_args(command: &mut Command, strategy: &YoutubeClientStrategy) {
    command.args(["--extractor-args", strategy.extractor_args]);
}

pub fn add_youtube_video_format_args(command: &mut Command, strategy: &YoutubeVideoFormatStrategy) {
    if strategy.check_formats {
        command.arg("--check-formats");
    }
    if let Some(selector) = strategy.selector {
        command.args(["-f", selector]);
    }
    if let Some(format) = strategy.merge_output_format {
        command.args(["--merge-output-format", format]);
    }
}

pub fn command(config: &YtdlpConfig) -> Command {
    let mut command = create_command(YTDLP_COMMAND);
    command.arg("--ignore-config");

    if let Ok(cache_dir) = app_paths::ytdlp_dir() {
        command.arg("--cache-dir").arg(cache_dir);
    }

    let proxy = config.proxy.trim();
    if !proxy.is_empty() {
        command.args(["--proxy", proxy]);
    }

    let cookies_path = config.cookies_path.trim();
    if !cookies_path.is_empty() && Path::new(cookies_path).is_file() {
        command.args(["--cookies", cookies_path]);
    }

    command.args([
        "--js-runtimes",
        "node",
        "--js-runtimes",
        "bun",
        "--js-runtimes",
        "quickjs",
        "--add-headers",
        YOUTUBE_ACCEPT_LANGUAGE,
        "--socket-timeout",
        YTDLP_SOCKET_TIMEOUT_SECONDS,
        "--extractor-retries",
        "3",
        "--retries",
        "10",
        "--file-access-retries",
        "3",
        "--fragment-retries",
        "10",
    ]);

    command
}

pub fn status() -> YtdlpStatus {
    let resolved_path = resolve_command_path(YTDLP_COMMAND)
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut cmd = create_command(YTDLP_COMMAND);

    match cmd.arg("--version").output() {
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
                resolved_path,
                config_policy: CONFIG_POLICY_IGNORE.to_string(),
            }
        }
        Ok(output) => YtdlpStatus {
            is_available: false,
            version: String::new(),
            message: stderr_or_default(&output.stderr, "yt-dlp 检测失败"),
            resolved_path,
            config_policy: CONFIG_POLICY_IGNORE.to_string(),
        },
        Err(error) => YtdlpStatus {
            is_available: false,
            version: String::new(),
            message: format!("未检测到 yt-dlp: {error}"),
            resolved_path,
            config_policy: CONFIG_POLICY_IGNORE.to_string(),
        },
    }
}

pub fn start_log_session(app: &AppHandle, operation: &str) -> Option<LogSession> {
    let logger = app.state::<AppLogger>();
    let session = logger.start_session("yt_dlp").ok()?;
    session.info(
        "operation_start",
        "yt-dlp 调用开始",
        json!({
            "operation": operation,
            "configPolicy": CONFIG_POLICY_IGNORE,
        }),
    );
    Some(session)
}

pub fn log_attempt_failure(
    log_session: Option<&LogSession>,
    operation: &str,
    strategy: &YoutubeClientStrategy,
    error: &str,
) {
    log_attempt_failure_with_profile(log_session, operation, strategy, None, error);
}

pub fn log_attempt_failure_with_profile(
    log_session: Option<&LogSession>,
    operation: &str,
    strategy: &YoutubeClientStrategy,
    profile_label: Option<&str>,
    error: &str,
) {
    if let Some(log_session) = log_session {
        log_session.warn(
            "client_strategy_failed",
            "yt-dlp 兼容模式失败",
            json!({
                "operation": operation,
                "clientStrategy": strategy.label,
                "profile": profile_label.unwrap_or(""),
                "error": compact_error(error),
            }),
        );
    }
}

pub fn log_attempt_success(
    log_session: Option<&LogSession>,
    operation: &str,
    strategy: &YoutubeClientStrategy,
) {
    log_attempt_success_with_profile(log_session, operation, strategy, None);
}

pub fn log_attempt_success_with_profile(
    log_session: Option<&LogSession>,
    operation: &str,
    strategy: &YoutubeClientStrategy,
    profile_label: Option<&str>,
) {
    if let Some(log_session) = log_session {
        log_session.info(
            "client_strategy_success",
            "yt-dlp 兼容模式成功",
            json!({
                "operation": operation,
                "clientStrategy": strategy.label,
                "profile": profile_label.unwrap_or(""),
            }),
        );
    }
}

pub fn run_with_youtube_client_fallback<T, F>(
    operation: &str,
    failure_message: &str,
    log_session: Option<&LogSession>,
    mut run_attempt: F,
) -> Result<T, String>
where
    F: FnMut(&YoutubeClientStrategy) -> Result<T, String>,
{
    let mut errors = Vec::new();
    for strategy in youtube_client_strategies() {
        match run_attempt(strategy) {
            Ok(value) => {
                log_attempt_success(log_session, operation, strategy);
                return Ok(value);
            }
            Err(error) => {
                log_attempt_failure(log_session, operation, strategy, &error);
                errors.push((strategy.label.to_string(), error));
            }
        }
    }

    Err(format_attempt_errors(failure_message, &errors))
}

pub fn format_attempt_errors(fallback: &str, errors: &[(String, String)]) -> String {
    let Some(selected_error) = select_attempt_error(errors) else {
        return fallback.to_string();
    };
    let compact = compact_error(selected_error);
    let attempted = errors
        .iter()
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>()
        .join("、");

    if attempted.is_empty() {
        compact
    } else if errors.len() == 1 {
        format!("{compact}（已尝试：{attempted}）")
    } else {
        format!("{compact}（已尝试 {} 种兼容模式）", errors.len())
    }
}

fn select_attempt_error(errors: &[(String, String)]) -> Option<&str> {
    errors
        .iter()
        .min_by_key(|(_, error)| attempt_error_priority(error))
        .map(|(_, error)| error.as_str())
}

fn attempt_error_priority(error: &str) -> u8 {
    let lower = error.to_ascii_lowercase();
    if lower.contains("sign in")
        || lower.contains("not a bot")
        || lower.contains("cookies")
        || lower.contains("cookie")
        || lower.contains("forbidden")
        || lower.contains("http error 403")
        || lower.contains("http error 429")
    {
        return 0;
    }

    if lower.contains("proxy")
        || lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("connection")
        || lower.contains("network")
    {
        return 1;
    }

    if lower.contains("requested format is not available") {
        return 3;
    }

    2
}

pub fn stderr_or_default(stderr: &[u8], fallback: &str) -> String {
    let message = String::from_utf8_lossy(stderr).trim().to_string();
    if message.is_empty() {
        fallback.to_string()
    } else {
        compact_error(&message)
    }
}

pub fn lines_or_default(lines: &[String], fallback: &str) -> String {
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

pub fn compact_error(error: &str) -> String {
    let lines = error
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    if lines.iter().any(|line| {
        let lower = line.to_ascii_lowercase();
        lower.contains("sign in") && lower.contains("not a bot")
    }) {
        return "YouTube 要求登录或人机验证，请更新 Cookies 后重试".to_string();
    }

    if lines.iter().any(|line| {
        line.to_ascii_lowercase()
            .contains("requested format is not available")
    }) {
        return "当前视频没有返回可用下载格式，请确认 yt-dlp 已更新，并检查 Cookies/代理是否能访问该视频".to_string();
    }

    if lines.iter().any(|line| {
        line.to_ascii_lowercase()
            .contains("no player clients have been requested")
    }) {
        return "yt-dlp 未请求可用的 YouTube 播放客户端".to_string();
    }

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
        || lower.contains("timeout")
        || lower.contains("http error")
        || lower.contains("forbidden")
        || lower.contains("sign in")
        || lower.contains("not a bot")
        || lower.contains("requested format is not available")
        || lower.contains("no player clients have been requested")
}

fn resolve_command_path(command_name: &str) -> Option<PathBuf> {
    let command_path = Path::new(command_name);
    if command_path.is_absolute() || command_name.contains(std::path::MAIN_SEPARATOR) {
        return command_path.is_file().then(|| command_path.to_path_buf());
    }

    let path_env = env::var_os("PATH")?;
    let candidates = executable_candidates(command_name);
    for dir in env::split_paths(&path_env) {
        for candidate in &candidates {
            let path = dir.join(candidate);
            if path.is_file() {
                return Some(path);
            }
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn executable_candidates(command_name: &str) -> Vec<String> {
    if Path::new(command_name).extension().is_some() {
        return vec![command_name.to_string()];
    }

    let pathext = env::var_os("PATHEXT")
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".to_string());
    pathext
        .split(';')
        .map(str::trim)
        .filter(|ext| !ext.is_empty())
        .map(|ext| format!("{command_name}{ext}"))
        .chain(std::iter::once(command_name.to_string()))
        .collect()
}

#[cfg(not(target_os = "windows"))]
fn executable_candidates(command_name: &str) -> Vec<String> {
    vec![command_name.to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command_args(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(|value| value.to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn base_command_ignores_global_config() {
        let command = command(&YtdlpConfig::default());
        let args = command_args(&command);

        assert!(args.iter().any(|arg| arg == "--ignore-config"));
        assert!(args.iter().any(|arg| arg == "--cache-dir"));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--js-runtimes" && pair[1] == "node"));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--js-runtimes" && pair[1] == "bun"));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--js-runtimes" && pair[1] == "quickjs"));
        assert!(args.iter().any(|arg| arg == "--socket-timeout"));
        assert!(args.iter().any(|arg| arg == "--extractor-retries"));
    }

    #[test]
    fn base_command_adds_proxy_when_configured() {
        let command = command(&YtdlpConfig::new("http://127.0.0.1:7890", ""));
        let args = command_args(&command);

        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--proxy" && pair[1] == "http://127.0.0.1:7890"));
    }

    #[test]
    fn base_command_adds_cookies_when_cached_file_exists() {
        let cookies_file = tempfile::NamedTempFile::new().expect("create cookies file");
        let cookies_path = cookies_file.path().to_string_lossy().to_string();
        let command = command(&YtdlpConfig::new("", &cookies_path));
        let args = command_args(&command);

        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--cookies" && pair[1] == cookies_path.as_str()));
    }

    #[test]
    fn base_command_skips_missing_cookies_file() {
        let command = command(&YtdlpConfig::new("", "missing-cookies.txt"));
        let args = command_args(&command);

        assert!(!args.iter().any(|arg| arg == "--cookies"));
    }

    #[test]
    fn youtube_client_strategies_are_explicit_and_ordered() {
        let strategies = youtube_client_strategies();

        assert_eq!(strategies[0].label, "default");
        assert_eq!(strategies[1].label, "web+ios");
        assert_eq!(strategies[2].label, "web_safari+ios");
        assert_eq!(strategies[3].label, "ios+web");
        assert_eq!(strategies[4].label, "mweb+web");
        assert!(strategies
            .iter()
            .skip(1)
            .all(|strategy| strategy.extractor_args.contains("player-client=")));
        assert!(strategies
            .iter()
            .all(|strategy| !strategy.extractor_args.contains("default,-android_vr")));
    }

    #[test]
    fn compact_error_handles_empty_player_clients() {
        let error = "ERROR: [youtube] BGBhHd-Pvxmw: No player clients have been requested";

        assert_eq!(
            compact_error(error),
            "yt-dlp 未请求可用的 YouTube 播放客户端"
        );
    }

    #[test]
    fn youtube_video_format_strategies_fall_back_to_unfiltered_default() {
        let strategies = youtube_video_format_strategies();

        assert_eq!(strategies[0].label, "preferred_mp4");
        assert!(strategies[0].check_formats);
        assert!(strategies[0].selector.is_some());
        assert_eq!(strategies.last().unwrap().label, "yt_dlp_default");
        assert!(strategies.last().unwrap().selector.is_none());
    }

    #[test]
    fn add_video_format_args_skips_selector_for_default_profile() {
        let mut command = create_command("yt-dlp");
        add_youtube_video_format_args(
            &mut command,
            youtube_video_format_strategies().last().unwrap(),
        );
        let args = command_args(&command);

        assert!(!args.iter().any(|arg| arg == "-f"));
        assert!(args.iter().any(|arg| arg == "--merge-output-format"));
    }

    #[test]
    fn format_attempt_errors_prefers_auth_errors_over_format_errors() {
        let errors = vec![
            (
                "default / preferred_mp4".to_string(),
                "ERROR: [youtube] abc: Sign in to confirm you are not a bot".to_string(),
            ),
            (
                "mweb+web / yt_dlp_default".to_string(),
                "ERROR: [youtube] abc: Requested format is not available".to_string(),
            ),
        ];

        assert!(format_attempt_errors("failed", &errors).starts_with("YouTube 要求登录或人机验证"));
    }
}
