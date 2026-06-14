use serde::Serialize;
use serde_json::json;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::app_log::{AppLogger, LogSession};
use crate::command_utils::create_command;
use tauri::{AppHandle, Manager};

const YTDLP_COMMAND: &str = "yt-dlp";
const CONFIG_POLICY_IGNORE: &str = "ignoreConfig";
const YTDLP_SOCKET_TIMEOUT_SECONDS: &str = "30";
const YOUTUBE_ACCEPT_LANGUAGE: &str = "Accept-Language: zh-CN,zh;q=0.9,en;q=0.8";

#[derive(Debug, Clone, Copy)]
pub struct YoutubeClientStrategy {
    pub label: &'static str,
    pub extractor_args: &'static str,
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

const YOUTUBE_CLIENT_STRATEGIES: [YoutubeClientStrategy; 4] = [
    YoutubeClientStrategy {
        label: "default",
        extractor_args: "youtube:lang=zh-CN",
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

pub fn youtube_client_strategies() -> &'static [YoutubeClientStrategy] {
    &YOUTUBE_CLIENT_STRATEGIES
}

pub fn add_youtube_extractor_args(command: &mut Command, strategy: &YoutubeClientStrategy) {
    command.args(["--extractor-args", strategy.extractor_args]);
}

pub fn command(config: &YtdlpConfig) -> Command {
    let mut command = create_command(YTDLP_COMMAND);
    command.arg("--ignore-config");

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
                    "yt-dlp 可用 · 忽略全局配置".to_string()
                } else {
                    format!("yt-dlp {version} · 忽略全局配置")
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
    if let Some(log_session) = log_session {
        log_session.warn(
            "client_strategy_failed",
            "yt-dlp 兼容模式失败",
            json!({
                "operation": operation,
                "clientStrategy": strategy.label,
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
    if let Some(log_session) = log_session {
        log_session.info(
            "client_strategy_success",
            "yt-dlp 兼容模式成功",
            json!({
                "operation": operation,
                "clientStrategy": strategy.label,
            }),
        );
    }
}

pub fn format_attempt_errors(fallback: &str, errors: &[(String, String)]) -> String {
    let Some((_, last_error)) = errors.last() else {
        return fallback.to_string();
    };
    let compact = compact_error(last_error);
    let attempted = errors
        .iter()
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>()
        .join("、");

    if attempted.is_empty() {
        compact
    } else {
        format!("{compact}")
    }
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
        || lower.contains("http error")
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
        assert_eq!(strategies[1].label, "web_safari+ios");
        assert_eq!(strategies[2].label, "ios+web");
        assert_eq!(strategies[3].label, "mweb+web");
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
}
