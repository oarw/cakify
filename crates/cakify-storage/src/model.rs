use crate::StorageError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

impl MessageRole {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }

    pub(crate) fn from_storage(value: String) -> Result<Self, StorageError> {
        match value.as_str() {
            "system" => Ok(Self::System),
            "user" => Ok(Self::User),
            "assistant" => Ok(Self::Assistant),
            "tool" => Ok(Self::Tool),
            _ => Err(StorageError::InvalidStoredValue {
                field: "messages.role",
                value,
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessagePartKind {
    Text,
    ReasoningSummary,
    Image,
    File,
    ToolCall,
    ToolResult,
    Citation,
    Error,
}

impl MessagePartKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::ReasoningSummary => "reasoning_summary",
            Self::Image => "image",
            Self::File => "file",
            Self::ToolCall => "tool_call",
            Self::ToolResult => "tool_result",
            Self::Citation => "citation",
            Self::Error => "error",
        }
    }

    pub(crate) fn from_storage(value: String) -> Result<Self, StorageError> {
        match value.as_str() {
            "text" => Ok(Self::Text),
            "reasoning_summary" => Ok(Self::ReasoningSummary),
            "image" => Ok(Self::Image),
            "file" => Ok(Self::File),
            "tool_call" => Ok(Self::ToolCall),
            "tool_result" => Ok(Self::ToolResult),
            "citation" => Ok(Self::Citation),
            "error" => Ok(Self::Error),
            _ => Err(StorageError::InvalidStoredValue {
                field: "message_parts.kind",
                value,
            }),
        }
    }

    pub(crate) const fn supports_text_checkpoint(self) -> bool {
        matches!(self, Self::Text | Self::ReasoningSummary)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunStatus {
    Preparing,
    Requesting,
    Streaming,
    AwaitingApproval,
    ToolRunning,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

impl RunStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::Requesting => "requesting",
            Self::Streaming => "streaming",
            Self::AwaitingApproval => "awaiting_approval",
            Self::ToolRunning => "tool_running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }

    pub(crate) fn from_storage(value: String) -> Result<Self, StorageError> {
        match value.as_str() {
            "preparing" => Ok(Self::Preparing),
            "requesting" => Ok(Self::Requesting),
            "streaming" => Ok(Self::Streaming),
            "awaiting_approval" => Ok(Self::AwaitingApproval),
            "tool_running" => Ok(Self::ToolRunning),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
            _ => Err(StorageError::InvalidStoredValue {
                field: "runs.status",
                value,
            }),
        }
    }

    pub const fn is_active(self) -> bool {
        matches!(
            self,
            Self::Preparing
                | Self::Requesting
                | Self::Streaming
                | Self::AwaitingApproval
                | Self::ToolRunning
        )
    }

    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::Interrupted
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewProviderModel {
    pub model_id: String,
    pub display_name: Option<String>,
    pub capabilities_json: String,
    pub fetched_at: i64,
}

impl NewProviderModel {
    pub fn new(model_id: impl Into<String>, fetched_at: i64) -> Self {
        Self {
            model_id: model_id.into(),
            display_name: None,
            capabilities_json: "{}".to_owned(),
            fetched_at,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderModelRecord {
    pub provider_id: String,
    pub model_id: String,
    pub display_name: Option<String>,
    pub capabilities_json: String,
    pub fetched_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewProviderProfile {
    pub id: String,
    pub kind: String,
    pub endpoint: Option<String>,
    pub display_name: String,
    pub credential_ref: Option<String>,
    pub default_model: Option<String>,
    pub metadata_json: String,
    pub created_at: i64,
    pub models: Vec<NewProviderModel>,
}

impl NewProviderProfile {
    pub fn new(
        id: impl Into<String>,
        kind: impl Into<String>,
        display_name: impl Into<String>,
        created_at: i64,
    ) -> Self {
        Self {
            id: id.into(),
            kind: kind.into(),
            endpoint: None,
            display_name: display_name.into(),
            credential_ref: None,
            default_model: None,
            metadata_json: "{}".to_owned(),
            created_at,
            models: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderProfileRecord {
    pub id: String,
    pub kind: String,
    pub endpoint: Option<String>,
    pub display_name: String,
    pub credential_ref: Option<String>,
    pub default_model: Option<String>,
    pub metadata_json: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub disabled_at: Option<i64>,
    pub models: Vec<ProviderModelRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderProfileUpdate {
    pub id: String,
    pub kind: String,
    pub endpoint: Option<String>,
    pub display_name: String,
    pub credential_ref: Option<String>,
    pub default_model: Option<String>,
    pub metadata_json: String,
    pub expected_updated_at: i64,
    pub updated_at: i64,
    /// `None` preserves the cache; `Some` atomically replaces it with this list.
    pub models: Option<Vec<NewProviderModel>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderProfileStatusUpdate {
    pub id: String,
    pub disabled_at: Option<i64>,
    pub expected_updated_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeletedProviderProfile {
    pub id: String,
    pub credential_ref: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum McpTransport {
    Stdio,
    StreamableHttp,
}

impl McpTransport {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Stdio => "stdio",
            Self::StreamableHttp => "streamable_http",
        }
    }

    pub(crate) fn from_storage(value: String) -> Result<Self, StorageError> {
        match value.as_str() {
            "stdio" => Ok(Self::Stdio),
            "streamable_http" => Ok(Self::StreamableHttp),
            _ => Err(StorageError::InvalidStoredValue {
                field: "mcp_servers.transport",
                value,
            }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewMcpServer {
    pub id: String,
    pub display_name: String,
    pub transport: McpTransport,
    pub config_json: String,
    pub enabled: bool,
    pub created_at: i64,
}

impl NewMcpServer {
    pub fn stdio(
        id: impl Into<String>,
        display_name: impl Into<String>,
        command: impl Into<String>,
        created_at: i64,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            transport: McpTransport::Stdio,
            config_json: serde_json::json!({
                "command": command.into(),
                "args": [],
            })
            .to_string(),
            enabled: false,
            created_at,
        }
    }

    pub fn streamable_http(
        id: impl Into<String>,
        display_name: impl Into<String>,
        url: impl Into<String>,
        created_at: i64,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            transport: McpTransport::StreamableHttp,
            config_json: serde_json::json!({ "url": url.into() }).to_string(),
            enabled: false,
            created_at,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct McpServerRecord {
    pub id: String,
    pub display_name: String,
    pub transport: McpTransport,
    pub config_json: String,
    pub enabled: bool,
    pub capabilities_json: Option<String>,
    pub schema_hash: Option<String>,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl McpServerRecord {
    pub fn target(&self) -> Option<String> {
        let config: serde_json::Value = serde_json::from_str(&self.config_json).ok()?;
        match self.transport {
            McpTransport::Stdio => config.get("command")?.as_str().map(str::to_owned),
            McpTransport::StreamableHttp => config.get("url")?.as_str().map(str::to_owned),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct McpServerStatusUpdate {
    pub id: String,
    pub enabled: bool,
    pub expected_updated_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewConversation {
    pub id: String,
    pub title: String,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub provider_snapshot_json: String,
    pub system_instruction: Option<String>,
    pub created_at: i64,
}

impl NewConversation {
    pub fn new(id: impl Into<String>, title: impl Into<String>, created_at: i64) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            provider_id: None,
            model_id: None,
            provider_snapshot_json: "{}".to_owned(),
            system_instruction: None,
            created_at,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationRecord {
    pub id: String,
    pub title: String,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub provider_snapshot_json: String,
    pub system_instruction: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
    pub deleted_at: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationCursor {
    pub updated_at: i64,
    pub id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationQuery {
    pub limit: usize,
    pub cursor: Option<ConversationCursor>,
    pub include_archived: bool,
}

impl ConversationQuery {
    pub const MAX_LIMIT: usize = 100;

    pub const fn new(limit: usize) -> Self {
        Self {
            limit,
            cursor: None,
            include_archived: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationPage {
    pub items: Vec<ConversationRecord>,
    pub next_cursor: Option<ConversationCursor>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewMessagePart {
    pub id: String,
    pub kind: MessagePartKind,
    pub text_content: Option<String>,
    pub content_json: Option<String>,
    pub attachment_id: Option<String>,
    pub created_at: i64,
}

impl NewMessagePart {
    pub fn text(id: impl Into<String>, text: impl Into<String>, created_at: i64) -> Self {
        Self {
            id: id.into(),
            kind: MessagePartKind::Text,
            text_content: Some(text.into()),
            content_json: None,
            attachment_id: None,
            created_at,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewMessage {
    pub id: String,
    pub conversation_id: String,
    pub role: MessageRole,
    pub ordinal: i64,
    pub parent_message_id: Option<String>,
    pub edited_from_message_id: Option<String>,
    pub created_at: i64,
    pub parts: Vec<NewMessagePart>,
}

impl NewMessage {
    pub fn new(
        id: impl Into<String>,
        conversation_id: impl Into<String>,
        role: MessageRole,
        ordinal: i64,
        created_at: i64,
        parts: Vec<NewMessagePart>,
    ) -> Self {
        Self {
            id: id.into(),
            conversation_id: conversation_id.into(),
            role,
            ordinal,
            parent_message_id: None,
            edited_from_message_id: None,
            created_at,
            parts,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessagePartRecord {
    pub id: String,
    pub message_id: String,
    pub ordinal: i64,
    pub kind: MessagePartKind,
    pub text_content: Option<String>,
    pub content_json: Option<String>,
    pub attachment_id: Option<String>,
    pub created_at: i64,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageRecord {
    pub id: String,
    pub conversation_id: String,
    pub role: MessageRole,
    pub ordinal: i64,
    pub parent_message_id: Option<String>,
    pub edited_from_message_id: Option<String>,
    pub created_at: i64,
    pub deleted_at: Option<i64>,
    pub parts: Vec<MessagePartRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationThread {
    pub conversation: ConversationRecord,
    pub messages: Vec<MessageRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewRun {
    pub id: String,
    pub conversation_id: String,
    pub assistant_message_id: Option<String>,
    pub status: RunStatus,
    pub provider_snapshot_json: String,
    pub model_id: String,
    pub started_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunRecord {
    pub id: String,
    pub conversation_id: String,
    pub assistant_message_id: Option<String>,
    pub status: RunStatus,
    pub provider_snapshot_json: String,
    pub model_id: String,
    pub started_at: i64,
    pub updated_at: i64,
    pub finished_at: Option<i64>,
    pub finish_reason: Option<String>,
    pub error_kind: Option<String>,
    pub error_message: Option<String>,
    pub usage_json: Option<String>,
    pub cancel_source: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunUpdate {
    pub id: String,
    pub status: RunStatus,
    pub updated_at: i64,
    pub finished_at: Option<i64>,
    pub finish_reason: Option<String>,
    pub error_kind: Option<String>,
    pub error_message: Option<String>,
    pub usage_json: Option<String>,
    pub cancel_source: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextCheckpoint {
    pub conversation_id: String,
    pub message_id: String,
    pub part_id: String,
    pub text_content: String,
    pub revision: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrashRecoveryReport {
    pub recovered_at: i64,
    pub interrupted_run_ids: Vec<String>,
}

impl CrashRecoveryReport {
    pub fn recovered_count(&self) -> usize {
        self.interrupted_run_ids.len()
    }
}
