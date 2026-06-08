use crate::settings::{AppSettings, LlmConfig};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::Notify;
use tokio::time::sleep;

const MIN_AI_CONCURRENCY: usize = 1;
const MAX_AI_CONCURRENCY: usize = 100;
const OPENAI_CHAT_COMPLETIONS_PATH: &str = "chat/completions";
const OPENAI_RESPONSES_PATH: &str = "responses";
const ANTHROPIC_MESSAGES_PATH: &str = "messages";
const DEFAULT_AI_REQUEST_ATTEMPTS: usize = 3;
const RATE_LIMIT_BACKOFF_MIN_SECONDS: u64 = 15;
const RATE_LIMIT_BACKOFF_MAX_SECONDS: u64 = 90;
const RAW_RESPONSE_LOG_CHARS: usize = 8_000;
const DEFAULT_AI_CONNECT_TIMEOUT_SECONDS: u64 = 60;
const CONNECTION_CHECK_REQUEST_TIMEOUT_SECONDS: u64 = 60;
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
    temperature: Option<f32>,
    force_non_streaming: bool,
    disable_reasoning: bool,
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
    connection_check_timeout: Duration,
    rate_limit_cooldown_until: Arc<Mutex<Option<Instant>>>,
}

impl AiService {
    pub fn new(thread_count: u32) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(DEFAULT_AI_CONNECT_TIMEOUT_SECONDS))
            .build()
            .map_err(|error| format!("无法初始化 AI 客户端: {error}"))?;

        Ok(Self {
            client,
            limiter: Arc::new(AiConcurrencyLimiter::new(thread_count)),
            connection_check_timeout: Duration::from_secs(CONNECTION_CHECK_REQUEST_TIMEOUT_SECONDS),
            rate_limit_cooldown_until: Arc::new(Mutex::new(None)),
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
        let response = tokio::time::timeout(
            self.connection_check_timeout,
            self.chat_for_connection_check(
                settings,
                TEST_SYSTEM_PROMPT.to_string(),
                TEST_USER_PROMPT.to_string(),
                16,
            ),
        )
        .await
        .map_err(|_| {
            format!(
                "LLM 连接检查超时（{} 秒）",
                self.connection_check_timeout.as_secs()
            )
        })??;
        let latency_ms = started_at.elapsed().as_millis();
        let (service, config) = selected_llm_config(settings)?;

        Ok(LlmConnectionCheckResult {
            service: service.to_string(),
            model: config.model.trim().to_string(),
            latency_ms,
            message: format!("模型响应: {}", summarize_response_text(&response)),
        })
    }

    #[allow(dead_code)]
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
            temperature: None,
            force_non_streaming: false,
            disable_reasoning: false,
        };

        self.send_chat(service, config, &request, None).await
    }

    pub async fn chat_for_structured_output(
        &self,
        settings: &AppSettings,
        system_prompt: String,
        user_prompt: String,
        max_output_tokens: u32,
    ) -> Result<String, String> {
        self.chat(settings, system_prompt, user_prompt, max_output_tokens)
            .await
    }

    async fn chat_for_connection_check(
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
            temperature: None,
            force_non_streaming: true,
            disable_reasoning: true,
        };

        self.send_chat(
            service,
            config,
            &request,
            Some(self.connection_check_timeout),
        )
        .await
    }

    async fn send_chat(
        &self,
        service: &str,
        config: &LlmConfig,
        request: &AiChatRequest,
        request_timeout: Option<Duration>,
    ) -> Result<String, String> {
        validate_llm_config(config)?;

        let mut last_error = String::new();

        for attempt in 1..=DEFAULT_AI_REQUEST_ATTEMPTS {
            self.wait_for_rate_limit_cooldown().await?;

            let result = {
                let _permit = self.limiter.clone().acquire().await?;
                match service {
                    "openai" => {
                        self.send_openai_chat_completion(config, request, request_timeout)
                            .await
                    }
                    "openai-responses" => {
                        self.send_openai_response(config, request, request_timeout)
                            .await
                    }
                    "anthropic" => {
                        self.send_anthropic_message(config, request, request_timeout)
                            .await
                    }
                    _ => return Err(format!("暂不支持该 LLM 服务: {service}")),
                }
            };

            match result {
                Ok(response) => return Ok(response),
                Err(error) => {
                    last_error = error;
                    if attempt < DEFAULT_AI_REQUEST_ATTEMPTS {
                        let delay = retry_delay(attempt, &last_error);
                        if is_rate_limit_error(&last_error) {
                            self.extend_rate_limit_cooldown(delay)?;
                        }
                        sleep(delay).await;
                        continue;
                    }
                }
            }
        }

        Err(format!(
            "LLM 请求失败，已尝试 {} 次: {}",
            DEFAULT_AI_REQUEST_ATTEMPTS, last_error
        ))
    }

    async fn wait_for_rate_limit_cooldown(&self) -> Result<(), String> {
        loop {
            let delay = {
                let guard = self
                    .rate_limit_cooldown_until
                    .lock()
                    .map_err(|error| format!("AI 限流冷却锁定失败: {error}"))?;

                guard
                    .and_then(|until| until.checked_duration_since(Instant::now()))
                    .unwrap_or_default()
            };

            if delay.is_zero() {
                return Ok(());
            }

            sleep(delay).await;
        }
    }

    fn extend_rate_limit_cooldown(&self, delay: Duration) -> Result<(), String> {
        let cooldown_until = Instant::now() + delay;
        let mut guard = self
            .rate_limit_cooldown_until
            .lock()
            .map_err(|error| format!("AI 限流冷却锁定失败: {error}"))?;

        if guard
            .map(|current| cooldown_until > current)
            .unwrap_or(true)
        {
            *guard = Some(cooldown_until);
        }

        Ok(())
    }

    async fn send_openai_chat_completion(
        &self,
        config: &LlmConfig,
        request: &AiChatRequest,
        request_timeout: Option<Duration>,
    ) -> Result<String, String> {
        let is_streaming = is_streaming_enabled(config, request);
        let mut payload = json!({
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
            "stream": is_streaming,
            "max_tokens": request.max_output_tokens,
        });

        if let Some(temperature) = request.temperature {
            payload["temperature"] = json!(temperature);
        }

        if let Some(effort) = openai_chat_reasoning_effort(config, request) {
            payload["reasoning_effort"] = json!(effort);
        }

        let url = api_url(&config.base_url, OPENAI_CHAT_COMPLETIONS_PATH);
        let headers = openai_headers(&config.api_key)?;

        if is_streaming {
            let body = self
                .post_text(&url, headers, &payload, request_timeout)
                .await?;
            parse_openai_chat_completion_stream_text(&body, !request.disable_reasoning)
        } else {
            let value = self
                .post_json(&url, headers, &payload, request_timeout)
                .await?;
            parse_openai_chat_completion_text(&value, !request.disable_reasoning)
        }
    }

    async fn send_openai_response(
        &self,
        config: &LlmConfig,
        request: &AiChatRequest,
        request_timeout: Option<Duration>,
    ) -> Result<String, String> {
        let is_streaming = is_streaming_enabled(config, request);
        let mut payload = json!({
            "model": config.model.trim(),
            "instructions": request.system_prompt.as_str(),
            "input": request.user_prompt.as_str(),
            "stream": is_streaming,
            "max_output_tokens": request.max_output_tokens,
        });

        if let Some(effort) = openai_responses_reasoning_effort(config, request) {
            payload["reasoning"] = json!({ "effort": effort });
        }

        let url = api_url(&config.base_url, OPENAI_RESPONSES_PATH);
        let headers = openai_headers(&config.api_key)?;

        if is_streaming {
            let body = self
                .post_text(&url, headers, &payload, request_timeout)
                .await?;
            parse_openai_response_stream_text(&body)
        } else {
            let value = self
                .post_json(&url, headers, &payload, request_timeout)
                .await?;
            parse_openai_response_text(&value)
        }
    }

    async fn send_anthropic_message(
        &self,
        config: &LlmConfig,
        request: &AiChatRequest,
        request_timeout: Option<Duration>,
    ) -> Result<String, String> {
        let is_streaming = is_streaming_enabled(config, request);
        let mut payload = json!({
            "model": config.model.trim(),
            "system": request.system_prompt.as_str(),
            "messages": [
                {
                    "role": "user",
                    "content": request.user_prompt.as_str(),
                },
            ],
            "stream": is_streaming,
            "max_tokens": request.max_output_tokens,
        });

        if let Some(effort) = anthropic_reasoning_effort(config, request) {
            payload["thinking"] = json!({
                "type": "adaptive",
            });
            payload["output_config"] = json!({ "effort": effort });
        }

        let url = api_url(&config.base_url, ANTHROPIC_MESSAGES_PATH);
        let headers = anthropic_headers(&config.api_key)?;

        if is_streaming {
            let body = self
                .post_text(&url, headers, &payload, request_timeout)
                .await?;
            parse_anthropic_message_stream_text(&body)
        } else {
            let value = self
                .post_json(&url, headers, &payload, request_timeout)
                .await?;
            parse_anthropic_message_text(&value)
        }
    }

    async fn post_json(
        &self,
        url: &str,
        headers: HeaderMap,
        payload: &Value,
        request_timeout: Option<Duration>,
    ) -> Result<Value, String> {
        let body = self
            .post_text(url, headers, payload, request_timeout)
            .await?;

        serde_json::from_str(&body).map_err(|error| {
            format!(
                "LLM 响应解析失败: {error}; 原始响应: {}",
                truncate_text(&body, RAW_RESPONSE_LOG_CHARS)
            )
        })
    }

    async fn post_text(
        &self,
        url: &str,
        headers: HeaderMap,
        payload: &Value,
        request_timeout: Option<Duration>,
    ) -> Result<String, String> {
        let mut request_builder = self.client.post(url).headers(headers).json(payload);

        if let Some(request_timeout) = request_timeout {
            request_builder = request_builder.timeout(request_timeout);
        }

        let response = request_builder.send().await.map_err(format_request_error)?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| format!("LLM 响应读取失败: {error}"))?;

        if !status.is_success() {
            return Err(format!(
                "LLM 请求失败（HTTP {}）: {}",
                status.as_u16(),
                truncate_text(&body, RAW_RESPONSE_LOG_CHARS)
            ));
        }

        Ok(body)
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

fn selected_llm_config<'a>(settings: &'a AppSettings) -> Result<(&'a str, &'a LlmConfig), String> {
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

fn is_streaming_enabled(config: &LlmConfig, request: &AiChatRequest) -> bool {
    config.is_streaming && !request.force_non_streaming
}

fn openai_chat_reasoning_effort(
    config: &LlmConfig,
    request: &AiChatRequest,
) -> Option<&'static str> {
    openai_reasoning_effort(config, request)
}

fn openai_responses_reasoning_effort(
    config: &LlmConfig,
    request: &AiChatRequest,
) -> Option<&'static str> {
    openai_reasoning_effort(config, request)
}

fn openai_reasoning_effort(config: &LlmConfig, request: &AiChatRequest) -> Option<&'static str> {
    if request.disable_reasoning {
        return None;
    }

    match config.reasoning_effort.trim() {
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "ultra-high" => Some("xhigh"),
        _ => None,
    }
}

fn anthropic_reasoning_effort(config: &LlmConfig, request: &AiChatRequest) -> Option<&'static str> {
    if request.disable_reasoning {
        return None;
    }

    match config.reasoning_effort.trim() {
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "ultra-high" => Some("max"),
        _ => None,
    }
}

fn retry_delay(attempt: usize, error: &str) -> Duration {
    if is_rate_limit_error(error) {
        let exponential_seconds = RATE_LIMIT_BACKOFF_MIN_SECONDS
            .saturating_mul(2_u64.saturating_pow(attempt.saturating_sub(1) as u32))
            .min(RATE_LIMIT_BACKOFF_MAX_SECONDS);
        let jitter_ms = jitter_millis(0, 1200);
        return Duration::from_secs(exponential_seconds) + Duration::from_millis(jitter_ms);
    }

    let base_ms = 500_u64.saturating_mul(attempt as u64).min(2_000);
    Duration::from_millis(base_ms + jitter_millis(0, 300))
}

fn is_rate_limit_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("http 429")
        || lower.contains("\"code\": 429")
        || lower.contains("too many requests")
        || lower.contains("rate limit")
        || lower.contains("ratelimit")
        || lower.contains("limitation")
}

fn format_request_error(error: reqwest::Error) -> String {
    if error.is_timeout() {
        return format!("LLM 请求超时: {error}");
    }

    if error.is_connect() {
        return format!("LLM 连接失败: {error}");
    }

    format!("LLM 请求失败: {error}")
}

fn jitter_millis(min: u64, max: u64) -> u64 {
    if max <= min {
        return min;
    }

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos() as u64)
        .unwrap_or(0);

    min + nanos % (max - min + 1)
}

fn api_url(base_url: &str, path: &str) -> String {
    format!("{}/{}", normalize_base_url(base_url), path)
}

fn normalize_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return String::new();
    }

    let address = trimmed
        .split_once("://")
        .map(|(_, value)| value)
        .unwrap_or(trimmed);

    if address.contains('/') {
        trimmed.to_string()
    } else {
        format!("{trimmed}/v1")
    }
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

fn parse_openai_chat_completion_text(
    value: &Value,
    allow_reasoning_fallback: bool,
) -> Result<String, String> {
    let choices = value
        .get("choices")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!(
                "OpenAI 响应内容为空: {}",
                truncate_text(&value.to_string(), RAW_RESPONSE_LOG_CHARS)
            )
        })?;

    let text = choices
        .iter()
        .filter_map(extract_openai_choice_content)
        .collect::<Vec<_>>()
        .join("")
        .trim()
        .to_string()
        .pipe_non_empty()
        .or_else(|| {
            if !allow_reasoning_fallback {
                return None;
            }

            choices
                .iter()
                .filter_map(extract_openai_choice_reasoning_content)
                .collect::<Vec<_>>()
                .join("")
                .trim()
                .to_string()
                .pipe_non_empty()
        })
        .ok_or_else(|| {
            format!(
                "OpenAI 响应内容为空: {}",
                truncate_text(&value.to_string(), RAW_RESPONSE_LOG_CHARS)
            )
        })?;

    Ok(text)
}

fn parse_openai_chat_completion_stream_text(
    body: &str,
    allow_reasoning_fallback: bool,
) -> Result<String, String> {
    let mut content_text = String::new();
    let mut reasoning_fallback_text = String::new();

    for value in parse_sse_json_values(body) {
        let Some(choices) = value.get("choices").and_then(Value::as_array) else {
            continue;
        };

        for choice in choices {
            if let Some(chunk_text) = extract_openai_choice_content(choice) {
                content_text.push_str(&chunk_text);
            }

            if let Some(chunk_text) = extract_openai_choice_reasoning_content(choice) {
                reasoning_fallback_text.push_str(&chunk_text);
            }
        }
    }

    content_text
        .trim()
        .to_string()
        .pipe_non_empty()
        .or_else(|| {
            if allow_reasoning_fallback {
                reasoning_fallback_text.trim().to_string().pipe_non_empty()
            } else {
                None
            }
        })
        .ok_or_else(|| {
            format!(
                "OpenAI 流式响应内容为空: {}",
                truncate_text(body, RAW_RESPONSE_LOG_CHARS)
            )
        })
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
        .ok_or_else(|| {
            format!(
                "OpenAI Responses 响应内容为空: {}",
                truncate_text(&value.to_string(), RAW_RESPONSE_LOG_CHARS)
            )
        })
}

fn parse_openai_response_stream_text(body: &str) -> Result<String, String> {
    let mut text = String::new();

    for value in parse_sse_json_values(body) {
        if value.get("type").and_then(Value::as_str) == Some("response.output_text.delta") {
            if let Some(delta) = value.get("delta").and_then(Value::as_str) {
                text.push_str(delta);
                continue;
            }
        }

        if let Some(delta) = value
            .get("delta")
            .and_then(Value::as_str)
            .filter(|delta| !delta.trim().is_empty())
        {
            text.push_str(delta);
            continue;
        }

        if text.is_empty() {
            if let Ok(snapshot_text) = parse_openai_response_text(&value) {
                text.push_str(&snapshot_text);
            }
        }
    }

    text.trim().to_string().pipe_non_empty().ok_or_else(|| {
        format!(
            "OpenAI Responses 流式响应内容为空: {}",
            truncate_text(body, RAW_RESPONSE_LOG_CHARS)
        )
    })
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
        .ok_or_else(|| {
            format!(
                "Anthropic 响应内容为空: {}",
                truncate_text(&value.to_string(), RAW_RESPONSE_LOG_CHARS)
            )
        })
}

fn parse_anthropic_message_stream_text(body: &str) -> Result<String, String> {
    let mut text = String::new();

    for value in parse_sse_json_values(body) {
        if value.get("type").and_then(Value::as_str) == Some("content_block_delta") {
            if let Some(delta) = value
                .get("delta")
                .and_then(|delta| delta.get("text"))
                .and_then(Value::as_str)
            {
                text.push_str(delta);
                continue;
            }
        }

        if value.get("type").and_then(Value::as_str) == Some("content_block_start") {
            if let Some(start_text) = value
                .get("content_block")
                .and_then(|content| content.get("text"))
                .and_then(Value::as_str)
            {
                text.push_str(start_text);
            }
        }
    }

    text.trim().to_string().pipe_non_empty().ok_or_else(|| {
        format!(
            "Anthropic 流式响应内容为空: {}",
            truncate_text(body, RAW_RESPONSE_LOG_CHARS)
        )
    })
}

fn extract_openai_choice_content(choice: &Value) -> Option<String> {
    if let Some(text) = choice.get("text").and_then(extract_text_value) {
        return Some(text);
    }

    choice
        .get("message")
        .or_else(|| choice.get("delta"))
        .and_then(|message| message.get("content"))
        .and_then(extract_text_value)
}

fn extract_openai_choice_reasoning_content(choice: &Value) -> Option<String> {
    choice
        .get("message")
        .or_else(|| choice.get("delta"))
        .and_then(|message| message.get("reasoning_content"))
        .and_then(extract_text_value)
}

fn extract_text_value(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }

    if let Some(text) = value.get("text").and_then(Value::as_str) {
        return Some(text.to_string());
    }

    if let Some(content) = value.get("content").and_then(extract_text_value) {
        return Some(content);
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

fn parse_sse_json_values(body: &str) -> Vec<Value> {
    body.lines()
        .filter_map(|line| line.trim().strip_prefix("data:"))
        .map(str::trim)
        .filter(|data| !data.is_empty() && *data != "[DONE]")
        .filter_map(|data| serde_json::from_str::<Value>(data).ok())
        .collect()
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
