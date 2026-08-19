use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChatToolFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChatToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ChatToolFunction,
}

impl ChatToolCall {
    pub fn function(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: "function".to_owned(),
            function: ChatToolFunction {
                name: name.into(),
                arguments: arguments.into(),
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ChatToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn assistant_with_tool_calls(
        content: impl Into<String>,
        tool_calls: Vec<ChatToolCall>,
    ) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
            tool_calls,
            tool_call_id: None,
        }
    }

    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Tool,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    /// A JSON Schema object. Provider adapters must parse and validate it before sending.
    pub parameters_json: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
    pub temperature: Option<f32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ProviderStreamEvent {
    TextDelta(String),
    ToolCallDelta {
        index: u32,
        id: Option<String>,
        name: Option<String>,
        arguments_delta: String,
    },
    Usage(Usage),
    Finished {
        reason: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderErrorKind {
    NotConfigured,
    InvalidRequest,
    Authentication,
    RateLimited,
    Transport,
    Protocol,
    Cancelled,
}

impl ProviderErrorKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotConfigured => "not_configured",
            Self::InvalidRequest => "invalid_request",
            Self::Authentication => "authentication",
            Self::RateLimited => "rate_limited",
            Self::Transport => "transport",
            Self::Protocol => "protocol",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct ProviderError {
    kind: ProviderErrorKind,
    message: String,
}

impl ProviderError {
    pub fn new(kind: ProviderErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub const fn kind(&self) -> ProviderErrorKind {
        self.kind
    }

    pub fn public_message(&self) -> &str {
        &self.message
    }
}

pub type StreamSink<'a> = dyn FnMut(ProviderStreamEvent) -> bool + 'a;

pub trait ChatProvider: Send + Sync + 'static {
    fn stream(
        &self,
        request: ChatRequest,
        cancellation: Arc<AtomicBool>,
        sink: &mut StreamSink<'_>,
    ) -> Result<(), ProviderError>;
}

pub struct MissingProvider;

impl ChatProvider for MissingProvider {
    fn stream(
        &self,
        _request: ChatRequest,
        cancellation: Arc<AtomicBool>,
        _sink: &mut StreamSink<'_>,
    ) -> Result<(), ProviderError> {
        if cancellation.load(Ordering::Acquire) {
            return Err(ProviderError::new(
                ProviderErrorKind::Cancelled,
                "请求已取消",
            ));
        }
        Err(ProviderError::new(
            ProviderErrorKind::NotConfigured,
            "尚未配置可用的模型 Provider",
        ))
    }
}
