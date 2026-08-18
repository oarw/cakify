//! Provider adapters. Raw response bodies, authorization headers, and secrets
//! never cross this crate's public boundary.

use std::{
    io::{BufRead, BufReader},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
    time::Duration,
};

use cakify_core::{
    ChatProvider, ChatRequest, ProviderError, ProviderErrorKind, ProviderStreamEvent, SecretId,
    SecretStore, StreamSink, Usage,
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
        if request.model.trim().is_empty() {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "模型 ID 不能为空",
            ));
        }
        if request.messages.is_empty() {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "请求至少需要一条消息",
            ));
        }

        let mut body = Map::new();
        body.insert("model".to_owned(), Value::String(request.model.clone()));
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
            let tools = request
                .tools
                .iter()
                .map(|tool| {
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
        Ok(Value::Object(body))
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
    let mut line = String::new();
    let mut data = String::new();
    loop {
        if cancellation.load(Ordering::Acquire) {
            return Err(cancelled());
        }
        line.clear();
        let read = reader.read_line(&mut line).map_err(|_| {
            ProviderError::new(ProviderErrorKind::Transport, "读取 Provider 流式响应失败")
        })?;
        if read == 0 {
            if !data.is_empty() {
                if data != "[DONE]" {
                    dispatch_sse_data(&data, sink)?;
                }
            }
            return Ok(());
        }
        if line.len() > MAX_SSE_LINE_BYTES {
            return Err(ProviderError::new(
                ProviderErrorKind::Protocol,
                "Provider 返回了过大的流式事件",
            ));
        }
        let line = line.trim_end_matches(&['\r', '\n'][..]);
        if line.is_empty() {
            if !data.is_empty() {
                if data == "[DONE]" {
                    return Ok(());
                }
                dispatch_sse_data(&data, sink)?;
                data.clear();
            }
            continue;
        }
        if line.starts_with(':') {
            continue;
        }
        if let Some(value) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(value.strip_prefix(' ').unwrap_or(value));
        }
    }
}

fn dispatch_sse_data(data: &str, sink: &mut StreamSink<'_>) -> Result<(), ProviderError> {
    let payload: Value = serde_json::from_str(data).map_err(|_| {
        ProviderError::new(
            ProviderErrorKind::Protocol,
            "Provider 返回了无法解析的流式事件",
        )
    })?;

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

    let Some(choice) = payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
    else {
        return Ok(());
    };
    let delta = choice.get("delta").unwrap_or(&Value::Null);
    if let Some(content) = delta.get("content").and_then(Value::as_str) {
        if !content.is_empty() && !sink(ProviderStreamEvent::TextDelta(content.to_owned())) {
            return Err(cancelled());
        }
    }

    if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
        for tool_call in tool_calls {
            let index = tool_call
                .get("index")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or(0);
            let function = tool_call.get("function").unwrap_or(&Value::Null);
            if !sink(ProviderStreamEvent::ToolCallDelta {
                index,
                id: tool_call
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                name: function
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                arguments_delta: function
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            }) {
                return Err(cancelled());
            }
        }
    }

    if choice
        .get("finish_reason")
        .is_some_and(|value| !value.is_null())
    {
        if !sink(ProviderStreamEvent::Finished {
            reason: choice
                .get("finish_reason")
                .and_then(Value::as_str)
                .map(str::to_owned),
        }) {
            return Err(cancelled());
        }
    }
    Ok(())
}

fn cancelled() -> ProviderError {
    ProviderError::new(ProviderErrorKind::Cancelled, "请求已取消")
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, io::Cursor, sync::Mutex};

    use cakify_core::{ChatMessage, ChatRole, SecretError, SecretInput, SecretValue};

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
    fn request_body_encodes_messages_and_validated_tools() {
        let request = ChatRequest {
            model: "test-model".to_owned(),
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: "hello".to_owned(),
            }],
            tools: vec![cakify_core::ToolDefinition {
                name: "clock".to_owned(),
                description: "read time".to_owned(),
                parameters_json: r#"{"type":"object"}"#.to_owned(),
            }],
            temperature: Some(0.5),
        };
        let body = OpenAiCompatibleProvider::request_body(&request).expect("request body");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["tools"][0]["function"]["name"], "clock");
        assert_eq!(body["stream_options"]["include_usage"], true);
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
