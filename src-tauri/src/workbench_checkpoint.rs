use crate::settings::SettingsStore;
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

const CHECKPOINT_STATUS_ACTIVE: &str = "active";
const CHECKPOINT_STATUS_DONE: &str = "done";
const CHECKPOINT_STATUS_FAILED: &str = "failed";

#[derive(Debug, Clone)]
pub(crate) struct WorkbenchCheckpointContext {
    task_id: String,
    scope: String,
    input_key: String,
}

impl WorkbenchCheckpointContext {
    pub(crate) fn new(
        task_id: impl Into<String>,
        scope: impl Into<String>,
        input_key: impl Into<String>,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            scope: scope.into(),
            input_key: input_key.into(),
        }
    }

    pub(crate) fn child(&self, scope_suffix: impl AsRef<str>) -> Self {
        Self {
            task_id: self.task_id.clone(),
            scope: format!("{}:{}", self.scope, scope_suffix.as_ref()),
            input_key: self.input_key.clone(),
        }
    }

    pub(crate) fn task_id(&self) -> &str {
        &self.task_id
    }

    pub(crate) fn scope(&self) -> &str {
        &self.scope
    }

    pub(crate) fn input_key(&self) -> &str {
        &self.input_key
    }
}

pub(crate) fn checkpoint_hash(value: &Value) -> String {
    let text = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub(crate) fn load_checkpoint<T: DeserializeOwned>(
    store: &SettingsStore,
    context: &WorkbenchCheckpointContext,
    checkpoint_key: &str,
) -> Result<Option<T>, String> {
    let payload = store.with_connection(|connection| {
        connection
            .query_row(
                "
                SELECT payload
                FROM home_workbench_checkpoints
                WHERE task_id = ?1
                  AND scope = ?2
                  AND checkpoint_key = ?3
                  AND input_key = ?4
                  AND status = ?5
                LIMIT 1
                ",
                params![
                    context.task_id(),
                    context.scope(),
                    checkpoint_key,
                    context.input_key(),
                    CHECKPOINT_STATUS_DONE,
                ],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("无法读取工作台检查点: {error}"))
    })?;

    payload
        .map(|payload| {
            serde_json::from_str::<T>(&payload)
                .map_err(|error| format!("无法解析工作台检查点: {error}"))
        })
        .transpose()
}

pub(crate) fn mark_checkpoint_active(
    store: &SettingsStore,
    context: &WorkbenchCheckpointContext,
    checkpoint_key: &str,
) -> Result<(), String> {
    upsert_checkpoint(
        store,
        context,
        checkpoint_key,
        CHECKPOINT_STATUS_ACTIVE,
        Value::Null,
        "",
    )
}

pub(crate) fn mark_checkpoint_done<T: Serialize>(
    store: &SettingsStore,
    context: &WorkbenchCheckpointContext,
    checkpoint_key: &str,
    payload: &T,
) -> Result<(), String> {
    let payload =
        serde_json::to_value(payload).map_err(|error| format!("无法保存工作台检查点: {error}"))?;
    upsert_checkpoint(
        store,
        context,
        checkpoint_key,
        CHECKPOINT_STATUS_DONE,
        payload,
        "",
    )
}

pub(crate) fn mark_checkpoint_failed(
    store: &SettingsStore,
    context: &WorkbenchCheckpointContext,
    checkpoint_key: &str,
    error: &str,
) -> Result<(), String> {
    upsert_checkpoint(
        store,
        context,
        checkpoint_key,
        CHECKPOINT_STATUS_FAILED,
        Value::Null,
        error,
    )
}

fn upsert_checkpoint(
    store: &SettingsStore,
    context: &WorkbenchCheckpointContext,
    checkpoint_key: &str,
    status: &str,
    payload: Value,
    error_message: &str,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    let payload_text = serde_json::to_string(&payload).unwrap_or_else(|_| "null".to_string());
    store.with_connection(|connection| {
        let created_at = connection
            .query_row(
                "
                SELECT created_at
                FROM home_workbench_checkpoints
                WHERE task_id = ?1 AND scope = ?2 AND checkpoint_key = ?3
                LIMIT 1
                ",
                params![context.task_id(), context.scope(), checkpoint_key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("无法检查工作台检查点: {error}"))?
            .unwrap_or_else(|| now.clone());
        connection
            .execute(
                "
                INSERT INTO home_workbench_checkpoints (
                    task_id, scope, checkpoint_key, input_key, status,
                    payload, error_message, created_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ON CONFLICT(task_id, scope, checkpoint_key) DO UPDATE SET
                    input_key = excluded.input_key,
                    status = excluded.status,
                    payload = excluded.payload,
                    error_message = excluded.error_message,
                    updated_at = excluded.updated_at
                ",
                params![
                    context.task_id(),
                    context.scope(),
                    checkpoint_key,
                    context.input_key(),
                    status,
                    payload_text,
                    error_message,
                    created_at,
                    now,
                ],
            )
            .map(|_| ())
            .map_err(|error| format!("无法保存工作台检查点: {error}"))
    })
}
