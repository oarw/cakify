//! Provider adapters. Raw response bodies, authorization headers, and secrets
//! never cross this crate's public boundary.

use std::{
    collections::HashSet,
    io::{BufRead, BufReader},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
    time::Duration,
};

use cakify_core::{
    ChatProvider, ChatRequest, ChatRole, ProviderError, ProviderErrorKind, ProviderStreamEvent,
    SecretId, SecretStore, StreamSink, Usage,
};
use reqwest::{
    blocking::Client,
    header::{HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    redirect::Policy,
    StatusCode,
};
use serde_json::{json, Map, Value};
use url::Url;
use zeroize::Zeroizing;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(180);
const MAX_SSE_LINE_BYTES: usize = 1_048_576;
const MAX_SSE_EVENT_BYTES: usize = 1_048_576;
const MAX_STREAM_BYTES: usize = 32 * 1_048_576;
const MAX_STREAM_EVENTS: usize = 131_072;
const MAX_REQUEST_BODY_BYTES: usize = 32 * 1_048_576;
const MAX_MODEL_ID_BYTES: usize = 256;
const MAX_MESSAGES: usize = 4_096;
const MAX_MESSAGE_CONTENT_BYTES: usize = 16 * 1_048_576;
const MAX_TOOLS: usize = 128;
const MAX_TOOL_NAME_BYTES: usize = 64;
const MAX_TOOL_DESCRIPTION_BYTES: usize = 4_096;
const MAX_TOOL_SCHEMA_BYTES: usize = 1_048_576;
const MAX_TOOL_CALL_ID_BYTES: usize = 256;
const MAX_TOOL_ARGUMENT_BYTES: usize = 1_048_576;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenAiConfig {
    endpoint: Url,
    credential_id: Option<SecretId>,
}

impl OpenAiConfig {
    pub fn new(
        endpoint: impl AsRef<str>,
        credential_id: Option<SecretId>,
    ) -> Result<Self, ProviderError> {
        let endpoint = normalize_endpoint(endpoint.as_ref())?;
        Ok(Self {
            endpoint,
            credential_id,
        })
    }

    pub fn endpoint(&self) -> &Url {
        &self.endpoint
    }

    pub fn credential_id(&self) -> Option<&SecretId> {
        self.credential_id.as_ref()
    }
}

pub struct OpenAiCompatibleProvider {
    config: OpenAiConfig,
    client: Client,
    secrets: Arc<dyn SecretStore>,
}

#[derive(Default)]
pub struct ProviderRouter {
    provider: RwLock<Option<Arc<dyn ChatProvider>>>,
}

impl ProviderRouter {
    pub fn set(&self, provider: Arc<dyn ChatProvider>) {
        if let Ok(mut current) = self.provider.write() {
            *current = Some(provider);
        }
    }

    pub fn clear(&self) {
        if let Ok(mut current) = self.provider.write() {
            *current = None;
        }
    }

    pub fn is_configured(&self) -> bool {
        self.provider
            .read()
            .is_ok_and(|provider| provider.is_some())
    }
}

impl ChatProvider for ProviderRouter {
    fn stream(
        &self,
        request: ChatRequest,
        cancellation: Arc<AtomicBool>,
        sink: &mut StreamSink<'_>,
    ) -> Result<(), ProviderError> {
        let provider = self
            .provider
            .read()
            .map_err(|_| {
                ProviderError::new(ProviderErrorKind::NotConfigured, "Provider 配置暂时不可用")
            })?
            .clone()
            .ok_or_else(|| {
                ProviderError::new(
                    ProviderErrorKind::NotConfigured,
                    "尚未配置可用的模型 Provider",
                )
            })?;
        provider.stream(request, cancellation, sink)
    }
}

impl OpenAiCompatibleProvider {
    pub fn new(config: OpenAiConfig, secrets: Arc<dyn SecretStore>) -> Result<Self, ProviderError> {
        let client = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .redirect(Policy::none())
            .user_agent("Cakify/0.1")
            .build()
            .map_err(|_| {
                ProviderError::new(ProviderErrorKind::Transport, "无法初始化安全网络客户端")
            })?;
        Ok(Self {
            config,
            client,
            secrets,
        })
    }

    fn request_body(request: &ChatRequest) -> Result<Value, ProviderError> {
        let model = request.model.trim();
        if model.is_empty() {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "模型 ID 不能为空",
            ));
        }
        if model.len() > MAX_MODEL_ID_BYTES {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "模型 ID 过长",
            ));
        }
        if request.messages.is_empty() {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "请求至少需要一条消息",
            ));
        }
        if request.messages.len() > MAX_MESSAGES {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "请求包含过多消息",
            ));
        }
        let mut payload_bytes = validate_messages(request)?;
        if request.tools.len() > MAX_TOOLS {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "请求包含过多工具定义",
            ));
        }
        for tool in &request.tools {
            payload_bytes = payload_bytes
                .checked_add(tool.name.len())
                .and_then(|bytes| bytes.checked_add(tool.description.len()))
                .and_then(|bytes| bytes.checked_add(tool.parameters_json.len()))
                .ok_or_else(request_too_large)?;
            if payload_bytes > MAX_REQUEST_BODY_BYTES {
                return Err(request_too_large());
            }
        }

        let mut body = Map::new();
        body.insert("model".to_owned(), Value::String(model.to_owned()));
        body.insert("stream".to_owned(), Value::Bool(true));
        body.insert(
            "stream_options".to_owned(),
            json!({ "include_usage": true }),
        );
        body.insert(
            "messages".to_owned(),
            serde_json::to_value(&request.messages).map_err(|_| {
                ProviderError::new(ProviderErrorKind::InvalidRequest, "无法编码聊天消息")
            })?,
        );
        if let Some(temperature) = request.temperature {
            if !temperature.is_finite() || !(0.0..=2.0).contains(&temperature) {
                return Err(ProviderError::new(
                    ProviderErrorKind::InvalidRequest,
                    "temperature 必须在 0 到 2 之间",
                ));
            }
            body.insert("temperature".to_owned(), json!(temperature));
        }
        if !request.tools.is_empty() {
            let mut names = HashSet::with_capacity(request.tools.len());
            let tools = request
                .tools
                .iter()
                .map(|tool| {
                    validate_tool_name(&tool.name)?;
                    if !names.insert(tool.name.as_str()) {
                        return Err(ProviderError::new(
                            ProviderErrorKind::InvalidRequest,
                            format!("工具 {} 重复定义", tool.name),
                        ));
                    }
                    if tool.description.len() > MAX_TOOL_DESCRIPTION_BYTES {
                        return Err(ProviderError::new(
                            ProviderErrorKind::InvalidRequest,
                            format!("工具 {} 的说明过长", tool.name),
                        ));
                    }
                    if tool.parameters_json.len() > MAX_TOOL_SCHEMA_BYTES {
                        return Err(ProviderError::new(
                            ProviderErrorKind::InvalidRequest,
                            format!("工具 {} 的参数 Schema 过大", tool.name),
                        ));
                    }
                    let parameters: Value =
                        serde_json::from_str(&tool.parameters_json).map_err(|_| {
                            ProviderError::new(
                                ProviderErrorKind::InvalidRequest,
                                format!("工具 {} 的参数 Schema 不是有效 JSON", tool.name),
                            )
                        })?;
                    if !parameters.is_object() {
                        return Err(ProviderError::new(
                            ProviderErrorKind::InvalidRequest,
                            format!("工具 {} 的参数 Schema 必须是 JSON object", tool.name),
                        ));
                    }
                    Ok(json!({
                        "type": "function",
                        "function": {
                            "name": tool.name,
                            "description": tool.description,
                            "parameters": parameters,
                        }
                    }))
                })
                .collect::<Result<Vec<_>, ProviderError>>()?;
            body.insert("tools".to_owned(), Value::Array(tools));
        }
        let body = Value::Object(body);
        let encoded_len = serde_json::to_vec(&body)
            .map_err(|_| {
                ProviderError::new(ProviderErrorKind::InvalidRequest, "无法编码聊天请求")
            })?
            .len();
        if encoded_len > MAX_REQUEST_BODY_BYTES {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "聊天请求超过大小上限",
            ));
        }
        Ok(body)
    }

    fn authorization(&self) -> Result<Option<HeaderValue>, ProviderError> {
        let Some(credential_id) = &self.config.credential_id else {
            return Ok(None);
        };
        let secret = self
            .secrets
            .get(credential_id)
            .map_err(|error| match error {
                cakify_core::SecretError::NotFound { .. } => ProviderError::new(
                    ProviderErrorKind::Authentication,
                    "Provider 的 API Key 不存在",
                ),
                _ => ProviderError::new(
                    ProviderErrorKind::Authentication,
                    "无法从 Windows 安全存储读取 API Key",
                ),
            })?;

        let mut value = Zeroizing::new(Vec::with_capacity(7 + secret.expose_secret().len()));
        value.extend_from_slice(b"Bearer ");
        value.extend_from_slice(secret.expose_secret());
        let mut header = HeaderValue::from_bytes(&value).map_err(|_| {
            ProviderError::new(
                ProviderErrorKind::Authentication,
                "API Key 包含 HTTP Header 不允许的字符",
            )
        })?;
        header.set_sensitive(true);
        Ok(Some(header))
    }
}

fn validate_messages(request: &ChatRequest) -> Result<usize, ProviderError> {
    let mut payload_bytes = 0_usize;
    for message in &request.messages {
        if message.content.len() > MAX_MESSAGE_CONTENT_BYTES {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "单条消息超过大小上限",
            ));
        }
        match message.role {
            ChatRole::System | ChatRole::User
                if !message.tool_calls.is_empty() || message.tool_call_id.is_some() =>
            {
                return Err(ProviderError::new(
                    ProviderErrorKind::InvalidRequest,
                    "普通消息不能携带工具调用字段",
                ));
            }
            ChatRole::Assistant if message.tool_call_id.is_some() => {
                return Err(ProviderError::new(
                    ProviderErrorKind::InvalidRequest,
                    "助手消息不能携带工具结果 ID",
                ));
            }
            ChatRole::Tool if !message.tool_calls.is_empty() || message.tool_call_id.is_none() => {
                return Err(ProviderError::new(
                    ProviderErrorKind::InvalidRequest,
                    "工具结果消息缺少调用 ID",
                ));
            }
            _ => {}
        }
        payload_bytes = payload_bytes
            .checked_add(message.content.len())
            .ok_or_else(request_too_large)?;
        if let Some(tool_call_id) = &message.tool_call_id {
            payload_bytes = payload_bytes
                .checked_add(tool_call_id.len())
                .ok_or_else(request_too_large)?;
            if tool_call_id.is_empty() || tool_call_id.len() > MAX_TOOL_CALL_ID_BYTES {
                return Err(ProviderError::new(
                    ProviderErrorKind::InvalidRequest,
                    "工具结果包含无效的调用 ID",
                ));
            }
        }
        for tool_call in &message.tool_calls {
            payload_bytes = payload_bytes
                .checked_add(tool_call.id.len())
                .and_then(|bytes| bytes.checked_add(tool_call.kind.len()))
                .and_then(|bytes| bytes.checked_add(tool_call.function.name.len()))
                .and_then(|bytes| bytes.checked_add(tool_call.function.arguments.len()))
                .ok_or_else(request_too_large)?;
            if tool_call.id.is_empty() || tool_call.id.len() > MAX_TOOL_CALL_ID_BYTES {
                return Err(ProviderError::new(
                    ProviderErrorKind::InvalidRequest,
                    "助手工具调用包含无效的调用 ID",
                ));
            }
            if tool_call.kind != "function" {
                return Err(ProviderError::new(
                    ProviderErrorKind::InvalidRequest,
                    "助手工具调用类型无效",
                ));
            }
            validate_tool_name(&tool_call.function.name)?;
            if tool_call.function.arguments.len() > MAX_TOOL_ARGUMENT_BYTES {
                return Err(ProviderError::new(
                    ProviderErrorKind::InvalidRequest,
                    "助手工具调用参数过大",
                ));
            }
            let arguments: Value = serde_json::from_str(&tool_call.function.arguments).map_err(|_| {
                ProviderError::new(
                    ProviderErrorKind::InvalidRequest,
                    "助手工具调用参数不是有效 JSON",
                )
            })?;
            if !arguments.is_object() {
                return Err(ProviderError::new(
                    ProviderErrorKind::InvalidRequest,
                    "助手工具调用参数必须是 JSON object",
                ));
            }
        }
        if payload_bytes > MAX_REQUEST_BODY_BYTES {
            return Err(request_too_large());
        }
    }
    Ok(payload_bytes)
}

fn validate_tool_name(name: &str) -> Result<(), ProviderError> {
    if name.is_empty()
        || name.len() > MAX_TOOL_NAME_BYTES
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(ProviderError::new(
            ProviderErrorKind::InvalidRequest,
            "工具名称只能包含 1-64 个 ASCII 字母、数字、下划线或连字符",
        ));
    }
    Ok(())
}

impl ChatProvider for OpenAiCompatibleProvider {
    fn stream(
        &self,
        request: ChatRequest,
        cancellation: Arc<AtomicBool>,
        sink: &mut StreamSink<'_>,
    ) -> Result<(), ProviderError> {
        if cancellation.load(Ordering::Acquire) {
            return Err(cancelled());
        }

        let body = Self::request_body(&request)?;
        let mut request = self
            .client
            .post(self.config.endpoint.clone())
            .header(ACCEPT, "text/event-stream")
            .header(CONTENT_TYPE, "application/json")
            .json(&body);
        if let Some(authorization) = self.authorization()? {
            request = request.header(AUTHORIZATION, authorization);
        }

        let response = request.send().map_err(map_request_error)?;
        let status = response.status();
        if !status.is_success() {
            return Err(map_status(status));
        }

        consume_sse(BufReader::new(response), cancellation, sink)
    }
}

fn normalize_endpoint(endpoint: &str) -> Result<Url, ProviderError> {
    let mut url = Url::parse(endpoint.trim()).map_err(|_| {
        ProviderError::new(
            ProviderErrorKind::InvalidRequest,
            "Provider endpoint 不是有效 URL",
        )
    })?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(ProviderError::new(
            ProviderErrorKind::InvalidRequest,
            "Provider endpoint 不能包含认证信息、query 或 fragment",
        ));
    }
    match url.scheme() {
        "https" => {}
        "http" if is_loopback(url.host_str()) => {}
        _ => {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "远程 Provider endpoint 必须使用 HTTPS",
            ));
        }
    }

    let path = url.path().trim_end_matches('/');
    let normalized_path = if path.ends_with("/chat/completions") {
        path.to_owned()
    } else if path.ends_with("/v1") {
        format!("{path}/chat/completions")
    } else if path.is_empty() {
        "/v1/chat/completions".to_owned()
    } else {
        format!("{path}/v1/chat/completions")
    };
    url.set_path(&normalized_path);
    Ok(url)
}

fn is_loopback(host: Option<&str>) -> bool {
    matches!(host, Some("localhost" | "127.0.0.1" | "[::1]" | "::1"))
}

fn map_request_error(error: reqwest::Error) -> ProviderError {
    let message = if error.is_timeout() {
        "模型请求超时"
    } else if error.is_connect() {
        "无法连接模型 Provider"
    } else if error.is_body() || error.is_decode() {
        "读取模型响应失败"
    } else {
        "模型网络请求失败"
    };
    ProviderError::new(ProviderErrorKind::Transport, message)
}

fn map_status(status: StatusCode) -> ProviderError {
    let (kind, message) = match status.as_u16() {
        401 | 403 => (ProviderErrorKind::Authentication, "Provider 拒绝了 API Key"),
        429 => (
            ProviderErrorKind::RateLimited,
            "Provider 请求过于频繁，请稍后重试",
        ),
        400..=499 => (ProviderErrorKind::InvalidRequest, "Provider 拒绝了聊天请求"),
        _ => (ProviderErrorKind::Transport, "Provider 服务暂时不可用"),
    };
    ProviderError::new(kind, message)
}

fn consume_sse<R: BufRead>(
    mut reader: R,
    cancellation: Arc<AtomicBool>,
    sink: &mut StreamSink<'_>,
) -> Result<(), ProviderError> {
    let mut line = Vec::new();
    let mut data = String::new();
    let mut total_bytes = 0_usize;
    let mut event_count = 0_usize;
    let mut saw_terminal_event = false;
    let mut first_line = true;
    let mut skip_lf_after_cr = false;
    loop {
        if cancellation.load(Ordering::Acquire) {
            return Err(cancelled());
        }
        let Some(read) = read_bounded_line(&mut reader, &mut line, &mut skip_lf_after_cr)? else {
            if !data.is_empty() {
                if data.trim() == "[DONE]" {
                    return Ok(());
                }
                saw_terminal_event |= dispatch_sse_data(&data, sink)?;
            }
            return if saw_terminal_event {
                Ok(())
            } else {
                Err(ProviderError::new(
                    ProviderErrorKind::Protocol,
                    "Provider 流式响应意外结束",
                ))
            };
        };
        total_bytes = total_bytes.checked_add(read).ok_or_else(stream_too_large)?;
        if total_bytes > MAX_STREAM_BYTES {
            return Err(stream_too_large());
        }
        let line = std::str::from_utf8(&line).map_err(|_| {
            ProviderError::new(
                ProviderErrorKind::Protocol,
                "Provider 返回了非 UTF-8 流式事件",
            )
        })?;
        let line = line.trim_end_matches(&['\r', '\n'][..]);
        let line = if first_line {
            first_line = false;
            line.strip_prefix('\u{feff}').unwrap_or(line)
        } else {
            line
        };
        if line.is_empty() {
            if !data.is_empty() {
                if data.trim() == "[DONE]" {
                    return Ok(());
                }
                event_count = event_count.checked_add(1).ok_or_else(too_many_events)?;
                if event_count > MAX_STREAM_EVENTS {
                    return Err(too_many_events());
                }
                saw_terminal_event |= dispatch_sse_data(&data, sink)?;
                data.clear();
            }
            continue;
        }
        if line.starts_with(':') {
            continue;
        }
        if let Some(value) = line.strip_prefix("data:") {
            let value = value.strip_prefix(' ').unwrap_or(value);
            let separator_bytes = usize::from(!data.is_empty());
            let next_len = data
                .len()
                .checked_add(separator_bytes)
                .and_then(|len| len.checked_add(value.len()))
                .ok_or_else(event_too_large)?;
            if next_len > MAX_SSE_EVENT_BYTES {
                return Err(event_too_large());
            }
            if separator_bytes != 0 {
                data.push('\n');
            }
            data.push_str(value);
        }
    }
}

fn read_bounded_line<R: BufRead>(
    reader: &mut R,
    line: &mut Vec<u8>,
    skip_lf_after_cr: &mut bool,
) -> Result<Option<usize>, ProviderError> {
    line.clear();
    let mut total_read = 0_usize;
    loop {
        let buffer = reader.fill_buf().map_err(|_| {
            ProviderError::new(ProviderErrorKind::Transport, "读取 Provider 流式响应失败")
        })?;
        if buffer.is_empty() {
            return Ok((!line.is_empty()).then_some(total_read));
        }
        if *skip_lf_after_cr {
            *skip_lf_after_cr = false;
            if buffer.first() == Some(&b'\n') {
                reader.consume(1);
                total_read += 1;
                continue;
            }
        }
        let consumed = buffer
            .iter()
            .position(|byte| matches!(*byte, b'\r' | b'\n'))
            .map_or(buffer.len(), |position| position + 1);
        let ended_with_cr = buffer.get(consumed.saturating_sub(1)) == Some(&b'\r');
        let next_len = line
            .len()
            .checked_add(consumed)
            .ok_or_else(event_too_large)?;
        if next_len > MAX_SSE_LINE_BYTES {
            return Err(event_too_large());
        }
        line.extend_from_slice(&buffer[..consumed]);
        reader.consume(consumed);
        total_read = total_read
            .checked_add(consumed)
            .ok_or_else(stream_too_large)?;
        if line
            .last()
            .is_some_and(|byte| matches!(*byte, b'\r' | b'\n'))
        {
            *skip_lf_after_cr = ended_with_cr;
            return Ok(Some(total_read));
        }
    }
}

fn dispatch_sse_data(
    data: &str,
    sink: &mut StreamSink<'_>,
) -> Result<bool, ProviderError> {
    let payload: Value = serde_json::from_str(data).map_err(|_| {
        ProviderError::new(
            ProviderErrorKind::Protocol,
            "Provider 返回了无法解析的流式事件",
        )
    })?;
    if !payload.is_object() {
        return Err(ProviderError::new(
            ProviderErrorKind::Protocol,
            "Provider 返回了无效的流式事件结构",
        ));
    }
    if let Some(error) = payload.get("error").filter(|value| !value.is_null()) {
        return Err(map_stream_error(error));
    }

    if let Some(usage) = payload.get("usage").filter(|value| !value.is_null()) {
        if !sink(ProviderStreamEvent::Usage(Usage {
            input_tokens: usage
                .get("prompt_tokens")
                .and_then(Value::as_u64)
                .or_else(|| usage.get("input_tokens").and_then(Value::as_u64)),
            output_tokens: usage
                .get("completion_tokens")
                .and_then(Value::as_u64)
                .or_else(|| usage.get("output_tokens").and_then(Value::as_u64)),
            total_tokens: usage.get("total_tokens").and_then(Value::as_u64),
        })) {
            return Err(cancelled());
        }
    }

    let choices = match payload.get("choices") {
        Some(Value::Array(choices)) => choices,
        Some(Value::Null) | None => return Ok(false),
        Some(_) => {
            return Err(ProviderError::new(
                ProviderErrorKind::Protocol,
                "Provider 返回了无效的 choices 字段",
            ))
        }
    };
    let Some(choice) = choices.first() else {
        return Ok(false);
    };
    let delta = match choice.get("delta") {
        Some(Value::Object(delta)) => Some(delta),
        Some(Value::Null) | None => None,
        Some(_) => {
            return Err(ProviderError::new(
                ProviderErrorKind::Protocol,
                "Provider 返回了无效的文本增量结构",
            ))
        }
    };
    if let Some(content) = delta.and_then(|delta| delta.get("content")) {
        if !content.is_null() {
            let content = content.as_str().ok_or_else(|| {
                ProviderError::new(
                    ProviderErrorKind::Protocol,
                    "Provider 返回了无效的文本增量",
                )
            })?;
            if !content.is_empty() && !sink(ProviderStreamEvent::TextDelta(content.to_owned())) {
                return Err(cancelled());
            }
        }
    }

    let tool_calls = match delta.and_then(|delta| delta.get("tool_calls")) {
        Some(Value::Array(tool_calls)) => Some(tool_calls),
        Some(Value::Null) | None => None,
        Some(_) => {
            return Err(ProviderError::new(
                ProviderErrorKind::Protocol,
                "Provider 返回了无效的工具调用列表",
            ))
        }
    };
    if let Some(tool_calls) = tool_calls {
        for tool_call in tool_calls {
            let tool_call = tool_call.as_object().ok_or_else(|| {
                ProviderError::new(
                    ProviderErrorKind::Protocol,
                    "Provider 返回了无效的工具调用增量",
                )
            })?;
            let index = match tool_call.get("index") {
                None => 0,
                Some(value) => value
                    .as_u64()
                    .and_then(|value| u32::try_from(value).ok())
                    .ok_or_else(|| {
                        ProviderError::new(
                            ProviderErrorKind::Protocol,
                            "Provider 返回了无效的工具调用索引",
                        )
                    })?,
            };
            let function = match tool_call.get("function") {
                Some(Value::Object(function)) => Some(function),
                Some(Value::Null) | None => None,
                Some(_) => {
                    return Err(ProviderError::new(
                        ProviderErrorKind::Protocol,
                        "Provider 返回了无效的工具函数增量",
                    ))
                }
            };
            let arguments_delta = match function.and_then(|value| value.get("arguments")) {
                Some(Value::String(arguments)) => arguments.clone(),
                Some(Value::Null) | None => String::new(),
                Some(_) => {
                    return Err(ProviderError::new(
                        ProviderErrorKind::Protocol,
                        "Provider 返回了无效的工具参数增量",
                    ))
                }
            };
            let id = match tool_call.get("id") {
                Some(Value::String(id)) => Some(id.clone()),
                Some(Value::Null) | None => None,
                Some(_) => {
                    return Err(ProviderError::new(
                        ProviderErrorKind::Protocol,
                        "Provider 返回了无效的工具调用 ID",
                    ))
                }
            };
            let name = match function.and_then(|value| value.get("name")) {
                Some(Value::String(name)) => Some(name.clone()),
                Some(Value::Null) | None => None,
                Some(_) => {
                    return Err(ProviderError::new(
                        ProviderErrorKind::Protocol,
                        "Provider 返回了无效的工具名称增量",
                    ))
                }
            };
            if !sink(ProviderStreamEvent::ToolCallDelta {
                index,
                id,
                name,
                arguments_delta,
            }) {
                return Err(cancelled());
            }
        }
    }

    let finished = if let Some(reason) = choice.get("finish_reason") {
        if reason.is_null() {
            false
        } else {
            let reason = reason.as_str().ok_or_else(|| {
                ProviderError::new(
                    ProviderErrorKind::Protocol,
                    "Provider 返回了无效的结束原因",
                )
            })?;
            if !sink(ProviderStreamEvent::Finished {
                reason: Some(reason.to_owned()),
            }) {
                return Err(cancelled());
            }
            true
        }
    } else {
        false
    };
    Ok(finished)
}

fn map_stream_error(error: &Value) -> ProviderError {
    let marker = ["code", "type"]
        .into_iter()
        .filter_map(|field| error.get(field).and_then(Value::as_str))
        .filter(|value| value.len() <= 128)
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join(" ");
    if marker.contains("auth")
        || marker.contains("api_key")
        || marker.contains("unauthorized")
        || marker.contains("permission")
    {
        ProviderError::new(
            ProviderErrorKind::Authentication,
            "Provider 拒绝了 API Key",
        )
    } else if marker.contains("rate") || marker.contains("quota") {
        ProviderError::new(
            ProviderErrorKind::RateLimited,
            "Provider 请求过于频繁或额度不足，请稍后重试",
        )
    } else if marker.contains("invalid")
        || marker.contains("context_length")
        || marker.contains("request")
    {
        ProviderError::new(
            ProviderErrorKind::InvalidRequest,
            "Provider 拒绝了聊天请求",
        )
    } else {
        ProviderError::new(
            ProviderErrorKind::Protocol,
            "Provider 在流式响应中返回了错误",
        )
    }
}

fn event_too_large() -> ProviderError {
    ProviderError::new(
        ProviderErrorKind::Protocol,
        "Provider 返回了过大的流式事件",
    )
}

fn stream_too_large() -> ProviderError {
    ProviderError::new(
        ProviderErrorKind::Protocol,
        "Provider 流式响应超过大小上限",
    )
}

fn too_many_events() -> ProviderError {
    ProviderError::new(
        ProviderErrorKind::Protocol,
        "Provider 返回了过多流式事件",
    )
}

fn request_too_large() -> ProviderError {
    ProviderError::new(
        ProviderErrorKind::InvalidRequest,
        "聊天请求超过大小上限",
    )
}

fn cancelled() -> ProviderError {
    ProviderError::new(ProviderErrorKind::Cancelled, "请求已取消")
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        io::{Cursor, Read, Write},
        net::TcpListener,
        sync::{mpsc, Mutex},
        thread,
        time::Duration,
    };

    use cakify_core::{
        ChatMessage, ChatToolCall, SecretError, SecretInput, SecretValue, ToolDefinition,
    };

    use super::*;

    #[derive(Default)]
    struct MemorySecrets {
        values: Mutex<HashMap<SecretId, Vec<u8>>>,
    }

    impl SecretStore for MemorySecrets {
        fn put(&self, id: &SecretId, value: &SecretInput) -> Result<(), SecretError> {
            self.values
                .lock()
                .expect("secret lock")
                .insert(id.clone(), value.expose_secret().to_vec());
            Ok(())
        }

        fn get(&self, id: &SecretId) -> Result<SecretValue, SecretError> {
            self.values
                .lock()
                .expect("secret lock")
                .get(id)
                .cloned()
                .ok_or_else(|| SecretError::NotFound { id: id.clone() })
                .and_then(SecretValue::from_bytes)
        }

        fn delete(&self, id: &SecretId) -> Result<(), SecretError> {
            self.values.lock().expect("secret lock").remove(id);
            Ok(())
        }

        fn contains(&self, id: &SecretId) -> Result<bool, SecretError> {
            Ok(self.values.lock().expect("secret lock").contains_key(id))
        }
    }

    fn header_end(bytes: &[u8]) -> Option<usize> {
        bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|position| position + 4)
    }

    fn content_length(headers: &[u8]) -> usize {
        String::from_utf8_lossy(headers)
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.eq_ignore_ascii_case("content-length") {
                    value.trim().parse::<usize>().ok()
                } else {
                    None
                }
            })
            .expect("request content-length")
    }

    #[test]
    fn endpoint_requires_https_except_for_loopback() {
        assert_eq!(
            OpenAiConfig::new("https://api.example.test/v1", None)
                .expect("https endpoint")
                .endpoint()
                .as_str(),
            "https://api.example.test/v1/chat/completions"
        );
        assert!(OpenAiConfig::new("http://example.test/v1", None).is_err());
        assert!(OpenAiConfig::new("http://127.0.0.1:11434/v1", None).is_ok());
        assert!(OpenAiConfig::new("https://user:pass@example.test/v1", None).is_err());
    }

    #[test]
    fn sends_real_http_request_and_streams_synthetic_response() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind synthetic provider");
        let address = listener.local_addr().expect("synthetic provider address");
        let (captured_tx, captured_rx) = mpsc::sync_channel(1);
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().expect("accept provider request");
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("request read timeout");
            let mut request = Vec::new();
            let expected_len = loop {
                let mut chunk = [0_u8; 4_096];
                let read = socket.read(&mut chunk).expect("read provider request");
                assert_ne!(read, 0, "request ended before headers");
                request.extend_from_slice(&chunk[..read]);
                if let Some(headers_end) = header_end(&request) {
                    let expected_len = headers_end + content_length(&request[..headers_end]);
                    if request.len() >= expected_len {
                        break expected_len;
                    }
                }
            };
            request.truncate(expected_len);
            captured_tx.send(request).expect("capture provider request");

            let response_body = concat!(
                "data: {\"choices\":[{\"delta\":{\"content\":\"synthetic\"},\"finish_reason\":null}]}\n\n",
                "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":1,\"total_tokens\":3}}\n\n",
                "data: [DONE]\n\n"
            );
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            )
            .expect("write provider response");
        });

        let secret_id = SecretId::new("Cakify/provider/network-test/api-key")
            .expect("synthetic secret id");
        let secrets = Arc::new(MemorySecrets::default());
        secrets
            .put(
                &secret_id,
                &SecretInput::from_utf8("synthetic-network-key").expect("synthetic key"),
            )
            .expect("store synthetic key");
        let provider = OpenAiCompatibleProvider::new(
            OpenAiConfig::new(format!("http://{address}/v1"), Some(secret_id))
                .expect("loopback config"),
            secrets,
        )
        .expect("provider");
        let mut events = Vec::new();
        provider
            .stream(
                ChatRequest {
                    model: "synthetic-model".to_owned(),
                    messages: vec![ChatMessage::user("hello")],
                    tools: Vec::new(),
                    temperature: None,
                },
                Arc::new(AtomicBool::new(false)),
                &mut |event| {
                    events.push(event);
                    true
                },
            )
            .expect("stream synthetic response");
        server.join().expect("synthetic provider server");

        let request = captured_rx.recv().expect("captured request");
        let headers_end = header_end(&request).expect("captured headers");
        let headers = String::from_utf8_lossy(&request[..headers_end]);
        assert!(headers.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"));
        assert!(headers.lines().any(|line| {
            line.eq_ignore_ascii_case("authorization: Bearer synthetic-network-key")
        }));
        let body: Value =
            serde_json::from_slice(&request[headers_end..]).expect("captured JSON request");
        assert_eq!(body["model"], "synthetic-model");
        assert_eq!(body["stream"], true);
        assert_eq!(body["stream_options"]["include_usage"], true);
        assert!(events.iter().any(|event| matches!(
            event,
            ProviderStreamEvent::TextDelta(delta) if delta == "synthetic"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            ProviderStreamEvent::Usage(Usage { total_tokens: Some(3), .. })
        )));
    }

    #[test]
    fn request_body_encodes_tool_round_trip_and_validated_tools() {
        let request = ChatRequest {
            model: "  test-model  ".to_owned(),
            messages: vec![
                ChatMessage::user("hello"),
                ChatMessage::assistant_with_tool_calls(
                    "",
                    vec![ChatToolCall::function(
                        "call-1",
                        "clock",
                        r#"{"city":"上海"}"#,
                    )],
                ),
                ChatMessage::tool("call-1", r#"{"time":"12:00"}"#),
            ],
            tools: vec![ToolDefinition {
                name: "clock".to_owned(),
                description: "read time".to_owned(),
                parameters_json: r#"{"type":"object"}"#.to_owned(),
            }],
            temperature: Some(0.5),
        };
        let body = OpenAiCompatibleProvider::request_body(&request).expect("request body");
        assert_eq!(body["model"], "test-model");
        assert_eq!(body["messages"][0]["role"], "user");
        assert!(body["messages"][0].get("tool_calls").is_none());
        assert_eq!(
            body["messages"][1]["tool_calls"][0]["function"]["arguments"],
            r#"{"city":"上海"}"#
        );
        assert_eq!(body["messages"][2]["role"], "tool");
        assert_eq!(body["messages"][2]["tool_call_id"], "call-1");
        assert_eq!(body["tools"][0]["function"]["name"], "clock");
        assert_eq!(body["stream_options"]["include_usage"], true);
    }

    #[test]
    fn request_body_rejects_invalid_tool_protocol_and_limits() {
        let duplicate = ChatRequest {
            model: "test-model".to_owned(),
            messages: vec![ChatMessage::user("hello")],
            tools: vec![
                ToolDefinition {
                    name: "clock".to_owned(),
                    description: "one".to_owned(),
                    parameters_json: "{}".to_owned(),
                },
                ToolDefinition {
                    name: "clock".to_owned(),
                    description: "two".to_owned(),
                    parameters_json: "{}".to_owned(),
                },
            ],
            temperature: None,
        };
        assert_eq!(
            OpenAiCompatibleProvider::request_body(&duplicate)
                .expect_err("duplicate tool")
                .kind(),
            ProviderErrorKind::InvalidRequest
        );

        let invalid_name = ChatRequest {
            tools: vec![ToolDefinition {
                name: "not valid".to_owned(),
                description: String::new(),
                parameters_json: "{}".to_owned(),
            }],
            ..duplicate.clone()
        };
        assert!(OpenAiCompatibleProvider::request_body(&invalid_name).is_err());

        let invalid_arguments = ChatRequest {
            messages: vec![ChatMessage::assistant_with_tool_calls(
                "",
                vec![ChatToolCall::function("call-1", "clock", "[]")],
            )],
            tools: Vec::new(),
            ..duplicate
        };
        assert!(OpenAiCompatibleProvider::request_body(&invalid_arguments).is_err());
    }

    #[test]
    fn parses_text_tools_usage_and_finish_without_raw_payload_escape() {
        let fixture = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hello \"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call-1\",\"function\":{\"name\":\"clock\",\"arguments\":\"{\\\"\"}}]},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"city\\\":\\\"上海\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}],\"usage\":{\"prompt_tokens\":4,\"completion_tokens\":3,\"total_tokens\":7}}\n\n",
            "data: [DONE]\n\n"
        );
        let mut events = Vec::new();
        consume_sse(
            Cursor::new(fixture),
            Arc::new(AtomicBool::new(false)),
            &mut |event| {
                events.push(event);
                true
            },
        )
        .expect("parse fixture");

        assert!(matches!(
            &events[0],
            ProviderStreamEvent::TextDelta(delta) if delta == "Hello "
        ));
        assert!(events.iter().any(|event| matches!(
            event,
            ProviderStreamEvent::Usage(Usage {
                total_tokens: Some(7),
                ..
            })
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            ProviderStreamEvent::Finished {
                reason: Some(reason)
            } if reason == "tool_calls"
        )));
        let tool_deltas = events
            .iter()
            .filter_map(|event| match event {
                ProviderStreamEvent::ToolCallDelta {
                    index,
                    arguments_delta,
                    ..
                } => Some((*index, arguments_delta.as_str())),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(tool_deltas, vec![(0, "{\""), (0, "city\":\"上海\"}")]);
    }

    #[test]
    fn parses_multiple_tool_indexes_and_finish_at_eof() {
        let fixture = concat!(
            "\u{feff}: keepalive\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[",
            "{\"index\":0,\"id\":\"call-a\",\"function\":{\"name\":\"alpha\",\"arguments\":\"{}\"}},",
            "{\"index\":1,\"id\":\"call-b\",\"function\":{\"name\":\"beta\",\"arguments\":\"{}\"}}",
            "]},\"finish_reason\":\"tool_calls\"}]}"
        );
        let mut events = Vec::new();
        consume_sse(
            Cursor::new(fixture),
            Arc::new(AtomicBool::new(false)),
            &mut |event| {
                events.push(event);
                true
            },
        )
        .expect("terminal event permits EOF without done marker");

        assert!(events.iter().any(|event| matches!(
            event,
            ProviderStreamEvent::ToolCallDelta { index: 0, id: Some(id), .. } if id == "call-a"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            ProviderStreamEvent::ToolCallDelta { index: 1, id: Some(id), .. } if id == "call-b"
        )));
    }

    #[test]
    fn accepts_all_sse_line_endings() {
        for separator in ["\n", "\r\n", "\r"] {
            let fixture = [
                "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"},\"finish_reason\":null}]}",
                "",
                "data: [DONE]",
                "",
            ]
            .join(separator);
            let mut events = Vec::new();
            consume_sse(
                Cursor::new(fixture),
                Arc::new(AtomicBool::new(false)),
                &mut |event| {
                    events.push(event);
                    true
                },
            )
            .expect("supported SSE line ending");
            assert!(events.iter().any(
                |event| matches!(event, ProviderStreamEvent::TextDelta(text) if text == "ok")
            ));
        }
    }

    #[test]
    fn rejects_malformed_stream_delta_fields() {
        for payload in [
            r#"{"choices":[{"delta":"bad","finish_reason":"stop"}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":{}},"finish_reason":"tool_calls"}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":"zero"}]},"finish_reason":"tool_calls"}]}"#,
        ] {
            let fixture = format!("data: {payload}\n\n");
            let error = consume_sse(
                Cursor::new(fixture),
                Arc::new(AtomicBool::new(false)),
                &mut |_| true,
            )
            .expect_err("malformed stream field");
            assert_eq!(error.kind(), ProviderErrorKind::Protocol);
        }
    }

    #[test]
    fn rejects_abrupt_stream_and_oversized_line_or_event() {
        let abrupt = "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"},\"finish_reason\":null}]}\n\n";
        let error = consume_sse(
            Cursor::new(abrupt),
            Arc::new(AtomicBool::new(false)),
            &mut |_| true,
        )
        .expect_err("missing done or finish reason");
        assert_eq!(error.kind(), ProviderErrorKind::Protocol);

        let oversized_line = format!("data: {}\n\n", "x".repeat(MAX_SSE_LINE_BYTES));
        let error = consume_sse(
            Cursor::new(oversized_line),
            Arc::new(AtomicBool::new(false)),
            &mut |_| true,
        )
        .expect_err("oversized line");
        assert_eq!(error.kind(), ProviderErrorKind::Protocol);

        let half = "x".repeat(MAX_SSE_EVENT_BYTES / 2);
        let oversized_event = format!("data: {half}\ndata: {half}\n\n");
        let error = consume_sse(
            Cursor::new(oversized_event),
            Arc::new(AtomicBool::new(false)),
            &mut |_| true,
        )
        .expect_err("oversized cumulative event");
        assert_eq!(error.kind(), ProviderErrorKind::Protocol);
    }

    #[test]
    fn stream_error_and_sink_cancellation_never_expose_raw_payload() {
        let secret_marker = "synthetic-response-secret-marker";
        let fixture = format!(
            "data: {{\"error\":{{\"type\":\"invalid_api_key\",\"message\":\"{secret_marker}\"}}}}\n\n"
        );
        let error = consume_sse(
            Cursor::new(fixture),
            Arc::new(AtomicBool::new(false)),
            &mut |_| true,
        )
        .expect_err("stream error");
        assert_eq!(error.kind(), ProviderErrorKind::Authentication);
        assert!(!error.public_message().contains(secret_marker));
        assert!(!error.to_string().contains(secret_marker));

        let normal = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"stop\"},\"finish_reason\":null}]}\n\n",
            "data: [DONE]\n\n"
        );
        let error = consume_sse(
            Cursor::new(normal),
            Arc::new(AtomicBool::new(false)),
            &mut |_| false,
        )
        .expect_err("sink cancellation");
        assert_eq!(error.kind(), ProviderErrorKind::Cancelled);
    }

    #[test]
    fn authorization_is_sensitive_and_loaded_on_demand() {
        let id = SecretId::new("Cakify/provider/test/api-key").expect("secret id");
        let secrets = Arc::new(MemorySecrets::default());
        secrets
            .put(
                &id,
                &SecretInput::from_utf8("synthetic-key").expect("secret input"),
            )
            .expect("store key");
        let provider = OpenAiCompatibleProvider::new(
            OpenAiConfig::new("https://api.example.test/v1", Some(id)).expect("provider config"),
            secrets,
        )
        .expect("provider");

        let header = provider
            .authorization()
            .expect("authorization")
            .expect("configured header");
        assert!(header.is_sensitive());
        assert_eq!(header.as_bytes(), b"Bearer synthetic-key");
    }
}
