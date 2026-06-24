use chrono::Local;
use serde_json::{json, Value};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;

use crate::app_paths;
use crate::command_utils::create_command;

#[derive(Clone)]
pub struct AppLogger {
    inner: Arc<AppLoggerInner>,
}

#[derive(Clone)]
pub struct LogSession {
    inner: Arc<LogSessionInner>,
}

struct AppLoggerInner {
    log_dir: PathBuf,
    writer: Mutex<DailyLogWriter>,
    session_sequence: AtomicU64,
}

struct LogSessionInner {
    session_id: String,
    scope: String,
    logger: Arc<AppLoggerInner>,
}

struct DailyLogWriter {
    date: String,
    file: File,
}

impl AppLogger {
    pub fn new(_app: &AppHandle) -> Result<Self, String> {
        let log_dir = app_paths::app_log_dir()?;
        Self::new_with_log_dir(log_dir)
    }

    fn new_with_log_dir(log_dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&log_dir).map_err(|error| format!("无法创建日志目录: {error}"))?;
        let writer = DailyLogWriter::open(&log_dir, &current_log_date())?;
        Ok(Self {
            inner: Arc::new(AppLoggerInner {
                log_dir,
                writer: Mutex::new(writer),
                session_sequence: AtomicU64::new(0),
            }),
        })
    }

    pub fn start_session(&self, scope: &str) -> Result<LogSession, String> {
        let now = Local::now();
        let sequence = self
            .inner
            .session_sequence
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        let session_id = format!(
            "{}{:03}-{:06}",
            now.format("%Y%m%d%H%M%S"),
            now.timestamp_subsec_millis(),
            sequence
        );

        let session = LogSession {
            inner: Arc::new(LogSessionInner {
                session_id,
                scope: scope.to_string(),
                logger: Arc::clone(&self.inner),
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
        fs::create_dir_all(&self.inner.log_dir)
            .map_err(|error| format!("无法创建日志目录: {error}"))?;
        open_path(&self.inner.log_dir)?;
        Ok(self.inner.log_dir.to_string_lossy().to_string())
    }

    pub fn info(&self, scope: &str, event: &str, message: &str, fields: Value) {
        let _ = self.write_once("INFO", scope, event, message, fields);
    }

    pub fn warn(&self, scope: &str, event: &str, message: &str, fields: Value) {
        let _ = self.write_once("WARN", scope, event, message, fields);
    }

    pub fn error(&self, scope: &str, event: &str, message: &str, fields: Value) {
        let _ = self.write_once("ERROR", scope, event, message, fields);
    }

    fn write_once(
        &self,
        level: &str,
        scope: &str,
        event: &str,
        message: &str,
        fields: Value,
    ) -> Result<(), String> {
        let session_id = format!(
            "event-{}",
            self.inner
                .session_sequence
                .fetch_add(1, Ordering::Relaxed)
                .saturating_add(1)
        );
        let record = format_log_record(
            timestamp_millis(),
            level,
            scope,
            &session_id,
            event,
            message,
            &fields,
        );
        self.inner.write_record(&record)
    }
}

impl LogSession {
    pub fn path_string(&self) -> String {
        self.inner.logger.today_log_path_string()
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

        self.inner.logger.write_record(&record)
    }
}

impl AppLoggerInner {
    fn write_record(&self, record: &str) -> Result<(), String> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|error| format!("日志文件锁定失败: {error}"))?;
        writer.ensure_current(&self.log_dir)?;
        writeln!(writer.file, "{record}").map_err(|error| format!("日志写入失败: {error}"))
    }

    fn today_log_path_string(&self) -> String {
        daily_log_path(&self.log_dir, &current_log_date())
            .to_string_lossy()
            .to_string()
    }
}

impl DailyLogWriter {
    fn open(log_dir: &Path, date: &str) -> Result<Self, String> {
        fs::create_dir_all(log_dir).map_err(|error| format!("无法创建日志目录: {error}"))?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(daily_log_path(log_dir, date))
            .map_err(|error| format!("无法创建日志文件: {error}"))?;

        Ok(Self {
            date: date.to_string(),
            file,
        })
    }

    fn ensure_current(&mut self, log_dir: &Path) -> Result<(), String> {
        let date = current_log_date();
        if self.date == date {
            return Ok(());
        }

        *self = Self::open(log_dir, &date)?;
        Ok(())
    }
}

fn current_log_date() -> String {
    Local::now().format("%Y%m%d").to_string()
}

fn daily_log_path(log_dir: &Path, date: &str) -> PathBuf {
    log_dir.join(format!("{date}.log"))
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

    if normalized.contains(' ')
        || normalized.contains('|')
        || normalized.contains('=')
        || normalized.contains('"')
        || normalized.contains('\\')
    {
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
        let mut command = create_command("explorer");
        command.arg(path);
        command
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = create_command("open");
        command.arg(path);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = create_command("xdg-open");
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::thread;

    #[test]
    fn writes_all_sessions_to_one_daily_file() {
        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let logger = AppLogger::new_with_log_dir(temp_dir.path().to_path_buf())
            .expect("logger should be created");

        let first = logger
            .start_session("first_scope")
            .expect("first session should start");
        first.info("first_event", "第一条日志", json!({ "value": 1 }));

        let second = logger
            .start_session("second_scope")
            .expect("second session should start");
        second.warn("second_event", "第二条日志", json!({ "value": 2 }));

        let daily_path = temp_dir.path().join(format!("{}.log", current_log_date()));
        let daily_path_string = daily_path.to_string_lossy().to_string();
        assert_eq!(first.path_string(), daily_path_string);
        assert_eq!(second.path_string(), daily_path_string);

        let entries = fs::read_dir(temp_dir.path())
            .expect("log dir should be readable")
            .collect::<Result<Vec<_>, _>>()
            .expect("entries should be readable");
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].file_name().to_string_lossy(),
            format!("{}.log", current_log_date())
        );

        let content = fs::read_to_string(daily_path).expect("daily log should be readable");
        assert!(content.contains("[first_scope] session_start"));
        assert!(content.contains("[first_scope] first_event"));
        assert!(content.contains("[second_scope] session_start"));
        assert!(content.contains("[second_scope] second_event"));
    }

    #[test]
    fn formats_inline_fields_without_multiline_records() {
        let record = format_log_record(
            123,
            "INFO",
            "test_scope",
            "test-session",
            "event",
            "message",
            &json!({
                "plain": "value",
                "spaced": "value with space",
                "multiline": "a\nb",
                "object": { "nested": true },
            }),
        );

        assert!(!record.contains('\n'));
        assert!(record.contains("plain=value"));
        assert!(record.contains("spaced=\"value with space\""));
        assert!(record.contains("multiline=\"a\\nb\""));
        assert!(record.contains("object=\"{\\\"nested\\\":true}\""));
    }

    #[test]
    fn concurrent_sessions_share_daily_writer() {
        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let logger = AppLogger::new_with_log_dir(temp_dir.path().to_path_buf())
            .expect("logger should be created");

        let handles = (0..8)
            .map(|thread_index| {
                let logger = logger.clone();
                thread::spawn(move || {
                    let session = logger
                        .start_session("concurrent")
                        .expect("session should start");
                    for event_index in 0..20 {
                        session.info(
                            "concurrent_event",
                            "并发写入",
                            json!({
                                "thread": thread_index,
                                "event": event_index,
                            }),
                        );
                    }
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.join().expect("thread should finish");
        }

        let content =
            fs::read_to_string(temp_dir.path().join(format!("{}.log", current_log_date())))
                .expect("daily log should be readable");
        let event_count = content
            .lines()
            .filter(|line| line.contains("concurrent_event"))
            .count();
        let session_count = content
            .lines()
            .filter(|line| line.contains("session_start"))
            .count();

        assert_eq!(event_count, 160);
        assert_eq!(session_count, 8);
    }
}
