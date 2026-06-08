use chrono::Local;
use serde_json::{json, Value};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

#[derive(Clone)]
pub struct AppLogger {
    log_dir: PathBuf,
}

#[derive(Clone)]
pub struct LogSession {
    inner: Arc<LogSessionInner>,
}

struct LogSessionInner {
    session_id: String,
    scope: String,
    file_path: PathBuf,
    file: Mutex<File>,
}

impl AppLogger {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let log_dir = app
            .path()
            .app_log_dir()
            .map_err(|error| format!("无法获取日志目录: {error}"))?;

        fs::create_dir_all(&log_dir).map_err(|error| format!("无法创建日志目录: {error}"))?;

        Ok(Self { log_dir })
    }

    pub fn start_session(&self, scope: &str) -> Result<LogSession, String> {
        let now = Local::now();
        let mut session_id = now.format("%Y%m%d%H%M%S").to_string();
        let file_path = self.log_dir.join(format!("{session_id}.log"));
        let file_path = if file_path.exists() {
            session_id = format!("{}{:03}", session_id, now.timestamp_subsec_millis());
            self.log_dir.join(format!("{session_id}.log"))
        } else {
            file_path
        };
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .map_err(|error| format!("无法创建日志文件: {error}"))?;

        let session = LogSession {
            inner: Arc::new(LogSessionInner {
                session_id,
                scope: scope.to_string(),
                file_path,
                file: Mutex::new(file),
            }),
        };

        session.info(
            "session_start",
            "日志会话开始",
            json!({ "logPath": session.path_string() }),
        );

        Ok(session)
    }

    pub fn open_log_directory(&self) -> Result<String, String> {
        fs::create_dir_all(&self.log_dir).map_err(|error| format!("无法创建日志目录: {error}"))?;
        open_path(&self.log_dir)?;
        Ok(self.log_dir.to_string_lossy().to_string())
    }
}

impl LogSession {
    pub fn path_string(&self) -> String {
        self.inner.file_path.to_string_lossy().to_string()
    }

    pub fn info(&self, event: &str, message: &str, fields: Value) {
        let _ = self.write("INFO", event, message, fields);
    }

    pub fn warn(&self, event: &str, message: &str, fields: Value) {
        let _ = self.write("WARN", event, message, fields);
    }

    pub fn error(&self, event: &str, message: &str, fields: Value) {
        let _ = self.write("ERROR", event, message, fields);
    }

    fn write(&self, level: &str, event: &str, message: &str, fields: Value) -> Result<(), String> {
        let record = format_log_record(
            timestamp_millis(),
            level,
            &self.inner.scope,
            &self.inner.session_id,
            event,
            message,
            &fields,
        );

        let mut file = self
            .inner
            .file
            .lock()
            .map_err(|error| format!("日志文件锁定失败: {error}"))?;
        writeln!(file, "{record}").map_err(|error| format!("日志写入失败: {error}"))
    }
}

fn format_log_record(
    timestamp_ms: u128,
    level: &str,
    scope: &str,
    session_id: &str,
    event: &str,
    message: &str,
    fields: &Value,
) -> String {
    let mut record = format!(
        "[{}] [{level}] [{scope}] {event} - {message} | ts={timestamp_ms} session={session_id}",
        Local::now().format("%Y-%m-%d %H:%M:%S%.3f")
    );

    if let Some(fields) = fields.as_object() {
        for (key, value) in fields {
            append_inline_log_field(&mut record, key, value);
        }
    } else if !fields.is_null() {
        append_inline_log_field(&mut record, "fields", fields);
    }

    record
}

fn append_inline_log_field(record: &mut String, key: &str, value: &Value) {
    record.push(' ');
    record.push_str(key);
    record.push('=');
    record.push_str(&format_field_value(value));
}

fn format_field_value(value: &Value) -> String {
    let value = match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    };

    let normalized = value
        .replace("\r\n", "\\n")
        .replace('\r', "\\n")
        .replace('\n', "\\n")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if normalized.contains(' ') || normalized.contains('|') || normalized.contains('=') {
        format!("\"{}\"", normalized.replace('"', "\\\""))
    } else {
        normalized
    }
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn open_path(path: &PathBuf) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("explorer");
        command.arg(path);
        command
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(path);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(path);
        command
    };

    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("无法打开日志目录: {error}"))
}

#[tauri::command]
pub fn open_log_directory(app_logger: tauri::State<'_, AppLogger>) -> Result<String, String> {
    app_logger.open_log_directory()
}
