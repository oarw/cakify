mod composer;
mod markdown;

use std::{
    sync::{atomic::AtomicBool, Arc},
    time::SystemTime,
};

use cakify_core::{
    builtin_tool_definitions, put_then_commit_reference, AppCommand, AppEvent, BuiltinToolExecutor,
    ConversationId, CoreEvents, CoreRuntime, RequestId, RunId, SecretId, SecretInput, SecretStore,
    ToolCall, ToolExecutionError, ToolExecutor, Usage, CURRENT_TIME_TOOL_NAME,
};
use cakify_mcp::{McpEvent, McpEvents, McpHandle, McpRuntime, McpServerConfig};
use cakify_platform_windows::{app_data_paths, CredentialManagerSecretStore};
use cakify_provider::{OpenAiCompatibleProvider, OpenAiConfig, ProviderRouter};
use cakify_storage::{
    McpServerRecord, McpServerStatusUpdate, McpTransport as StoredMcpTransport, NewMcpServer,
    NewProviderProfile, ProviderProfileRecord, ProviderProfileUpdate, StorageActor, StorageConfig,
    StorageError, StorageHandle,
};
use composer::{bind_input_keys, editor_actions, Submit, TextEditor};
use gpui::{
    div, prelude::*, px, rgb, size, App, Bounds, Context, CursorStyle, Div, Entity, FontWeight,
    MouseButton, MouseUpEvent, SharedString, TitlebarOptions, Window, WindowBounds, WindowOptions,
};
use gpui_platform::application;
use markdown::{parse_markdown, MarkdownBlock};

const WINDOW_WIDTH: f32 = 960.0;
const WINDOW_HEIGHT: f32 = 620.0;
const PROVIDER_ID: &str = "default-openai-compatible";
const PROVIDER_SECRET_ID: &str = "Cakify/provider/default/api-key";

struct DesktopToolExecutor {
    mcp: McpHandle,
}

impl ToolExecutor for DesktopToolExecutor {
    fn execute(
        &self,
        name: &str,
        arguments_json: &str,
        cancellation: Arc<AtomicBool>,
    ) -> Result<String, ToolExecutionError> {
        if name == CURRENT_TIME_TOOL_NAME {
            BuiltinToolExecutor.execute(name, arguments_json, cancellation)
        } else {
            self.mcp.execute(name, arguments_json, cancellation)
        }
    }
}

struct DesktopServices {
    core: CoreRuntime,
    events: CoreEvents,
    mcp_runtime: McpRuntime,
    mcp: McpHandle,
    mcp_events: McpEvents,
    storage_actor: StorageActor,
    storage: StorageHandle,
    secrets: Arc<dyn SecretStore>,
    provider_router: Arc<ProviderRouter>,
    provider_profile: Option<ProviderProfileRecord>,
    mcp_servers: Vec<McpServerRecord>,
    data_root: String,
    startup_status: String,
}

impl DesktopServices {
    fn initialize() -> Result<Self, String> {
        let paths = app_data_paths().map_err(|error| error.to_string())?;
        paths.create_layout().map_err(|error| error.to_string())?;
        let storage_actor = StorageActor::open(StorageConfig::new(paths.data.join("cakify.db")))
            .map_err(|error| error.to_string())?;
        let storage = storage_actor.handle();
        let secrets: Arc<dyn SecretStore> = Arc::new(CredentialManagerSecretStore);
        let provider_router = Arc::new(ProviderRouter::default());
        let provider_profile = storage
            .get_provider_profile(PROVIDER_ID)
            .map_err(|error| error.to_string())?;
        let mcp_servers = storage
            .list_mcp_servers()
            .map_err(|error| error.to_string())?;
        let mcp_runtime = McpRuntime::start().map_err(|error| error.to_string())?;
        let mcp = mcp_runtime.handle();
        let mcp_events = mcp_runtime.events();

        let mut mcp_startup_error = None;
        for server in mcp_servers.iter().filter(|server| server.enabled) {
            let result = McpServerConfig::try_from(server)
                .map_err(|error| error.to_string())
                .and_then(|config| mcp.connect(config).map_err(|error| error.to_string()));
            if let Err(error) = result {
                mcp_startup_error = Some(format!("{}：{error}", server.display_name));
            }
        }

        let mut startup_status = if let Some(profile) = &provider_profile {
            match provider_from_profile(profile, secrets.clone()) {
                Ok(provider) => {
                    provider_router.set(provider);
                    "Provider 已就绪".to_owned()
                }
                Err(error) => error,
            }
        } else {
            "配置 Provider 后即可开始对话".to_owned()
        };
        if let Some(error) = mcp_startup_error {
            startup_status = format!("MCP 启动失败：{error}");
        }
        let core = CoreRuntime::start_with_provider_and_tools(
            provider_router.clone(),
            Arc::new(DesktopToolExecutor { mcp: mcp.clone() }),
        )
        .map_err(|error| error.to_string())?;
        let events = core.events();

        Ok(Self {
            core,
            events,
            mcp_runtime,
            mcp,
            mcp_events,
            storage_actor,
            storage,
            secrets,
            provider_router,
            provider_profile,
            mcp_servers,
            data_root: paths.root.display().to_string(),
            startup_status,
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Panel {
    None,
    Provider,
    Mcp,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum MessageRole {
    User,
    Assistant,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum MessageState {
    Sending,
    Streaming,
    Complete,
    Cancelled,
    Error,
}

struct UiMessage {
    role: MessageRole,
    content: String,
    run_id: Option<RunId>,
    state: MessageState,
    usage: Option<Usage>,
    error: Option<String>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ToolApprovalState {
    Streaming,
    AwaitingApproval,
    Approved,
    Executing,
    Complete,
    Failed,
    Denied,
}

#[derive(Clone)]
struct UiToolCall {
    run_id: RunId,
    index: u32,
    id: String,
    name: String,
    arguments_json: String,
    output: Option<String>,
    state: ToolApprovalState,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum McpTransport {
    Stdio,
    Http,
}

#[derive(Clone, Eq, PartialEq)]
enum McpConnectionState {
    Disabled,
    Connecting,
    Connected { tool_count: usize },
    Failed(String),
}

struct McpServerUi {
    id: String,
    name: String,
    target: String,
    transport: McpTransport,
    enabled: bool,
    updated_at: i64,
    connection: McpConnectionState,
}

struct CakifyApp {
    core: CoreRuntime,
    events: CoreEvents,
    _mcp_runtime: McpRuntime,
    mcp: McpHandle,
    mcp_events: McpEvents,
    _storage_actor: StorageActor,
    storage: StorageHandle,
    secrets: Arc<dyn SecretStore>,
    provider_router: Arc<ProviderRouter>,
    provider_profile: Option<ProviderProfileRecord>,
    composer: Entity<TextEditor>,
    endpoint_editor: Entity<TextEditor>,
    model_editor: Entity<TextEditor>,
    key_editor: Entity<TextEditor>,
    mcp_name_editor: Entity<TextEditor>,
    mcp_target_editor: Entity<TextEditor>,
    mcp_args_editor: Entity<TextEditor>,
    panel: Panel,
    mcp_transport: McpTransport,
    mcp_servers: Vec<McpServerUi>,
    messages: Vec<UiMessage>,
    tool_calls: Vec<UiToolCall>,
    status: SharedString,
    _data_root: SharedString,
    revision: u64,
    next_request: u64,
    active_conversation: Option<ConversationId>,
    active_run: Option<RunId>,
    pending_assistant: Option<usize>,
    last_prompt: Option<String>,
}

impl CakifyApp {
    fn new(services: DesktopServices, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let endpoint = services
            .provider_profile
            .as_ref()
            .and_then(|profile| profile.endpoint.clone())
            .unwrap_or_else(|| "https://api.openai.com/v1".to_owned());
        let model = services
            .provider_profile
            .as_ref()
            .and_then(|profile| profile.default_model.clone())
            .unwrap_or_else(|| "gpt-5-mini".to_owned());
        let composer =
            cx.new(|cx| TextEditor::new("", "输入消息，与 AI 对话", false, true, window, cx));
        let endpoint_editor = cx.new(|cx| {
            TextEditor::new(
                endpoint,
                "https://api.example.com/v1",
                false,
                false,
                window,
                cx,
            )
        });
        let model_editor = cx.new(|cx| TextEditor::new(model, "模型 ID", false, false, window, cx));
        let key_editor = cx.new(|cx| {
            TextEditor::new(
                "",
                if services.provider_profile.is_some() {
                    "API Key 已安全保存；留空保持不变"
                } else {
                    "API Key；本机模型可留空"
                },
                true,
                false,
                window,
                cx,
            )
        });
        let mcp_name_editor =
            cx.new(|cx| TextEditor::new("", "Server 名称", false, false, window, cx));
        let mcp_target_editor = cx
            .new(|cx| TextEditor::new("", "命令或 Streamable HTTP URL", false, false, window, cx));
        let mcp_args_editor = cx.new(|cx| {
            TextEditor::new(
                "[]",
                r#"JSON 参数，例如 ["-y","@modelcontextprotocol/server"]"#,
                false,
                false,
                window,
                cx,
            )
        });

        Self {
            core: services.core,
            events: services.events,
            _mcp_runtime: services.mcp_runtime,
            mcp: services.mcp,
            mcp_events: services.mcp_events,
            _storage_actor: services.storage_actor,
            storage: services.storage,
            secrets: services.secrets,
            provider_router: services.provider_router,
            provider_profile: services.provider_profile,
            composer,
            endpoint_editor,
            model_editor,
            key_editor,
            mcp_name_editor,
            mcp_target_editor,
            mcp_args_editor,
            panel: Panel::None,
            mcp_transport: McpTransport::Stdio,
            mcp_servers: services
                .mcp_servers
                .into_iter()
                .map(mcp_record_to_ui)
                .collect(),
            messages: Vec::new(),
            tool_calls: Vec::new(),
            status: services.startup_status.into(),
            _data_root: services.data_root.into(),
            revision: 0,
            next_request: 1,
            active_conversation: None,
            active_run: None,
            pending_assistant: None,
            last_prompt: None,
        }
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        self.start_event_bridge(cx);
        self.start_mcp_event_bridge(cx);
        let _ = self.core.handle().try_dispatch(AppCommand::Bootstrap);
        self.request_new_conversation();
    }

    fn start_mcp_event_bridge(&mut self, cx: &mut Context<Self>) {
        let events = self.mcp_events.receiver();
        cx.spawn(async move |this, cx| {
            while let Ok(event) = events.recv().await {
                if this
                    .update(cx, |app, cx| {
                        app.apply_mcp_event(event);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    fn apply_mcp_event(&mut self, event: McpEvent) {
        let (server_id, connection, status) = match event {
            McpEvent::ServerConnecting { server_id } => (
                server_id,
                McpConnectionState::Connecting,
                "MCP Server 正在连接".to_owned(),
            ),
            McpEvent::ServerConnected {
                server_id,
                tool_count,
            } => (
                server_id,
                McpConnectionState::Connected { tool_count },
                format!("MCP 已连接，发现 {tool_count} 个工具"),
            ),
            McpEvent::ServerFailed { server_id, message } => (
                server_id,
                McpConnectionState::Failed(message.clone()),
                format!("MCP 连接失败：{message}"),
            ),
            McpEvent::ServerDisconnected { server_id } => (
                server_id,
                McpConnectionState::Disabled,
                "MCP Server 已断开".to_owned(),
            ),
        };
        if let Some(server) = self
            .mcp_servers
            .iter_mut()
            .find(|server| server.id == server_id)
        {
            server.connection = connection;
            self.status = status.into();
        }
    }

    fn start_event_bridge(&mut self, cx: &mut Context<Self>) {
        let events = self.events.receiver();
        cx.spawn(async move |this, cx| {
            while let Ok(event) = events.recv().await {
                if this
                    .update(cx, |app, cx| {
                        app.apply_event(event);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    fn request_new_conversation(&mut self) {
        let request_id = RequestId::new(self.next_request);
        self.next_request += 1;
        match self
            .core
            .handle()
            .try_dispatch(AppCommand::CreateConversation {
                request_id,
                title: "新会话".to_owned(),
            }) {
            Ok(()) => self.status = "正在创建会话".into(),
            Err(error) => self.status = error.to_string().into(),
        }
    }

    fn apply_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::CoreReady { revision } => self.revision = revision,
            AppEvent::CoreStopped { revision } => {
                self.revision = revision;
                self.status = "聊天核心已停止".into();
            }
            AppEvent::ConversationCreated {
                conversation_id,
                revision,
                ..
            } => {
                self.revision = revision;
                self.active_conversation = Some(conversation_id);
                self.status = if self.provider_router.is_configured() {
                    "就绪".into()
                } else {
                    "配置 Provider 后即可开始对话".into()
                };
            }
            AppEvent::DraftAccepted {
                run_id, revision, ..
            } => {
                self.revision = revision;
                self.active_run = Some(run_id);
                if let Some(index) = self.pending_assistant.take() {
                    if let Some(message) = self.messages.get_mut(index) {
                        message.run_id = Some(run_id);
                        message.state = MessageState::Streaming;
                    }
                }
                self.status = "正在生成".into();
            }
            AppEvent::AssistantDelta {
                run_id,
                delta,
                revision,
                ..
            } => {
                self.revision = revision;
                if let Some(message) = self
                    .messages
                    .iter_mut()
                    .find(|message| message.run_id == Some(run_id))
                {
                    message.content.push_str(&delta);
                    message.state = MessageState::Streaming;
                }
            }
            AppEvent::ToolCallDelta {
                run_id,
                index,
                id,
                name,
                arguments_delta,
                revision,
            } => {
                self.revision = revision;
                let tool_call = self.tool_call_mut(run_id, index);
                if let Some(id) = id {
                    tool_call.id = id;
                }
                if let Some(name) = name {
                    tool_call.name = name;
                }
                tool_call.arguments_json.push_str(&arguments_delta);
            }
            AppEvent::ToolApprovalRequested {
                run_id,
                call,
                revision,
            } => {
                self.revision = revision;
                self.finish_tool_call(run_id, call);
                self.status = "工具调用等待审批".into();
            }
            AppEvent::ToolApprovalResolved {
                run_id,
                tool_call_id,
                approved,
                revision,
            } => {
                self.revision = revision;
                if let Some(call) = self
                    .tool_calls
                    .iter_mut()
                    .find(|call| call.run_id == run_id && call.id == tool_call_id)
                {
                    call.state = if approved {
                        ToolApprovalState::Approved
                    } else {
                        ToolApprovalState::Denied
                    };
                }
            }
            AppEvent::ToolExecutionStarted {
                run_id,
                tool_call_id,
                revision,
            } => {
                self.revision = revision;
                if let Some(call) = self
                    .tool_calls
                    .iter_mut()
                    .find(|call| call.run_id == run_id && call.id == tool_call_id)
                {
                    call.state = ToolApprovalState::Executing;
                }
                self.status = "正在执行工具".into();
            }
            AppEvent::ToolExecutionCompleted {
                run_id,
                tool_call_id,
                output,
                revision,
            } => {
                self.revision = revision;
                if let Some(call) = self
                    .tool_calls
                    .iter_mut()
                    .find(|call| call.run_id == run_id && call.id == tool_call_id)
                {
                    call.state = ToolApprovalState::Complete;
                    call.output = Some(output);
                }
                self.status = "工具执行完成，模型继续生成".into();
            }
            AppEvent::ToolExecutionFailed {
                run_id,
                tool_call_id,
                message,
                revision,
            } => {
                self.revision = revision;
                if let Some(call) = self
                    .tool_calls
                    .iter_mut()
                    .find(|call| call.run_id == run_id && call.id == tool_call_id)
                {
                    call.state = ToolApprovalState::Failed;
                    call.output = Some(message);
                }
                self.status = "工具执行失败，错误已回填模型".into();
            }
            AppEvent::RunCompleted {
                run_id,
                usage,
                revision,
                ..
            } => {
                self.revision = revision;
                self.active_run = self.active_run.filter(|active| *active != run_id);
                if let Some(message) = self
                    .messages
                    .iter_mut()
                    .find(|message| message.run_id == Some(run_id))
                {
                    message.state = MessageState::Complete;
                    message.usage = usage;
                }
                self.status = if self.tool_calls.iter().any(|call| {
                    call.run_id == run_id && call.state == ToolApprovalState::AwaitingApproval
                }) {
                    "工具调用等待审批".into()
                } else {
                    "完成".into()
                };
            }
            AppEvent::RunFailed {
                run_id,
                message,
                revision,
                ..
            } => {
                self.revision = revision;
                self.active_run = self.active_run.filter(|active| *active != run_id);
                if let Some(item) = self
                    .messages
                    .iter_mut()
                    .find(|item| item.run_id == Some(run_id))
                {
                    item.state = MessageState::Error;
                    item.error = Some(message.clone());
                }
                self.status = message.into();
            }
            AppEvent::RunCancelled {
                run_id, revision, ..
            } => {
                self.revision = revision;
                self.active_run = self.active_run.filter(|active| *active != run_id);
                if let Some(message) = self
                    .messages
                    .iter_mut()
                    .find(|message| message.run_id == Some(run_id))
                {
                    message.state = MessageState::Cancelled;
                }
                self.status = "已停止".into();
            }
            AppEvent::Status { message, revision } => {
                self.revision = revision;
                self.status = message.into();
            }
        }
    }

    fn tool_call_mut(&mut self, run_id: RunId, index: u32) -> &mut UiToolCall {
        if let Some(position) = self
            .tool_calls
            .iter()
            .position(|call| call.run_id == run_id && call.index == index)
        {
            return &mut self.tool_calls[position];
        }
        self.tool_calls.push(UiToolCall {
            run_id,
            index,
            id: String::new(),
            name: String::new(),
            arguments_json: String::new(),
            output: None,
            state: ToolApprovalState::Streaming,
        });
        self.tool_calls.last_mut().expect("inserted tool call")
    }

    fn finish_tool_call(&mut self, run_id: RunId, call: ToolCall) {
        let existing = self.tool_call_mut(run_id, call.index);
        existing.id = call.id;
        existing.name = call.name;
        existing.arguments_json = call.arguments_json;
        existing.state = ToolApprovalState::AwaitingApproval;
    }

    fn new_conversation(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(run_id) = self.active_run.take() {
            let _ = self
                .core
                .handle()
                .try_dispatch(AppCommand::CancelRun { run_id });
        }
        self.messages.clear();
        self.tool_calls.clear();
        self.active_conversation = None;
        self.pending_assistant = None;
        self.last_prompt = None;
        self.request_new_conversation();
        cx.notify();
    }

    fn submit_from_keyboard(&mut self, _: &Submit, window: &mut Window, cx: &mut Context<Self>) {
        if self.composer.read(cx).focus_handle.is_focused(window) {
            self.send_current(cx);
        }
    }

    fn send_click(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(run_id) = self.active_run {
            self.cancel_run(run_id, cx);
        } else {
            self.send_current(cx);
        }
    }

    fn send_current(&mut self, cx: &mut Context<Self>) {
        if self.active_run.is_some() {
            return;
        }
        if !self.provider_router.is_configured() {
            self.panel = Panel::Provider;
            self.status = "请先保存 Provider 配置".into();
            cx.notify();
            return;
        }
        let Some(conversation_id) = self.active_conversation else {
            self.status = "会话仍在初始化".into();
            cx.notify();
            return;
        };
        let text = self.composer.read(cx).text(cx);
        if text.trim().is_empty() {
            return;
        }
        let model = self.model_editor.read(cx).text(cx);
        if model.trim().is_empty() {
            self.panel = Panel::Provider;
            self.status = "模型 ID 不能为空".into();
            cx.notify();
            return;
        }

        let request_id = RequestId::new(self.next_request);
        self.next_request += 1;
        self.messages.push(UiMessage {
            role: MessageRole::User,
            content: text.clone(),
            run_id: None,
            state: MessageState::Complete,
            usage: None,
            error: None,
        });
        self.messages.push(UiMessage {
            role: MessageRole::Assistant,
            content: String::new(),
            run_id: None,
            state: MessageState::Sending,
            usage: None,
            error: None,
        });
        self.pending_assistant = Some(self.messages.len() - 1);
        self.last_prompt = Some(text.clone());

        let mut tools = builtin_tool_definitions();
        tools.extend(self.mcp.tool_definitions());
        match self.core.handle().try_dispatch(AppCommand::SubmitDraft {
            request_id,
            conversation_id,
            model,
            text,
            tools,
            temperature: None,
        }) {
            Ok(()) => {
                self.composer.update(cx, |composer, cx| composer.clear(cx));
                self.status = "正在发送".into();
            }
            Err(error) => {
                if let Some(index) = self.pending_assistant.take() {
                    if let Some(message) = self.messages.get_mut(index) {
                        message.state = MessageState::Error;
                        message.error = Some(error.to_string());
                    }
                }
                self.status = error.to_string().into();
            }
        }
        cx.notify();
    }

    fn retry_last(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(prompt) = self.last_prompt.clone() else {
            return;
        };
        self.composer
            .update(cx, |composer, cx| composer.set_text(prompt, cx));
        self.send_current(cx);
    }

    fn use_suggestion(
        &mut self,
        prompt: String,
        _: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.composer
            .update(cx, |composer, cx| composer.set_text(prompt, cx));
        self.status = "已填入消息，可直接发送".into();
        cx.notify();
    }

    fn cancel_run(&mut self, run_id: RunId, cx: &mut Context<Self>) {
        match self
            .core
            .handle()
            .try_dispatch(AppCommand::CancelRun { run_id })
        {
            Ok(()) => self.status = "正在停止".into(),
            Err(error) => self.status = error.to_string().into(),
        }
        cx.notify();
    }

    fn toggle_provider(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.panel = if self.panel == Panel::Provider {
            Panel::None
        } else {
            Panel::Provider
        };
        cx.notify();
    }

    fn toggle_mcp(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.panel = if self.panel == Panel::Mcp {
            Panel::None
        } else {
            Panel::Mcp
        };
        cx.notify();
    }

    fn save_provider(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let endpoint = self.endpoint_editor.read(cx).text(cx);
        let model = self.model_editor.read(cx).text(cx);
        let key = self.key_editor.read(cx).text(cx);
        if endpoint.trim().is_empty() || model.trim().is_empty() {
            self.status = "Endpoint 和模型 ID 不能为空".into();
            cx.notify();
            return;
        }
        let secret_id = match SecretId::new(PROVIDER_SECRET_ID) {
            Ok(secret_id) => secret_id,
            Err(error) => {
                self.status = error.to_string().into();
                cx.notify();
                return;
            }
        };
        let credential_ref = if !key.is_empty()
            || self
                .provider_profile
                .as_ref()
                .and_then(|profile| profile.credential_ref.as_ref())
                .is_some()
        {
            Some(secret_id.as_str().to_owned())
        } else {
            None
        };

        let save = || {
            upsert_provider_profile(
                &self.storage,
                self.provider_profile.as_ref(),
                endpoint.clone(),
                model.clone(),
                credential_ref.clone(),
            )
        };
        let result = if key.is_empty() {
            save().map_err(|error| error.to_string())
        } else {
            let input = match SecretInput::from_utf8(key) {
                Ok(input) => input,
                Err(error) => {
                    self.status = error.to_string().into();
                    cx.notify();
                    return;
                }
            };
            let mut saved = None;
            put_then_commit_reference(self.secrets.as_ref(), &secret_id, &input, |_| {
                saved = Some(save()?);
                Ok::<(), StorageError>(())
            })
            .map_err(|error| error.to_string())
            .and_then(|()| saved.ok_or_else(|| "Provider profile 未写入".to_owned()))
        };

        match result {
            Ok(profile) => match provider_from_profile(&profile, self.secrets.clone()) {
                Ok(provider) => {
                    self.provider_router.set(provider);
                    self.provider_profile = Some(profile);
                    self.key_editor.update(cx, |editor, cx| editor.clear(cx));
                    self.status = "Provider 已保存".into();
                    self.panel = Panel::None;
                }
                Err(error) => self.status = error.into(),
            },
            Err(error) => self.status = error.into(),
        }
        cx.notify();
    }

    fn select_stdio(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.mcp_transport = McpTransport::Stdio;
        cx.notify();
    }

    fn select_http(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.mcp_transport = McpTransport::Http;
        cx.notify();
    }

    fn add_mcp_server(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let name = self.mcp_name_editor.read(cx).text(cx);
        let target = self.mcp_target_editor.read(cx).text(cx);
        if name.trim().is_empty() || target.trim().is_empty() {
            self.status = "MCP Server 名称和目标不能为空".into();
            cx.notify();
            return;
        }
        let now = match unix_millis() {
            Ok(now) => now,
            Err(error) => {
                self.status = error.into();
                cx.notify();
                return;
            }
        };
        let id = format!("mcp-{now}-{}", self.mcp_servers.len());
        let input = match self.mcp_transport {
            McpTransport::Stdio => {
                let args_json = self.mcp_args_editor.read(cx).text(cx);
                let args = match serde_json::from_str::<Vec<String>>(&args_json) {
                    Ok(args) => args,
                    Err(_) => {
                        self.status = "stdio 参数必须是 JSON 字符串数组".into();
                        cx.notify();
                        return;
                    }
                };
                NewMcpServer::stdio_with_args(id, name, target, args, now)
            }
            McpTransport::Http => NewMcpServer::streamable_http(id, name, target, now),
        };
        let stored = match self.storage.create_mcp_server(input) {
            Ok(stored) => stored,
            Err(error) => {
                self.status = error.to_string().into();
                cx.notify();
                return;
            }
        };
        self.mcp_servers.push(mcp_record_to_ui(stored));
        self.mcp_name_editor
            .update(cx, |editor, cx| editor.clear(cx));
        self.mcp_target_editor
            .update(cx, |editor, cx| editor.clear(cx));
        self.mcp_args_editor
            .update(cx, |editor, cx| editor.set_text("[]", cx));
        self.status = "MCP Server 已保存；启用后连接".into();
        cx.notify();
    }

    fn set_mcp_server_enabled(
        &mut self,
        id: String,
        enabled: bool,
        expected_updated_at: i64,
        cx: &mut Context<Self>,
    ) {
        if !enabled {
            if let Err(error) = self.mcp.disconnect(id.clone()) {
                self.status = format!("无法断开 MCP Server：{error}").into();
                cx.notify();
                return;
            }
        }
        let now = match next_timestamp(expected_updated_at) {
            Ok(now) => now,
            Err(error) => {
                self.status = error.into();
                cx.notify();
                return;
            }
        };
        match self.storage.set_mcp_server_enabled(McpServerStatusUpdate {
            id: id.clone(),
            enabled,
            expected_updated_at,
            updated_at: now,
        }) {
            Ok(stored) => {
                let dispatch_error = if enabled {
                    McpServerConfig::try_from(&stored)
                        .map_err(|error| error.to_string())
                        .and_then(|config| {
                            self.mcp.connect(config).map_err(|error| error.to_string())
                        })
                        .err()
                } else {
                    None
                };
                let mut replacement = mcp_record_to_ui(stored);
                if let Some(error) = dispatch_error {
                    replacement.connection = McpConnectionState::Failed(error.clone());
                    self.status = format!("MCP Server 已启用但连接失败：{error}").into();
                } else {
                    self.status = if enabled {
                        "MCP Server 已启用，正在连接".into()
                    } else {
                        "MCP Server 已停用".into()
                    };
                }
                if let Some(server) = self.mcp_servers.iter_mut().find(|server| server.id == id) {
                    *server = replacement;
                }
            }
            Err(error) => self.status = error.to_string().into(),
        }
        cx.notify();
    }

    fn delete_mcp_server(&mut self, id: String, cx: &mut Context<Self>) {
        if let Err(error) = self.mcp.disconnect(id.clone()) {
            self.status = format!("无法断开 MCP Server：{error}").into();
            cx.notify();
            return;
        }
        match self.storage.delete_mcp_server(&id) {
            Ok(()) => {
                self.mcp_servers.retain(|server| server.id != id);
                self.status = "MCP Server 已删除".into();
            }
            Err(error) => self.status = error.to_string().into(),
        }
        cx.notify();
    }

    fn resolve_tool(
        &mut self,
        run_id: RunId,
        tool_call_id: String,
        approved: bool,
        cx: &mut Context<Self>,
    ) {
        match self
            .core
            .handle()
            .try_dispatch(AppCommand::ResolveToolApproval {
                run_id,
                tool_call_id,
                approved,
            }) {
            Ok(()) => {
                self.status = if approved {
                    "已允许工具调用，等待执行".into()
                } else {
                    "已拒绝工具调用".into()
                };
            }
            Err(error) => self.status = error.to_string().into(),
        }
        cx.notify();
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> Div {
        let rail_surface = rgb(0xf1f3f8);
        let list_surface = rgb(0xf8f9fc);
        let border = rgb(0xe1e4ec);
        let text = rgb(0x262a34);
        let muted = rgb(0x7b8190);
        let selected = rgb(0xe7ebf6);
        let accent = rgb(0x5368a5);
        let rail = div()
            .w(px(56.0))
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .bg(rail_surface)
            .border_r_1()
            .border_color(border)
            .py_3()
            .child(brand_mark(30.0))
            .child(div().h(px(18.0)))
            .child(
                div()
                    .id("new-conversation-rail")
                    .w(px(36.0))
                    .h(px(36.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(10.0))
                    .bg(accent)
                    .text_color(rgb(0xffffff))
                    .text_lg()
                    .cursor_pointer()
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::new_conversation))
                    .child("+"),
            )
            .child(div().h(px(10.0)))
            .child(
                div()
                    .id("provider-rail")
                    .w(px(36.0))
                    .h(px(36.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(10.0))
                    .text_size(px(17.0))
                    .text_color(if self.panel == Panel::Provider {
                        accent
                    } else {
                        muted
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(selected))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::toggle_provider))
                    .child("◈"),
            )
            .child(
                div()
                    .id("mcp-rail")
                    .w(px(36.0))
                    .h(px(36.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(10.0))
                    .text_size(px(18.0))
                    .text_color(if self.panel == Panel::Mcp {
                        accent
                    } else {
                        muted
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(selected))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::toggle_mcp))
                    .child("⌁"),
            )
            .child(div().flex_1())
            .child(
                div()
                    .w(px(36.0))
                    .h(px(36.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(10.0))
                    .text_color(muted)
                    .text_size(px(16.0))
                    .child("⋯"),
            );
        let list = div()
            .w(px(214.0))
            .h_full()
            .flex()
            .flex_col()
            .bg(list_surface)
            .border_r_1()
            .border_color(border)
            .px_3()
            .py_3()
            .text_color(text)
            .child(
                div()
                    .h(px(34.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Cakify"),
                    )
                    .child(div().text_xs().text_color(muted).child("工作台")),
            )
            .child(
                div()
                    .mt_4()
                    .h(px(34.0))
                    .rounded(px(9.0))
                    .bg(rgb(0xeff1f5))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_sm()
                    .text_color(muted)
                    .child("⌕")
                    .child("搜索会话"),
            )
            .child(
                div()
                    .mt_5()
                    .mb_2()
                    .px_1()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(muted)
                    .child("会话"),
            )
            .child(
                div()
                    .id("new-conversation")
                    .h(px(48.0))
                    .rounded(px(10.0))
                    .bg(selected)
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::new_conversation))
                    .child(conversation_glyph(accent, "✦"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("新会话"),
                            )
                            .child(div().text_xs().text_color(muted).child(self.status.clone())),
                    ),
            )
            .child(
                div()
                    .mt_5()
                    .mb_2()
                    .px_1()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(muted)
                    .child("快捷入口"),
            )
            .child(
                div()
                    .id("provider-sidebar-link")
                    .h(px(38.0))
                    .rounded(px(9.0))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_sm()
                    .text_color(if self.panel == Panel::Provider {
                        accent
                    } else {
                        muted
                    })
                    .cursor_pointer()
                    .when(self.panel == Panel::Provider, |view| view.bg(selected))
                    .hover(|style| style.bg(selected))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::toggle_provider))
                    .child(conversation_glyph(accent, "◈"))
                    .child("Provider 设置"),
            )
            .child(
                div()
                    .id("mcp-sidebar-link")
                    .h(px(38.0))
                    .rounded(px(9.0))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_sm()
                    .text_color(if self.panel == Panel::Mcp {
                        accent
                    } else {
                        muted
                    })
                    .cursor_pointer()
                    .when(self.panel == Panel::Mcp, |view| view.bg(selected))
                    .hover(|style| style.bg(selected))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::toggle_mcp))
                    .child(conversation_glyph(accent, "⌁"))
                    .child("MCP 工具"),
            )
            .child(div().flex_1())
            .child(
                div()
                    .border_t_1()
                    .border_color(border)
                    .pt_3()
                    .text_xs()
                    .text_color(muted)
                    .child(format!("Core r{}", self.revision)),
            );
        div().h_full().flex().child(rail).child(list)
    }

    fn render_header(&self, cx: &mut Context<Self>) -> Div {
        let surface = rgb(0xfbfbfd);
        let border = rgb(0xe4e6ed);
        let text = rgb(0x262a34);
        let muted = rgb(0x7b8190);
        let hover = rgb(0xf0f2f7);
        let model = self.model_editor.read(cx).text(cx);
        div()
            .h(px(64.0))
            .flex()
            .items_center()
            .justify_between()
            .px_6()
            .bg(surface)
            .border_b_1()
            .border_color(border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(30.0))
                            .h(px(30.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(20.0))
                            .text_color(muted)
                            .child("☰"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_color(text)
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("新会话"),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .rounded(px(6.0))
                                    .bg(rgb(0xeff1f7))
                                    .text_xs()
                                    .text_color(muted)
                                    .child("当前"),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(7.0)).h(px(7.0)).rounded(px(99.0)).bg(
                        if self.provider_router.is_configured() {
                            rgb(0x1b986b)
                        } else {
                            rgb(0xd7a13e)
                        },
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_end()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(text)
                                    .child(if model.is_empty() {
                                        "未选择模型".to_owned()
                                    } else {
                                        model
                                    }),
                            )
                            .child(div().text_xs().text_color(muted).child(
                                if self.provider_router.is_configured() {
                                    "Provider 已连接"
                                } else {
                                    "需要配置 Provider"
                                },
                            )),
                    )
                    .child(
                        div()
                            .id("open-provider")
                            .w(px(30.0))
                            .h(px(30.0))
                            .ml_2()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(8.0))
                            .text_color(muted)
                            .text_size(px(16.0))
                            .cursor_pointer()
                            .hover(|style| style.bg(hover))
                            .on_mouse_up(MouseButton::Left, cx.listener(Self::toggle_provider))
                            .child("⚙"),
                    ),
            )
    }

    fn render_messages(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let background = rgb(0xfbfbfd);
        let muted = rgb(0x7b8190);
        let text = rgb(0x262a34);
        let soft = rgb(0xeff1f8);
        if self.messages.is_empty() {
            return div()
                .id("messages-scroll")
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .bg(background)
                .child(
                    div()
                        .w_full()
                        .max_w(px(620.0))
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_3()
                        .px_6()
                        .child(brand_mark(52.0))
                        .child(
                            div()
                                .mt_2()
                                .text_size(px(26.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(text)
                                .child("从一个问题开始"),
                        )
                        .child(div().text_sm().text_color(muted).child(
                            if self.provider_router.is_configured() {
                                "流式输出、工具审批和结果都会显示在这里"
                            } else {
                                "先配置一个 Provider，再开始你的第一段对话"
                            },
                        ))
                        .child(
                            div().mt_4().w_full().flex().gap_2().children(
                                [
                                    ("总结这段内容", "把重点整理成清晰的要点"),
                                    ("解释一个概念", "用简单的方式拆解复杂问题"),
                                    ("帮我写一段代码", "从需求到可运行的实现"),
                                ]
                                .into_iter()
                                .map(|(title, detail)| {
                                    let prompt = title.to_owned();
                                    div()
                                        .id(("suggestion", title))
                                        .flex_1()
                                        .rounded(px(10.0))
                                        .border_1()
                                        .border_color(rgb(0xe1e4ec))
                                        .bg(rgb(0xffffff))
                                        .p_3()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .cursor_pointer()
                                        .hover(|style| style.bg(soft))
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(move |app, event, window, cx| {
                                                app.use_suggestion(
                                                    prompt.clone(),
                                                    event,
                                                    window,
                                                    cx,
                                                );
                                            }),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(text)
                                                .child(title),
                                        )
                                        .child(div().text_xs().text_color(muted).child(detail))
                                }),
                            ),
                        ),
                );
        }

        div()
            .id("messages-scroll")
            .flex_1()
            .overflow_scroll()
            .bg(background)
            .px_5()
            .py_6()
            .child(
                div()
                    .w_full()
                    .max_w(px(760.0))
                    .mx_auto()
                    .flex()
                    .flex_col()
                    .gap_6()
                    .children(
                        self.messages
                            .iter()
                            .map(|message| self.render_message(message, cx)),
                    ),
            )
    }

    fn render_message(&self, message: &UiMessage, cx: &mut Context<Self>) -> Div {
        let text = rgb(0x262a34);
        let muted = rgb(0x7b8190);
        let user_surface = rgb(0xe8ecf8);
        let accent = rgb(0x5368a5);
        let error = rgb(0xc65460);
        match message.role {
            MessageRole::User => div()
                .w_full()
                .flex()
                .justify_end()
                .gap_2()
                .child(
                    div()
                        .max_w(px(560.0))
                        .rounded(px(12.0))
                        .bg(user_surface)
                        .px_4()
                        .py_3()
                        .text_color(text)
                        .line_height(px(22.0))
                        .child(message.content.clone()),
                )
                .child(conversation_glyph(accent, "我")),
            MessageRole::Assistant => {
                let run_tools = message.run_id.map_or_else(Vec::new, |run_id| {
                    self.tool_calls
                        .iter()
                        .filter(|call| call.run_id == run_id)
                        .cloned()
                        .collect::<Vec<_>>()
                });
                let mut body = div()
                    .w_full()
                    .flex()
                    .items_start()
                    .gap_3()
                    .text_color(text)
                    .child(conversation_glyph(accent, "C"));
                let mut content = div().flex_1().flex().flex_col().gap_3();
                content = content.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(text)
                                .child("Cakify"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(match message.state {
                                    MessageState::Sending => "准备中",
                                    MessageState::Streaming => "正在生成",
                                    MessageState::Complete => "已完成",
                                    MessageState::Cancelled => "已停止",
                                    MessageState::Error => "需要处理",
                                }),
                        ),
                );
                if !message.content.is_empty() {
                    content = content.child(render_markdown(&message.content));
                } else if message.state == MessageState::Sending
                    || message.state == MessageState::Streaming
                {
                    content = content.child(div().text_sm().text_color(muted).child("正在生成…"));
                }
                content = content.children(
                    run_tools
                        .into_iter()
                        .map(|call| self.render_tool_call(call, cx)),
                );
                if let Some(error_message) = &message.error {
                    content = content.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .text_sm()
                            .text_color(error)
                            .child(error_message.clone())
                            .child(
                                div()
                                    .id("retry-last")
                                    .cursor_pointer()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .on_mouse_up(MouseButton::Left, cx.listener(Self::retry_last))
                                    .child("重试"),
                            ),
                    );
                }
                if message.state == MessageState::Cancelled {
                    content = content.child(div().text_xs().text_color(muted).child("已停止"));
                }
                if let Some(usage) = &message.usage {
                    content =
                        content.child(div().text_xs().text_color(muted).child(format_usage(usage)));
                }
                body = body.child(content);
                body
            }
        }
    }

    fn render_tool_call(&self, call: UiToolCall, cx: &mut Context<Self>) -> Div {
        let surface = rgb(0xf3f4f8);
        let border = rgb(0xdfe2ea);
        let text = rgb(0x2f3440);
        let muted = rgb(0x7b8190);
        let accent = rgb(0x5368a5);
        let danger = rgb(0xc65460);
        let state = match call.state {
            ToolApprovalState::Streaming => "接收参数",
            ToolApprovalState::AwaitingApproval => "等待审批",
            ToolApprovalState::Approved => "已允许，等待执行",
            ToolApprovalState::Executing => "执行中",
            ToolApprovalState::Complete => "已完成",
            ToolApprovalState::Failed => "执行失败",
            ToolApprovalState::Denied => "已拒绝",
        };
        let mut view = div()
            .rounded(px(6.0))
            .border_1()
            .border_color(border)
            .bg(surface)
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(text)
                            .child(if call.name.is_empty() {
                                "工具调用".to_owned()
                            } else {
                                call.name.clone()
                            }),
                    )
                    .child(div().text_xs().text_color(muted).child(state)),
            )
            .child(
                div()
                    .id((
                        gpui::ElementId::from(("tool-arguments", call.run_id.value())),
                        call.index.to_string(),
                    ))
                    .max_h(px(140.0))
                    .overflow_y_scroll()
                    .text_xs()
                    .font_family("Cascadia Mono")
                    .text_color(muted)
                    .child(if call.arguments_json.is_empty() {
                        "{}".to_owned()
                    } else {
                        call.arguments_json.clone()
                    }),
            );
        if let Some(output) = &call.output {
            view = view.child(
                div()
                    .id((
                        gpui::ElementId::from(("tool-output", call.run_id.value())),
                        call.index.to_string(),
                    ))
                    .max_h(px(140.0))
                    .overflow_y_scroll()
                    .border_t_1()
                    .border_color(border)
                    .pt_2()
                    .text_xs()
                    .font_family("Cascadia Mono")
                    .text_color(if call.state == ToolApprovalState::Failed {
                        danger
                    } else {
                        text
                    })
                    .child(output.clone()),
            );
        }
        if call.state == ToolApprovalState::AwaitingApproval {
            let approve_id = call.id.clone();
            let deny_id = call.id.clone();
            let run_id = call.run_id;
            view = view.child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        div()
                            .id((
                                gpui::ElementId::from(("deny-tool", call.run_id.value())),
                                call.index.to_string(),
                            ))
                            .px_3()
                            .py_1()
                            .text_sm()
                            .text_color(danger)
                            .cursor_pointer()
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |app, _, _, cx| {
                                    app.resolve_tool(run_id, deny_id.clone(), false, cx);
                                }),
                            )
                            .child("拒绝"),
                    )
                    .child(
                        div()
                            .id((
                                gpui::ElementId::from(("approve-tool", call.run_id.value())),
                                call.index.to_string(),
                            ))
                            .rounded(px(5.0))
                            .bg(accent)
                            .text_color(rgb(0xffffff))
                            .px_3()
                            .py_1()
                            .text_sm()
                            .cursor_pointer()
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |app, _, _, cx| {
                                    app.resolve_tool(run_id, approve_id.clone(), true, cx);
                                }),
                            )
                            .child("允许"),
                    ),
            );
        }
        view
    }

    fn render_composer(&self, cx: &mut Context<Self>) -> Div {
        let surface = rgb(0xfbfbfd);
        let field = rgb(0xffffff);
        let border = rgb(0xdfe2ea);
        let accent = rgb(0x5368a5);
        let muted = rgb(0x7b8190);
        let focus_handle = self.composer.read(cx).focus_handle.clone();
        let send_label = if self.active_run.is_some() {
            "■"
        } else {
            "↑"
        };
        div()
            .bg(surface)
            .border_t_1()
            .border_color(border)
            .px_6()
            .pt_4()
            .pb_3()
            .child(
                div()
                    .max_w(px(780.0))
                    .mx_auto()
                    .rounded(px(12.0))
                    .border_1()
                    .border_color(border)
                    .bg(field)
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .gap_2()
                            .child(
                                div()
                                    .id("chat-composer")
                                    .key_context("TextInput")
                                    .track_focus(&focus_handle)
                                    .cursor(CursorStyle::IBeam)
                                    .map(editor_actions(self.composer.clone()))
                                    .flex_1()
                                    .h(px(58.0))
                                    .overflow_hidden()
                                    .p_1()
                                    .line_height(px(22.0))
                                    .text_size(px(14.0))
                                    .child(self.composer.clone()),
                            )
                            .child(
                                div()
                                    .id("send-message")
                                    .w(px(38.0))
                                    .h(px(38.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(10.0))
                                    .bg(if self.active_run.is_some() {
                                        rgb(0xc65460)
                                    } else {
                                        accent
                                    })
                                    .text_color(rgb(0xffffff))
                                    .text_size(px(18.0))
                                    .cursor_pointer()
                                    .on_mouse_up(MouseButton::Left, cx.listener(Self::send_click))
                                    .child(send_label),
                            ),
                    )
                    .child(
                        div()
                            .h(px(24.0))
                            .flex()
                            .items_center()
                            .gap_3()
                            .text_xs()
                            .text_color(muted)
                            .child(
                                div()
                                    .id("open-mcp-from-composer")
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .cursor_pointer()
                                    .hover(|style| style.text_color(accent))
                                    .on_mouse_up(MouseButton::Left, cx.listener(Self::toggle_mcp))
                                    .child("⌁")
                                    .child("工具"),
                            )
                            .child(div().text_color(rgb(0xa7acb7)).child("·"))
                            .child("支持 Markdown 与多行输入"),
                    ),
            )
            .child(
                div()
                    .max_w(px(780.0))
                    .mx_auto()
                    .mt_2()
                    .flex()
                    .justify_between()
                    .text_xs()
                    .text_color(muted)
                    .child(self.status.clone())
                    .child("Enter 发送 · Shift+Enter 换行"),
            )
    }

    fn render_provider_panel(&self, cx: &mut Context<Self>) -> Div {
        let surface = rgb(0xf8f9fc);
        let field = rgb(0xffffff);
        let border = rgb(0xdfe2ea);
        let text = rgb(0x262a34);
        let muted = rgb(0x7b8190);
        let accent = rgb(0x5368a5);
        div()
            .w(px(326.0))
            .h_full()
            .flex()
            .flex_col()
            .bg(surface)
            .border_l_1()
            .border_color(border)
            .p_4()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(text)
                            .child("Provider 设置"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().text_xs().text_color(muted).child(
                                if self.provider_router.is_configured() {
                                    "已配置"
                                } else {
                                    "未配置"
                                },
                            ))
                            .child(
                                div()
                                    .id("close-provider")
                                    .w(px(28.0))
                                    .h(px(28.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(8.0))
                                    .cursor_pointer()
                                    .hover(|style| style.bg(rgb(0xe9ecf3)))
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(Self::toggle_provider),
                                    )
                                    .child("×"),
                            ),
                    ),
            )
            .child(labeled_input(
                "Endpoint",
                "provider-endpoint",
                self.endpoint_editor.clone(),
                field,
                border,
                cx,
            ))
            .child(labeled_input(
                "模型 ID",
                "provider-model",
                self.model_editor.clone(),
                field,
                border,
                cx,
            ))
            .child(labeled_input(
                "API Key",
                "provider-key",
                self.key_editor.clone(),
                field,
                border,
                cx,
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("API Key 保存到 Windows Credential Manager"),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("save-provider")
                    .rounded(px(5.0))
                    .bg(accent)
                    .text_color(rgb(0xffffff))
                    .py_2()
                    .text_sm()
                    .text_center()
                    .cursor_pointer()
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::save_provider))
                    .child("保存"),
            )
    }

    fn render_mcp_panel(&self, cx: &mut Context<Self>) -> Div {
        let surface = rgb(0xf8f9fc);
        let field = rgb(0xffffff);
        let border = rgb(0xdfe2ea);
        let text = rgb(0x262a34);
        let muted = rgb(0x7b8190);
        let accent = rgb(0x5368a5);
        let selected = rgb(0xe7ebf6);
        div()
            .w(px(326.0))
            .h_full()
            .flex()
            .flex_col()
            .bg(surface)
            .border_l_1()
            .border_color(border)
            .p_4()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(text)
                            .child("MCP Servers"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(format!("{} 个", self.mcp_servers.len())),
                            )
                            .child(
                                div()
                                    .id("close-mcp")
                                    .w(px(28.0))
                                    .h(px(28.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(8.0))
                                    .cursor_pointer()
                                    .hover(|style| style.bg(rgb(0xe9ecf3)))
                                    .on_mouse_up(MouseButton::Left, cx.listener(Self::toggle_mcp))
                                    .child("×"),
                            ),
                    ),
            )
            .children(self.mcp_servers.iter().enumerate().map(|(index, server)| {
                let toggle_id = server.id.clone();
                let delete_id = server.id.clone();
                let next_enabled = !server.enabled;
                let expected_updated_at = server.updated_at;
                div()
                    .id(("mcp-server", index))
                    .border_b_1()
                    .border_color(border)
                    .py_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(server.name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(mcp_connection_label(server)),
                            ),
                    )
                    .child(div().mt_1().text_xs().text_color(muted).child(format!(
                        "{} · {}",
                        match server.transport {
                            McpTransport::Stdio => "stdio",
                            McpTransport::Http => "HTTP",
                        },
                        server.target
                    )))
                    .when(
                        matches!(&server.connection, McpConnectionState::Failed(_)),
                        |view| {
                            let McpConnectionState::Failed(message) = &server.connection else {
                                return view;
                            };
                            view.child(
                                div()
                                    .id(("mcp-connection-error", index))
                                    .mt_1()
                                    .max_h(px(54.0))
                                    .overflow_y_scroll()
                                    .text_xs()
                                    .text_color(rgb(0xa13d32))
                                    .child(message.clone()),
                            )
                        },
                    )
                    .child(
                        div()
                            .mt_2()
                            .flex()
                            .justify_end()
                            .gap_3()
                            .child(
                                div()
                                    .id(("delete-mcp", index))
                                    .text_xs()
                                    .text_color(rgb(0xa13d32))
                                    .cursor_pointer()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(move |app, _, _, cx| {
                                            app.delete_mcp_server(delete_id.clone(), cx);
                                        }),
                                    )
                                    .child("删除"),
                            )
                            .child(
                                div()
                                    .id(("toggle-mcp", index))
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(accent)
                                    .cursor_pointer()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(move |app, _, _, cx| {
                                            app.set_mcp_server_enabled(
                                                toggle_id.clone(),
                                                next_enabled,
                                                expected_updated_at,
                                                cx,
                                            );
                                        }),
                                    )
                                    .child(if server.enabled { "停用" } else { "启用" }),
                            ),
                    )
            }))
            .child(div().flex_1())
            .child(labeled_input(
                "名称",
                "mcp-name",
                self.mcp_name_editor.clone(),
                field,
                border,
                cx,
            ))
            .child(labeled_input(
                "目标",
                "mcp-target",
                self.mcp_target_editor.clone(),
                field,
                border,
                cx,
            ))
            .when(self.mcp_transport == McpTransport::Stdio, |view| {
                view.child(labeled_input(
                    "参数（JSON 数组）",
                    "mcp-args",
                    self.mcp_args_editor.clone(),
                    field,
                    border,
                    cx,
                ))
            })
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(
                        div()
                            .id("mcp-stdio")
                            .flex_1()
                            .rounded(px(5.0))
                            .py_2()
                            .text_sm()
                            .text_center()
                            .cursor_pointer()
                            .when(self.mcp_transport == McpTransport::Stdio, |style| {
                                style.bg(selected)
                            })
                            .on_mouse_up(MouseButton::Left, cx.listener(Self::select_stdio))
                            .child("stdio"),
                    )
                    .child(
                        div()
                            .id("mcp-http")
                            .flex_1()
                            .rounded(px(5.0))
                            .py_2()
                            .text_sm()
                            .text_center()
                            .cursor_pointer()
                            .when(self.mcp_transport == McpTransport::Http, |style| {
                                style.bg(selected)
                            })
                            .on_mouse_up(MouseButton::Left, cx.listener(Self::select_http))
                            .child("HTTP"),
                    ),
            )
            .child(
                div()
                    .id("add-mcp-server")
                    .rounded(px(5.0))
                    .border_1()
                    .border_color(accent)
                    .text_color(accent)
                    .py_2()
                    .text_sm()
                    .text_center()
                    .cursor_pointer()
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::add_mcp_server))
                    .child("添加 Server"),
            )
    }
}

impl Render for CakifyApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .bg(rgb(0xfbfbfd))
            .text_color(rgb(0x262a34))
            .on_action(cx.listener(Self::submit_from_keyboard))
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(self.render_header(cx))
                    .child(self.render_messages(cx))
                    .child(self.render_composer(cx)),
            )
            .when(self.panel == Panel::Provider, |view| {
                view.child(self.render_provider_panel(cx))
            })
            .when(self.panel == Panel::Mcp, |view| {
                view.child(self.render_mcp_panel(cx))
            })
    }
}

impl Drop for CakifyApp {
    fn drop(&mut self) {
        let _ = self.core.handle().shutdown();
    }
}

fn brand_mark(size: f32) -> Div {
    let accent = rgb(0x5368a5);
    let ink = rgb(0x2f394d);
    div()
        .w(px(size))
        .h(px(size))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(size * 0.28))
        .bg(rgb(0xe8ebf4))
        .text_size(px(size * 0.53))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(accent)
        .child(
            div()
                .w(px(size * 0.58))
                .flex()
                .flex_col()
                .items_start()
                .gap_1()
                .child(
                    div()
                        .w_full()
                        .h(px(size * 0.14))
                        .rounded(px(size * 0.08))
                        .bg(ink),
                )
                .child(
                    div()
                        .w(px(size * 0.38))
                        .h(px(size * 0.14))
                        .rounded(px(size * 0.08))
                        .bg(accent),
                )
                .child(
                    div()
                        .w_full()
                        .h(px(size * 0.14))
                        .rounded(px(size * 0.08))
                        .bg(ink),
                ),
        )
}

fn conversation_glyph(color: gpui::Rgba, label: &'static str) -> Div {
    div()
        .w(px(28.0))
        .h(px(28.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(9.0))
        .bg(rgb(0xe8ebf4))
        .text_size(px(12.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .child(label)
}

fn labeled_input(
    label: &'static str,
    id: &'static str,
    editor: Entity<TextEditor>,
    background: gpui::Rgba,
    border: gpui::Rgba,
    cx: &mut Context<CakifyApp>,
) -> Div {
    let focus_handle = editor.read(cx).focus_handle.clone();
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_xs().text_color(rgb(0x7b8190)).child(label))
        .child(
            div()
                .id(id)
                .key_context("TextInput")
                .track_focus(&focus_handle)
                .cursor(CursorStyle::IBeam)
                .map(editor_actions(editor.clone()))
                .h(px(38.0))
                .rounded(px(5.0))
                .border_1()
                .border_color(border)
                .bg(background)
                .px_2()
                .flex()
                .items_center()
                .overflow_hidden()
                .line_height(px(20.0))
                .text_size(px(13.0))
                .child(editor),
        )
}

fn render_markdown(source: &str) -> Div {
    let text = rgb(0x262a34);
    let muted = rgb(0x7b8190);
    let code_surface = rgb(0xf2f3f7);
    let border = rgb(0xdfe2ea);
    div().w_full().flex().flex_col().gap_3().children(
        parse_markdown(source)
            .into_iter()
            .enumerate()
            .map(|(index, block)| match block {
                MarkdownBlock::Paragraph(text_value) => div()
                    .w_full()
                    .line_height(px(23.0))
                    .text_color(text)
                    .child(text_value),
                MarkdownBlock::Heading { level, text: value } => div()
                    .w_full()
                    .mt_2()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_size(px(match level {
                        1 => 21.0,
                        2 => 19.0,
                        3 => 17.0,
                        _ => 15.0,
                    }))
                    .child(value),
                MarkdownBlock::Code {
                    language,
                    text: value,
                } => div()
                    .w_full()
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(border)
                    .bg(code_surface)
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(border)
                            .text_xs()
                            .text_color(muted)
                            .child(if language.is_empty() {
                                "代码".to_owned()
                            } else {
                                language
                            }),
                    )
                    .child(
                        div()
                            .id(("markdown-code", index))
                            .max_h(px(360.0))
                            .overflow_x_scroll()
                            .p_3()
                            .font_family("Cascadia Mono")
                            .text_size(px(13.0))
                            .line_height(px(20.0))
                            .child(value),
                    ),
                MarkdownBlock::Quote(value) => div()
                    .w_full()
                    .border_l_2()
                    .border_color(rgb(0x8290bd))
                    .pl_3()
                    .text_color(muted)
                    .child(value),
                MarkdownBlock::ListItem {
                    ordered,
                    text: value,
                } => div()
                    .w_full()
                    .flex()
                    .gap_2()
                    .child(if ordered { "1." } else { "•" })
                    .child(div().flex_1().child(value)),
                MarkdownBlock::Rule => div().w_full().h(px(1.0)).bg(border),
            }),
    )
}

fn upsert_provider_profile(
    storage: &StorageHandle,
    existing: Option<&ProviderProfileRecord>,
    endpoint: String,
    model: String,
    credential_ref: Option<String>,
) -> Result<ProviderProfileRecord, StorageError> {
    let now = unix_millis().map_err(|reason| StorageError::InvalidInput {
        field: "provider.updated_at",
        reason,
    })?;
    if let Some(existing) = existing {
        return storage.update_provider_profile(ProviderProfileUpdate {
            id: existing.id.clone(),
            kind: "openai-compatible".to_owned(),
            endpoint: Some(endpoint),
            display_name: "OpenAI Compatible".to_owned(),
            credential_ref,
            default_model: Some(model),
            metadata_json: "{}".to_owned(),
            expected_updated_at: existing.updated_at,
            updated_at: now.max(existing.updated_at + 1),
            models: None,
        });
    }
    let mut profile =
        NewProviderProfile::new(PROVIDER_ID, "openai-compatible", "OpenAI Compatible", now);
    profile.endpoint = Some(endpoint);
    profile.credential_ref = credential_ref;
    profile.default_model = Some(model);
    storage.create_provider_profile(profile)
}

fn provider_from_profile(
    profile: &ProviderProfileRecord,
    secrets: Arc<dyn SecretStore>,
) -> Result<Arc<OpenAiCompatibleProvider>, String> {
    let endpoint = profile
        .endpoint
        .as_deref()
        .ok_or_else(|| "Provider endpoint 缺失".to_owned())?;
    let credential = profile
        .credential_ref
        .as_deref()
        .map(SecretId::new)
        .transpose()
        .map_err(|error| error.to_string())?;
    let config = OpenAiConfig::new(endpoint, credential).map_err(|error| error.to_string())?;
    OpenAiCompatibleProvider::new(config, secrets)
        .map(Arc::new)
        .map_err(|error| error.to_string())
}

fn mcp_record_to_ui(record: McpServerRecord) -> McpServerUi {
    let target = record.target().unwrap_or_else(|| "配置不可用".to_owned());
    McpServerUi {
        id: record.id,
        name: record.display_name,
        target,
        transport: match record.transport {
            StoredMcpTransport::Stdio => McpTransport::Stdio,
            StoredMcpTransport::StreamableHttp => McpTransport::Http,
        },
        enabled: record.enabled,
        updated_at: record.updated_at,
        connection: if record.enabled {
            McpConnectionState::Connecting
        } else {
            McpConnectionState::Disabled
        },
    }
}

fn mcp_connection_label(server: &McpServerUi) -> String {
    match &server.connection {
        McpConnectionState::Disabled => {
            if server.enabled {
                "已启用 · 未连接".to_owned()
            } else {
                "已停用".to_owned()
            }
        }
        McpConnectionState::Connecting => "连接中".to_owned(),
        McpConnectionState::Connected { tool_count } => format!("已连接 · {tool_count} 工具"),
        McpConnectionState::Failed(_) => "连接失败".to_owned(),
    }
}

fn unix_millis() -> Result<i64, String> {
    let millis = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    i64::try_from(millis).map_err(|_| "系统时间超出范围".to_owned())
}

fn next_timestamp(expected: i64) -> Result<i64, String> {
    let minimum = expected
        .checked_add(1)
        .ok_or_else(|| "配置时间戳超出范围".to_owned())?;
    unix_millis().map(|now| now.max(minimum))
}

fn format_usage(usage: &Usage) -> String {
    match (usage.input_tokens, usage.output_tokens, usage.total_tokens) {
        (_, _, Some(total)) => format!("{total} tokens"),
        (Some(input), Some(output), None) => format!("{input} in · {output} out"),
        _ => "usage 已返回".to_owned(),
    }
}

fn main() {
    let services = DesktopServices::initialize().expect("initialize Cakify desktop services");
    application().run(move |cx: &mut App| {
        bind_input_keys(cx);
        let bounds = Bounds::centered(None, size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Cakify".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |window, cx| {
                cx.new(|cx| {
                    let mut app = CakifyApp::new(services, window, cx);
                    app.start(cx);
                    app
                })
            },
        )
        .expect("open Cakify window");
        cx.activate(true);
    });
}
