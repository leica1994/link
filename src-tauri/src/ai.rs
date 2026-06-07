use crate::settings::{AppSettings, LlmConfig};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Notify;

const MIN_AI_CONCURRENCY: usize = 1;
const MAX_AI_CONCURRENCY: usize = 100;
const OPENAI_CHAT_COMPLETIONS_PATH: &str = "chat/completions";
const OPENAI_RESPONSES_PATH: &str = "responses";
const ANTHROPIC_MESSAGES_PATH: &str = "messages";
const TEST_SYSTEM_PROMPT: &str = "你是一个连接测试助手。";
const TEST_USER_PROMPT: &str = "请只回复 OK。";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmConnectionCheckResult {
    pub service: String,
    pub model: String,
    pub latency_ms: u128,
    pub message: String,
}

#[derive(Debug, Clone)]
struct AiChatRequest {
    system_prompt: String,
    user_prompt: String,
    max_output_tokens: u32,
}

#[derive(Debug)]
struct AiConcurrencyState {
    limit: usize,
    in_use: usize,
}

#[derive(Debug)]
struct AiConcurrencyLimiter {
    state: Mutex<AiConcurrencyState>,
    notify: Notify,
}

#[derive(Debug)]
struct AiPermit {
    limiter: Arc<AiConcurrencyLimiter>,
}

pub struct AiService {
    client: reqwest::Client,
    limiter: Arc<AiConcurrencyLimiter>,
}

impl AiService {
    pub fn new(thread_count: u32) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| format!("无法初始化 AI 客户端: {error}"))?;

        Ok(Self {
            client,
            limiter: Arc::new(AiConcurrencyLimiter::new(thread_count)),
        })
    }

    pub fn update_thread_count(&self, thread_count: u32) -> Result<(), String> {
        self.limiter.update_limit(thread_count)
    }

    pub async fn check_llm_connection(
        &self,
        settings: &AppSettings,
    ) -> Result<LlmConnectionCheckResult, String> {
        let started_at = Instant::now();
        let response = self
            .chat(
                settings,
                TEST_SYSTEM_PROMPT.to_string(),
                TEST_USER_PROMPT.to_string(),
                16,
            )
            .await?;
        let latency_ms = started_at.elapsed().as_millis();
        let (service, config) = selected_llm_config(settings)?;

        Ok(LlmConnectionCheckResult {
            service: service.to_string(),
            model: config.model.trim().to_string(),
            latency_ms,
            message: format!("模型响应: {}", summarize_response_text(&response)),
        })
    }

    pub async fn chat(
        &self,
        settings: &AppSettings,
        system_prompt: String,
        user_prompt: String,
        max_output_tokens: u32,
    ) -> Result<String, String> {
        let (service, config) = selected_llm_config(settings)?;
        let request = AiChatRequest {
            system_prompt,
            user_prompt,
            max_output_tokens,
        };

        self.send_chat(service, config, &request).await
    }

    async fn send_chat(
        &self,
        service: &str,
        config: &LlmConfig,
        request: &AiChatRequest,
    ) -> Result<String, String> {
        validate_llm_config(config)?;

        let _permit = self.limiter.clone().acquire().await?;

        match service {
            "openai" => self.send_openai_chat_completion(config, request).await,
            "openai-responses" => self.send_openai_response(config, request).await,
            "anthropic" => self.send_anthropic_message(config, request).await,
            _ => Err(format!("暂不支持该 LLM 服务: {service}")),
        }
    }

    async fn send_openai_chat_completion(
        &self,
        config: &LlmConfig,
        request: &AiChatRequest,
    ) -> Result<String, String> {
        let payload = json!({
            "model": config.model.trim(),
            "messages": [
                {
                    "role": "system",
                    "content": request.system_prompt.as_str(),
                },
                {
                    "role": "user",
                    "content": request.user_prompt.as_str(),
                },
            ],
            "stream": false,
            "max_tokens": request.max_output_tokens,
        });
        let value = self
            .post_json(
                &api_url(&config.base_url, OPENAI_CHAT_COMPLETIONS_PATH),
                openai_headers(&config.api_key)?,
                &payload,
            )
            .await?;

        parse_openai_chat_completion_text(&value)
    }

    async fn send_openai_response(
        &self,
        config: &LlmConfig,
        request: &AiChatRequest,
    ) -> Result<String, String> {
        let payload = json!({
            "model": config.model.trim(),
            "instructions": request.system_prompt.as_str(),
            "input": request.user_prompt.as_str(),
            "stream": false,
            "max_output_tokens": request.max_output_tokens,
        });
        let value = self
            .post_json(
                &api_url(&config.base_url, OPENAI_RESPONSES_PATH),
                openai_headers(&config.api_key)?,
                &payload,
            )
            .await?;

        parse_openai_response_text(&value)
    }

    async fn send_anthropic_message(
        &self,
        config: &LlmConfig,
        request: &AiChatRequest,
    ) -> Result<String, String> {
        let payload = json!({
            "model": config.model.trim(),
            "system": request.system_prompt.as_str(),
            "messages": [
                {
                    "role": "user",
                    "content": request.user_prompt.as_str(),
                },
            ],
            "stream": false,
            "max_tokens": request.max_output_tokens,
        });
        let value = self
            .post_json(
                &api_url(&config.base_url, ANTHROPIC_MESSAGES_PATH),
                anthropic_headers(&config.api_key)?,
                &payload,
            )
            .await?;

        parse_anthropic_message_text(&value)
    }

    async fn post_json(
        &self,
        url: &str,
        headers: HeaderMap,
        payload: &Value,
    ) -> Result<Value, String> {
        let response = self
            .client
            .post(url)
            .headers(headers)
            .json(payload)
            .send()
            .await
            .map_err(|error| format!("LLM 请求失败: {error}"))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| format!("LLM 响应读取失败: {error}"))?;

        if !status.is_success() {
            return Err(format!(
                "LLM 请求失败（HTTP {}）: {}",
                status.as_u16(),
                truncate_text(&body, 500)
            ));
        }

        serde_json::from_str(&body).map_err(|error| format!("LLM 响应解析失败: {error}"))
    }
}

impl AiConcurrencyLimiter {
    fn new(thread_count: u32) -> Self {
        Self {
            state: Mutex::new(AiConcurrencyState {
                limit: normalize_thread_count(thread_count),
                in_use: 0,
            }),
            notify: Notify::new(),
        }
    }

    fn update_limit(&self, thread_count: u32) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|error| format!("AI 并发池锁定失败: {error}"))?;
        state.limit = normalize_thread_count(thread_count);
        drop(state);
        self.notify.notify_waiters();

        Ok(())
    }

    async fn acquire(self: Arc<Self>) -> Result<AiPermit, String> {
        loop {
            let notified = self.notify.notified();

            {
                let mut state = self
                    .state
                    .lock()
                    .map_err(|error| format!("AI 并发池锁定失败: {error}"))?;

                if state.in_use < state.limit {
                    state.in_use += 1;
                    return Ok(AiPermit {
                        limiter: Arc::clone(&self),
                    });
                }
            }

            notified.await;
        }
    }
}

impl Drop for AiPermit {
    fn drop(&mut self) {
        if let Ok(mut state) = self.limiter.state.lock() {
            state.in_use = state.in_use.saturating_sub(1);
        }

        self.limiter.notify.notify_one();
    }
}

fn selected_llm_config<'a>(
    settings: &'a AppSettings,
) -> Result<(&'a str, &'a LlmConfig), String> {
    let service = settings.selected_llm_service.trim();
    let config = settings
        .llm_configs
        .get(service)
        .ok_or_else(|| "缺少当前 LLM 服务配置".to_string())?;

    Ok((service, config))
}

fn validate_llm_config(config: &LlmConfig) -> Result<(), String> {
    if config.base_url.trim().is_empty() {
        return Err("请先填写 Base URL".to_string());
    }

    if config.api_key.trim().is_empty() {
        return Err("请先填写 API Key".to_string());
    }

    if config.model.trim().is_empty() {
        return Err("请先填写模型名称".to_string());
    }

    Ok(())
}

fn normalize_thread_count(thread_count: u32) -> usize {
    (thread_count as usize).clamp(MIN_AI_CONCURRENCY, MAX_AI_CONCURRENCY)
}

fn api_url(base_url: &str, path: &str) -> String {
    format!("{}/{}", base_url.trim().trim_end_matches('/'), path)
}

fn openai_headers(api_key: &str) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer {}", api_key.trim()))
            .map_err(|error| format!("API Key 格式无效: {error}"))?,
    );

    Ok(headers)
}

fn anthropic_headers(api_key: &str) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
    headers.insert(
        "x-api-key",
        HeaderValue::from_str(api_key.trim())
            .map_err(|error| format!("API Key 格式无效: {error}"))?,
    );

    Ok(headers)
}

fn parse_openai_chat_completion_text(value: &Value) -> Result<String, String> {
    value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(extract_text_value)
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| "OpenAI 响应内容为空".to_string())
}

fn parse_openai_response_text(value: &Value) -> Result<String, String> {
    if let Some(text) = value
        .get("output_text")
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
    {
        return Ok(text.to_string());
    }

    value
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|output| output.get("content").and_then(Value::as_array))
        .flatten()
        .filter_map(|content| content.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("")
        .trim()
        .to_string()
        .pipe_non_empty()
        .ok_or_else(|| "OpenAI Responses 响应内容为空".to_string())
}

fn parse_anthropic_message_text(value: &Value) -> Result<String, String> {
    value
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|content| content.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("")
        .trim()
        .to_string()
        .pipe_non_empty()
        .ok_or_else(|| "Anthropic 响应内容为空".to_string())
}

fn extract_text_value(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }

    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|content| content.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("")
        .trim()
        .to_string()
        .pipe_non_empty()
}

fn summarize_response_text(text: &str) -> String {
    truncate_text(text.trim(), 80)
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = normalized.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();

    if chars.next().is_some() {
        format!("{truncated}...")
    } else if truncated.is_empty() {
        "无响应内容".to_string()
    } else {
        truncated
    }
}

trait NonEmptyString {
    fn pipe_non_empty(self) -> Option<String>;
}

impl NonEmptyString for String {
    fn pipe_non_empty(self) -> Option<String> {
        if self.is_empty() {
            None
        } else {
            Some(self)
        }
    }
}

#[tauri::command]
pub async fn check_llm_connection(
    settings_store: tauri::State<'_, crate::settings::SettingsStore>,
    ai_service: tauri::State<'_, AiService>,
) -> Result<LlmConnectionCheckResult, String> {
    let settings = settings_store.load()?;

    ai_service.check_llm_connection(&settings).await
}
