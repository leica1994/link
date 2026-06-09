use base64::{engine::general_purpose, Engine as _};
use chrono::{FixedOffset, Utc};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tungstenite::client::IntoClientRequest;
use tungstenite::{connect, Message};
use uuid::Uuid;

use crate::settings::SettingsStore;

const EDGE_TTS_ENGINE: &str = "edge-tts";
const EDGE_TTS_ENGINE_LABEL: &str = "EDGE-TTS";
const EDGE_TTS_BASE_URL: &str = "speech.platform.bing.com/consumer/speech/synthesize/readaloud";
const EDGE_TTS_TRUSTED_CLIENT_TOKEN: &str = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";
const EDGE_TTS_SEC_MS_GEC_VERSION: &str = "1-143.0.3650.75";
const EDGE_TTS_CHROMIUM_MAJOR_VERSION: &str = "143";
const WINDOWS_EPOCH_SECONDS: u64 = 11_644_473_600;
const NANO_AI_TTS_ENGINE: &str = "nano-ai-tts";
const NANO_AI_TTS_ENGINE_LABEL: &str = "纳米AI TTS";
const NANO_AI_TTS_BASE_URL: &str = "https://bot.n.cn";
const NANO_AI_TTS_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/117.0.0.0 Safari/537.36";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingVoiceOption {
    pub engine: String,
    pub engine_label: String,
    pub model_key: String,
    pub display_name: String,
    pub locale: String,
    pub gender: String,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingModel {
    pub id: String,
    pub engine: String,
    pub engine_label: String,
    pub model_key: String,
    pub display_name: String,
    pub locale: String,
    pub gender: String,
    pub enabled: bool,
    pub metadata: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListDubbingVoicesRequest {
    pub engine: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddDubbingModelRequest {
    pub engine: String,
    pub model_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetDubbingModelEnabledRequest {
    pub id: String,
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteDubbingModelRequest {
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewDubbingVoiceRequest {
    pub engine: String,
    pub model_key: String,
    pub locale: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewDubbingVoiceResult {
    pub audio_data_url: String,
}

trait DubbingEngine {
    fn list_voices(&self) -> Result<Vec<DubbingVoiceOption>, String>;
    fn synthesize_preview(&self, model_key: &str, locale: Option<&str>) -> Result<Vec<u8>, String>;
}

struct EdgeTtsEngine;
struct NanoAiTtsEngine;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct EdgeTtsVoice {
    short_name: String,
    friendly_name: String,
    locale: String,
    gender: String,
    #[serde(default)]
    voice_tag: Value,
}

#[derive(Debug, Deserialize)]
struct NanoAiPlatformResponse {
    data: NanoAiPlatformData,
}

#[derive(Debug, Deserialize)]
struct NanoAiPlatformData {
    #[serde(default)]
    list: Vec<NanoAiRobot>,
}

#[derive(Debug, Deserialize)]
struct NanoAiRobot {
    #[serde(default)]
    tag: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    icon: String,
}

#[tauri::command]
pub fn list_dubbing_models(
    store: tauri::State<'_, SettingsStore>,
) -> Result<Vec<DubbingModel>, String> {
    read_dubbing_models(&store)
}

#[tauri::command]
pub fn list_dubbing_voices(
    request: ListDubbingVoicesRequest,
) -> Result<Vec<DubbingVoiceOption>, String> {
    engine_for(&request.engine)?.list_voices()
}

#[tauri::command]
pub fn add_dubbing_model(
    store: tauri::State<'_, SettingsStore>,
    request: AddDubbingModelRequest,
) -> Result<DubbingModel, String> {
    let voice = engine_for(&request.engine)?
        .list_voices()?
        .into_iter()
        .find(|voice| voice.model_key == request.model_key)
        .ok_or_else(|| "未找到该语音".to_string())?;

    insert_dubbing_model(&store, voice)
}

#[tauri::command]
pub fn set_dubbing_model_enabled(
    store: tauri::State<'_, SettingsStore>,
    request: SetDubbingModelEnabledRequest,
) -> Result<DubbingModel, String> {
    let updated_at = Utc::now().to_rfc3339();

    store.with_connection(|connection| {
        let changed = connection
            .execute(
                "
                UPDATE dubbing_models
                SET enabled = ?2, updated_at = ?3
                WHERE id = ?1
                ",
                params![request.id, if request.enabled { 1 } else { 0 }, updated_at],
            )
            .map_err(|error| format!("无法更新配音模型: {error}"))?;

        if changed == 0 {
            return Err("未找到该配音模型".to_string());
        }

        read_dubbing_model_by_id(connection, &request.id)
    })
}

#[tauri::command]
pub fn delete_dubbing_model(
    store: tauri::State<'_, SettingsStore>,
    request: DeleteDubbingModelRequest,
) -> Result<(), String> {
    store.with_connection(|connection| {
        let changed = connection
            .execute(
                "DELETE FROM dubbing_models WHERE id = ?1",
                params![request.id],
            )
            .map_err(|error| format!("无法删除配音模型: {error}"))?;

        if changed == 0 {
            return Err("未找到该配音模型".to_string());
        }

        Ok(())
    })
}

#[tauri::command]
pub fn preview_dubbing_voice(
    request: PreviewDubbingVoiceRequest,
) -> Result<PreviewDubbingVoiceResult, String> {
    let audio = engine_for(&request.engine)?
        .synthesize_preview(&request.model_key, request.locale.as_deref())?;
    let audio_data_url = format!(
        "data:audio/mpeg;base64,{}",
        general_purpose::STANDARD.encode(audio)
    );

    Ok(PreviewDubbingVoiceResult { audio_data_url })
}

fn engine_for(engine: &str) -> Result<Box<dyn DubbingEngine>, String> {
    match engine {
        EDGE_TTS_ENGINE => Ok(Box::new(EdgeTtsEngine)),
        NANO_AI_TTS_ENGINE => Ok(Box::new(NanoAiTtsEngine)),
        _ => Err("不支持的配音引擎".to_string()),
    }
}

fn read_dubbing_models(store: &SettingsStore) -> Result<Vec<DubbingModel>, String> {
    store.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "
                SELECT id, engine, model_key, display_name, locale, gender, enabled, metadata, created_at, updated_at
                FROM dubbing_models
                ORDER BY created_at DESC
                ",
            )
            .map_err(|error| format!("无法读取配音模型: {error}"))?;

        let rows = statement
            .query_map([], map_dubbing_model)
            .map_err(|error| format!("无法读取配音模型: {error}"))?;

        let mut models = Vec::new();
        for row in rows {
            models.push(row.map_err(|error| format!("无法解析配音模型: {error}"))?);
        }

        Ok(models)
    })
}

fn read_dubbing_model_by_id(
    connection: &rusqlite::Connection,
    id: &str,
) -> Result<DubbingModel, String> {
    connection
        .query_row(
            "
            SELECT id, engine, model_key, display_name, locale, gender, enabled, metadata, created_at, updated_at
            FROM dubbing_models
            WHERE id = ?1
            ",
            params![id],
            map_dubbing_model,
        )
        .optional()
        .map_err(|error| format!("无法读取配音模型: {error}"))?
        .ok_or_else(|| "未找到该配音模型".to_string())
}

fn insert_dubbing_model(
    store: &SettingsStore,
    voice: DubbingVoiceOption,
) -> Result<DubbingModel, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let metadata = serde_json::to_string(&voice.metadata)
        .map_err(|error| format!("无法序列化配音模型: {error}"))?;

    store.with_connection(|connection| {
        connection
            .execute(
                "
                INSERT INTO dubbing_models (
                    id,
                    engine,
                    model_key,
                    display_name,
                    locale,
                    gender,
                    enabled,
                    metadata,
                    created_at,
                    updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?9)
                ",
                params![
                    id,
                    voice.engine,
                    voice.model_key,
                    voice.display_name,
                    voice.locale,
                    voice.gender,
                    metadata,
                    now,
                    now,
                ],
            )
            .map_err(|error| {
                if error.to_string().contains("UNIQUE") {
                    "该语音模型已添加".to_string()
                } else {
                    format!("无法添加配音模型: {error}")
                }
            })?;

        read_dubbing_model_by_id(connection, &id)
    })
}

fn map_dubbing_model(row: &Row<'_>) -> rusqlite::Result<DubbingModel> {
    let engine: String = row.get(1)?;
    let metadata_text: String = row.get(7)?;
    let metadata = serde_json::from_str(&metadata_text).unwrap_or_else(|_| json!({}));

    Ok(DubbingModel {
        id: row.get(0)?,
        engine: engine.clone(),
        engine_label: engine_label(&engine).to_string(),
        model_key: row.get(2)?,
        display_name: row.get(3)?,
        locale: row.get(4)?,
        gender: row.get(5)?,
        enabled: row.get::<_, i64>(6)? != 0,
        metadata,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn engine_label(engine: &str) -> &'static str {
    match engine {
        EDGE_TTS_ENGINE => EDGE_TTS_ENGINE_LABEL,
        NANO_AI_TTS_ENGINE => NANO_AI_TTS_ENGINE_LABEL,
        _ => "未知引擎",
    }
}

impl DubbingEngine for EdgeTtsEngine {
    fn list_voices(&self) -> Result<Vec<DubbingVoiceOption>, String> {
        let url = format!(
            "https://{EDGE_TTS_BASE_URL}/voices/list?trustedclienttoken={EDGE_TTS_TRUSTED_CLIENT_TOKEN}&Sec-MS-GEC={}&Sec-MS-GEC-Version={EDGE_TTS_SEC_MS_GEC_VERSION}",
            generate_sec_ms_gec()?
        );
        let voices = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|error| format!("无法创建 EDGE-TTS 客户端: {error}"))?
            .get(url)
            .headers(edge_voice_headers())
            .send()
            .map_err(|error| format!("无法获取 EDGE-TTS 语音列表: {error}"))?
            .error_for_status()
            .map_err(|error| format!("EDGE-TTS 语音列表请求失败: {error}"))?
            .json::<Vec<EdgeTtsVoice>>()
            .map_err(|error| format!("无法解析 EDGE-TTS 语音列表: {error}"))?;

        Ok(voices
            .into_iter()
            .map(|voice| DubbingVoiceOption {
                engine: EDGE_TTS_ENGINE.to_string(),
                engine_label: EDGE_TTS_ENGINE_LABEL.to_string(),
                model_key: voice.short_name,
                display_name: voice.friendly_name,
                locale: voice.locale,
                gender: voice.gender,
                metadata: json!({
                    "voiceTag": voice.voice_tag,
                }),
            })
            .collect())
    }

    fn synthesize_preview(&self, model_key: &str, locale: Option<&str>) -> Result<Vec<u8>, String> {
        synthesize_edge_tts_audio(model_key, preview_text_for_voice(model_key, locale))
    }
}

impl DubbingEngine for NanoAiTtsEngine {
    fn list_voices(&self) -> Result<Vec<DubbingVoiceOption>, String> {
        let response = nano_ai_client()?
            .get(format!("{NANO_AI_TTS_BASE_URL}/api/robot/platform"))
            .headers(nano_ai_headers()?)
            .send()
            .map_err(|error| format!("无法获取纳米AI TTS 语音列表: {error}"))?
            .error_for_status()
            .map_err(|error| format!("纳米AI TTS 语音列表请求失败: {error}"))?
            .json::<NanoAiPlatformResponse>()
            .map_err(|error| format!("无法解析纳米AI TTS 语音列表: {error}"))?;

        let mut seen_model_keys = HashSet::new();
        let voices = response
            .data
            .list
            .into_iter()
            .filter_map(|robot| {
                let model_key = robot.tag.trim().to_string();
                if model_key.is_empty() || !seen_model_keys.insert(model_key.clone()) {
                    return None;
                }

                let title = robot.title.trim();
                Some(DubbingVoiceOption {
                    engine: NANO_AI_TTS_ENGINE.to_string(),
                    engine_label: NANO_AI_TTS_ENGINE_LABEL.to_string(),
                    model_key: model_key.clone(),
                    display_name: if title.is_empty() {
                        model_key
                    } else {
                        title.to_string()
                    },
                    locale: "zh-CN".to_string(),
                    gender: String::new(),
                    metadata: json!({
                        "iconUrl": robot.icon,
                    }),
                })
            })
            .collect::<Vec<_>>();

        if voices.is_empty() {
            return Err("纳米AI TTS 未返回语音列表".to_string());
        }

        Ok(voices)
    }

    fn synthesize_preview(&self, model_key: &str, locale: Option<&str>) -> Result<Vec<u8>, String> {
        let preview_locale = locale
            .filter(|value| !value.trim().is_empty())
            .or(Some("zh-CN"));
        synthesize_nano_ai_tts_audio(model_key, preview_text_for_voice(model_key, preview_locale))
    }
}

fn synthesize_nano_ai_tts_audio(role_id: &str, text: &str) -> Result<Vec<u8>, String> {
    let mut url = reqwest::Url::parse(&format!("{NANO_AI_TTS_BASE_URL}/api/tts/v1"))
        .map_err(|error| format!("无法创建纳米AI TTS 请求地址: {error}"))?;
    url.query_pairs_mut().append_pair("roleid", role_id);

    let response = nano_ai_client()?
        .post(url)
        .headers(nano_ai_headers()?)
        .form(&[("text", text), ("audio_type", "mp3"), ("format", "stream")])
        .send()
        .map_err(|error| format!("无法获取纳米AI TTS 试听音频: {error}"))?;
    let status = response.status();
    let audio = response
        .bytes()
        .map_err(|error| format!("无法读取纳米AI TTS 试听音频: {error}"))?;

    if !status.is_success() {
        return Err(format!("纳米AI TTS 请求失败: HTTP {status}"));
    }

    if audio.is_empty() {
        return Err("纳米AI TTS 未返回试听音频".to_string());
    }

    if looks_like_json_response(&audio) {
        let message = String::from_utf8_lossy(&audio);
        return Err(format!(
            "纳米AI TTS 返回异常: {}",
            truncate_response_text(&message)
        ));
    }

    Ok(audio.to_vec())
}

fn nano_ai_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent(NANO_AI_TTS_USER_AGENT)
        .build()
        .map_err(|error| format!("无法创建纳米AI TTS 客户端: {error}"))
}

fn nano_ai_headers() -> Result<HeaderMap, String> {
    let device = "Web";
    let version = "1.2";
    let timestamp = nano_ai_timestamp();
    let access_token = nano_ai_mid();
    let zm_ua = md5_hex(NANO_AI_TTS_USER_AGENT);
    let zm_token = md5_hex(&format!(
        "{device}{timestamp}{version}{access_token}{zm_ua}"
    ));

    let mut headers = HeaderMap::new();
    headers.insert("device-platform", HeaderValue::from_static(device));
    headers.insert("timestamp", header_value(&timestamp)?);
    headers.insert("access-token", header_value(&access_token)?);
    headers.insert("zm-token", header_value(&zm_token)?);
    headers.insert("zm-ver", HeaderValue::from_static(version));
    headers.insert("zm-ua", header_value(&zm_ua)?);
    headers.insert(USER_AGENT, HeaderValue::from_static(NANO_AI_TTS_USER_AGENT));

    Ok(headers)
}

fn header_value(value: &str) -> Result<HeaderValue, String> {
    HeaderValue::from_str(value).map_err(|error| format!("无法生成请求头: {error}"))
}

fn md5_hex(message: &str) -> String {
    format!("{:x}", md5::compute(message.as_bytes()))
}

fn nano_ai_timestamp() -> String {
    let offset = FixedOffset::east_opt(8 * 60 * 60).expect("valid fixed offset");
    Utc::now()
        .with_timezone(&offset)
        .format("%Y-%m-%dT%H:%M:%S+08:00")
        .to_string()
}

fn nano_ai_mid() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let timestamp = millis as f64 + random_fraction() + random_fraction();
    let raw = format!(
        "{}{}{}",
        nano_ai_hash(NANO_AI_TTS_BASE_URL),
        nano_ai_unique_hash(),
        format!("{timestamp:.6}")
    );

    raw.replace('.', "e").chars().take(32).collect()
}

fn nano_ai_unique_hash() -> u64 {
    let language = "zh-CN";
    let app_name = "chrome";
    let version = "1";
    let platform = "Win32";
    let width = 1920;
    let height = 1080;
    let color_depth = 24;
    let referrer = "https://bot.n.cn/chat";
    let mut value = format!(
        "{app_name}{version}.0{language}{platform}{NANO_AI_TTS_USER_AGENT}{width}x{height}{color_depth}{referrer}"
    );
    let mut length = value.chars().count() as u64;
    let mut index = 1_u64;

    while index != 0 {
        value.push_str(&(index ^ length).to_string());
        index -= 1;
        length += 1;
    }

    (random_u31() ^ nano_ai_hash(&value)) * 2_147_483_647
}

fn nano_ai_hash(value: &str) -> u64 {
    const HASH_MASK_1: u64 = 268_435_455;
    const HASH_MASK_2: u64 = 266_338_304;

    let mut result = 0_u64;
    for character in value.chars().rev() {
        let code = character as u64;
        result = ((result << 6) & HASH_MASK_1) + code + (code << 14);
        let overflow = result & HASH_MASK_2;
        if overflow != 0 {
            result ^= overflow >> 21;
        }
    }

    result
}

fn random_u31() -> u64 {
    (Uuid::new_v4().as_u128() % 2_147_483_648) as u64
}

fn random_fraction() -> f64 {
    (Uuid::new_v4().as_u128() % 1_000_000) as f64 / 1_000_000.0
}

fn looks_like_json_response(data: &[u8]) -> bool {
    data.iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
        .is_some_and(|byte| matches!(byte, b'{' | b'['))
}

fn truncate_response_text(text: &str) -> String {
    const MAX_LEN: usize = 160;
    let value = text.trim();
    if value.chars().count() <= MAX_LEN {
        return value.to_string();
    }

    format!("{}...", value.chars().take(MAX_LEN).collect::<String>())
}

fn synthesize_edge_tts_audio(model_key: &str, text: &str) -> Result<Vec<u8>, String> {
    let connection_id = connect_id();
    let request_url = format!(
        "wss://{EDGE_TTS_BASE_URL}/edge/v1?TrustedClientToken={EDGE_TTS_TRUSTED_CLIENT_TOKEN}&ConnectionId={connection_id}&Sec-MS-GEC={}&Sec-MS-GEC-Version={EDGE_TTS_SEC_MS_GEC_VERSION}",
        generate_sec_ms_gec()?
    );
    let mut request = request_url
        .into_client_request()
        .map_err(|error| format!("无法创建 EDGE-TTS WebSocket 请求: {error}"))?;

    {
        let headers = request.headers_mut();
        headers.insert("Pragma", "no-cache".parse().unwrap());
        headers.insert("Cache-Control", "no-cache".parse().unwrap());
        headers.insert(
            "Origin",
            "chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold"
                .parse()
                .unwrap(),
        );
        headers.insert("Sec-WebSocket-Version", "13".parse().unwrap());
        headers.insert("User-Agent", edge_user_agent().parse().unwrap());
        headers.insert(
            "Accept-Encoding",
            "gzip, deflate, br, zstd".parse().unwrap(),
        );
        headers.insert("Accept-Language", "en-US,en;q=0.9".parse().unwrap());
        headers.insert(
            "Cookie",
            format!("muid={};", generate_muid()).parse().unwrap(),
        );
    }

    let (mut socket, _) =
        connect(request).map_err(|error| format!("无法连接 EDGE-TTS 试听服务: {error}"))?;
    let timestamp = edge_date_string();

    socket
        .send(Message::Text(edge_speech_config_message().into()))
        .map_err(|error| format!("无法发送 EDGE-TTS 音频配置: {error}"))?;
    socket
        .send(Message::Text(
            edge_ssml_message(model_key, text, &timestamp).into(),
        ))
        .map_err(|error| format!("无法发送 EDGE-TTS 试听文本: {error}"))?;

    let mut audio = Vec::new();

    loop {
        let message = socket
            .read()
            .map_err(|error| format!("读取 EDGE-TTS 试听音频失败: {error}"))?;

        match message {
            Message::Binary(data) => {
                let chunk = parse_edge_audio_message(&data)?;
                audio.extend_from_slice(chunk);
            }
            Message::Text(text) => {
                if is_edge_turn_end(text.as_ref())? {
                    break;
                }
            }
            Message::Close(_) => break,
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
        }
    }

    if audio.is_empty() {
        return Err("EDGE-TTS 未返回试听音频".to_string());
    }

    Ok(audio)
}

fn edge_speech_config_message() -> String {
    format!(
        "X-Timestamp:{}\r\nContent-Type:application/json; charset=utf-8\r\nPath:speech.config\r\n\r\n{{\"context\":{{\"synthesis\":{{\"audio\":{{\"metadataoptions\":{{\"sentenceBoundaryEnabled\":\"true\",\"wordBoundaryEnabled\":\"false\"}},\"outputFormat\":\"audio-24khz-48kbitrate-mono-mp3\"}}}}}}}}\r\n",
        edge_date_string()
    )
}

fn edge_ssml_message(model_key: &str, text: &str, timestamp: &str) -> String {
    let cleaned_text = remove_incompatible_characters(text);
    let escaped_text = html_escape::encode_text(&cleaned_text);
    let ssml = format!(
        "<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='en-US'><voice name='{model_key}'><prosody pitch='+0Hz' rate='+0%' volume='+0%'>{escaped_text}</prosody></voice></speak>"
    );

    format!(
        "X-RequestId:{}\r\nContent-Type:application/ssml+xml\r\nX-Timestamp:{timestamp}Z\r\nPath:ssml\r\n\r\n{ssml}",
        connect_id()
    )
}

fn parse_edge_audio_message(data: &[u8]) -> Result<&[u8], String> {
    if data.len() < 2 {
        return Err("EDGE-TTS 音频响应缺少头部长度".to_string());
    }

    let header_length = u16::from_be_bytes([data[0], data[1]]) as usize;
    if header_length > data.len().saturating_sub(2) {
        return Err("EDGE-TTS 音频响应头部异常".to_string());
    }

    let header_start = 2;
    let header_end = header_start + header_length;
    let audio_start = header_end;
    let headers = parse_edge_headers(&data[header_start..header_end])?;

    if headers
        .iter()
        .any(|(key, value)| key == "Path" && value == "audio")
    {
        if audio_start > data.len() {
            return Ok(&[]);
        }

        return Ok(&data[audio_start..]);
    }

    Ok(&[])
}

fn is_edge_turn_end(data: &str) -> Result<bool, String> {
    let header_end = data
        .find("\r\n\r\n")
        .ok_or_else(|| "EDGE-TTS 文本响应格式异常".to_string())?;
    let headers = parse_edge_headers(data[..header_end].as_bytes())?;

    Ok(headers
        .iter()
        .any(|(key, value)| key == "Path" && value == "turn.end"))
}

fn parse_edge_headers(data: &[u8]) -> Result<Vec<(String, String)>, String> {
    let text = String::from_utf8_lossy(data);
    let mut headers = Vec::new();

    for line in text.split("\r\n").filter(|line| !line.is_empty()) {
        let Some((key, value)) = line.split_once(':') else {
            return Err("EDGE-TTS 响应头格式异常".to_string());
        };
        headers.push((key.to_string(), value.to_string()));
    }

    Ok(headers)
}

fn generate_sec_ms_gec() -> Result<String, String> {
    let unix_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("系统时间异常，无法生成 EDGE-TTS 请求签名: {error}"))?
        .as_secs();
    let windows_seconds = unix_seconds + WINDOWS_EPOCH_SECONDS;
    let rounded_seconds = windows_seconds - (windows_seconds % 300);
    let ticks = rounded_seconds * 10_000_000;
    let mut hasher = Sha256::new();
    hasher.update(format!("{ticks}{EDGE_TTS_TRUSTED_CLIENT_TOKEN}").as_bytes());

    Ok(format!("{:X}", hasher.finalize()))
}

fn edge_voice_headers() -> reqwest::header::HeaderMap {
    use reqwest::header::{HeaderMap, HeaderValue};

    let mut headers = HeaderMap::new();
    headers.insert(
        "Authority",
        HeaderValue::from_static("speech.platform.bing.com"),
    );
    headers.insert(
        "Sec-CH-UA",
        HeaderValue::from_str(&format!(
            "\" Not;A Brand\";v=\"99\", \"Microsoft Edge\";v=\"{EDGE_TTS_CHROMIUM_MAJOR_VERSION}\", \"Chromium\";v=\"{EDGE_TTS_CHROMIUM_MAJOR_VERSION}\""
        ))
        .unwrap(),
    );
    headers.insert("Sec-CH-UA-Mobile", HeaderValue::from_static("?0"));
    headers.insert("Accept", HeaderValue::from_static("*/*"));
    headers.insert("Sec-Fetch-Site", HeaderValue::from_static("none"));
    headers.insert("Sec-Fetch-Mode", HeaderValue::from_static("cors"));
    headers.insert("Sec-Fetch-Dest", HeaderValue::from_static("empty"));
    headers.insert(
        "User-Agent",
        HeaderValue::from_str(&edge_user_agent()).unwrap(),
    );
    headers.insert("Accept-Encoding", HeaderValue::from_static("identity"));
    headers.insert(
        "Accept-Language",
        HeaderValue::from_static("en-US,en;q=0.9"),
    );
    headers.insert(
        "Cookie",
        HeaderValue::from_str(&format!("muid={};", generate_muid())).unwrap(),
    );

    headers
}

fn edge_user_agent() -> String {
    format!(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{EDGE_TTS_CHROMIUM_MAJOR_VERSION}.0.0.0 Safari/537.36 Edg/{EDGE_TTS_CHROMIUM_MAJOR_VERSION}.0.0.0"
    )
}

fn edge_date_string() -> String {
    Utc::now()
        .format("%a %b %d %Y %H:%M:%S GMT+0000 (Coordinated Universal Time)")
        .to_string()
}

fn connect_id() -> String {
    Uuid::new_v4().simple().to_string()
}

fn generate_muid() -> String {
    Uuid::new_v4().simple().to_string().to_uppercase()
}

fn remove_incompatible_characters(text: &str) -> String {
    text.chars()
        .map(|character| {
            let code = character as u32;
            if (0..=8).contains(&code) || (11..=12).contains(&code) || (14..=31).contains(&code) {
                ' '
            } else {
                character
            }
        })
        .collect()
}

fn preview_text_for_voice(model_key: &str, locale: Option<&str>) -> &'static str {
    let language = locale
        .and_then(|value| value.split('-').next())
        .filter(|value| !value.is_empty())
        .or_else(|| model_key.split('-').next())
        .unwrap_or("en");

    match language {
        "ar" => "مرحبا، هذه معاينة للصوت.",
        "da" => "Hej, dette er en stemmeforhåndsvisning.",
        "de" => "Hallo, dies ist eine Stimmprobe.",
        "el" => "Γεια σας, αυτή είναι μια προεπισκόπηση φωνής.",
        "en" => "Hello, this is a voice preview.",
        "es" => "Hola, esta es una vista previa de voz.",
        "fi" => "Hei, tämä on äänen esikatselu.",
        "fr" => "Bonjour, ceci est un aperçu de la voix.",
        "he" => "שלום, זו תצוגה מקדימה של הקול.",
        "hi" => "नमस्ते, यह आवाज़ का पूर्वावलोकन है।",
        "id" => "Halo, ini adalah pratinjau suara.",
        "it" => "Ciao, questa è un'anteprima della voce.",
        "ja" => "こんにちは、これは音声プレビューです。",
        "ko" => "안녕하세요. 음성 미리 듣기입니다.",
        "nb" | "nn" | "no" => "Hei, dette er en forhåndsvisning av stemmen.",
        "nl" => "Hallo, dit is een stemvoorbeeld.",
        "pl" => "Cześć, to jest podgląd głosu.",
        "pt" => "Olá, esta é uma prévia de voz.",
        "ru" => "Здравствуйте, это предварительное прослушивание голоса.",
        "sv" => "Hej, det här är en röstförhandsvisning.",
        "th" => "สวัสดี นี่คือตัวอย่างเสียง",
        "tr" => "Merhaba, bu bir ses önizlemesidir.",
        "vi" => "Xin chào, đây là bản nghe thử giọng nói.",
        "zh" => "你好，这是一段配音试听。",
        _ => "1 2 3 4 5.",
    }
}
