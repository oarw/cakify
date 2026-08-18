mod composer;
mod markdown;

use std::{sync::Arc, time::SystemTime};

use cakify_core::{
    put_then_commit_reference, AppCommand, AppEvent, ConversationId, CoreEvents, CoreRuntime,
    RequestId, RunId, SecretId, SecretInput, SecretStore, ToolCall, Usage,
};
use cakify_platform_windows::{app_data_paths, CredentialManagerSecretStore};
use cakify_provider::{OpenAiCompatibleProvider, OpenAiConfig, ProviderRouter};
use cakify_storage::{
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

struct DesktopServices {
    core: CoreRuntime,
    events: CoreEvents,
    storage_actor: StorageActor,
    storage: StorageHandle,
    secrets: Arc<dyn SecretStore>,
    provider_router: Arc<ProviderRouter>,
    provider_profile: Option<ProviderProfileRecord>,
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

        let startup_status = if let Some(profile) = &provider_profile {
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
        let core = CoreRuntime::start_with_provider(provider_router.clone())
            .map_err(|error| error.to_string())?;
        let events = core.events();

        Ok(Self {
            core,
            events,
            storage_actor,
            storage,
            secrets,
            provider_router,
            provider_profile,
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
    Denied,
}

#[derive(Clone)]
struct UiToolCall {
    run_id: RunId,
    index: u32,
    id: String,
    name: String,
    arguments_json: String,
    state: ToolApprovalState,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum McpTransport {
    Stdio,
    Http,
}

struct McpServerUi {
    name: String,
    target: String,
    transport: McpTransport,
    enabled: bool,
}

struct CakifyApp {
    core: CoreRuntime,
    events: CoreEvents,
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
        let composer = cx.new(|cx| TextEditor::new("", "输入消息", false, true, window, cx));
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

        Self {
            core: services.core,
            events: services.events,
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
            panel: Panel::None,
            mcp_transport: McpTransport::Stdio,
            mcp_servers: Vec::new(),
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
        let _ = self.core.handle().try_dispatch(AppCommand::Bootstrap);
        self.request_new_conversation();
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

        match self.core.handle().try_dispatch(AppCommand::SubmitDraft {
            request_id,
            conversation_id,
            model,
            text,
            tools: Vec::new(),
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
        self.mcp_servers.push(McpServerUi {
            name,
            target,
            transport: self.mcp_transport,
            enabled: false,
        });
        self.mcp_name_editor
            .update(cx, |editor, cx| editor.clear(cx));
        self.mcp_target_editor
            .update(cx, |editor, cx| editor.clear(cx));
        self.status = "MCP Server 草稿已添加；连接器尚未启用".into();
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
                    "已允许工具调用，等待执行器".into()
                } else {
                    "已拒绝工具调用".into()
                };
            }
            Err(error) => self.status = error.to_string().into(),
        }
        cx.notify();
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> Div {
        let surface = rgb(0xf7f8f7);
        let border = rgb(0xdde1df);
        let text = rgb(0x1c2522);
        let muted = rgb(0x68736f);
        let selected = rgb(0xe7eeeb);
        div()
            .w(px(208.0))
            .h_full()
            .flex()
            .flex_col()
            .bg(surface)
            .border_r_1()
            .border_color(border)
            .px_3()
            .py_4()
            .text_color(text)
            .child(
                div()
                    .h(px(36.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Cakify"),
                    )
                    .child(
                        div()
                            .id("new-conversation")
                            .w(px(30.0))
                            .h(px(30.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(5.0))
                            .text_lg()
                            .cursor_pointer()
                            .hover(|style| style.bg(selected))
                            .on_mouse_up(MouseButton::Left, cx.listener(Self::new_conversation))
                            .child("+"),
                    ),
            )
            .child(
                div()
                    .mt_5()
                    .mb_2()
                    .text_xs()
                    .text_color(muted)
                    .child("会话"),
            )
            .child(
                div()
                    .rounded(px(5.0))
                    .bg(selected)
                    .px_3()
                    .py_2()
                    .text_sm()
                    .child("新会话"),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("open-mcp")
                    .rounded(px(5.0))
                    .px_3()
                    .py_2()
                    .text_sm()
                    .cursor_pointer()
                    .hover(|style| style.bg(selected))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::toggle_mcp))
                    .child(format!("MCP  ·  {}", self.mcp_servers.len())),
            )
            .child(
                div()
                    .mt_2()
                    .px_3()
                    .text_xs()
                    .text_color(muted)
                    .child(format!("Core r{}", self.revision)),
            )
    }

    fn render_header(&self, cx: &mut Context<Self>) -> Div {
        let surface = rgb(0xfcfdfc);
        let border = rgb(0xdde1df);
        let text = rgb(0x1c2522);
        let muted = rgb(0x68736f);
        let hover = rgb(0xf0f3f1);
        let model = self.model_editor.read(cx).text(cx);
        div()
            .h(px(58.0))
            .flex()
            .items_center()
            .justify_between()
            .px_5()
            .bg(surface)
            .border_b_1()
            .border_color(border)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(text)
                            .child("新会话"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(if model.is_empty() {
                                "未选择模型".to_owned()
                            } else {
                                model
                            }),
                    ),
            )
            .child(
                div()
                    .id("open-provider")
                    .rounded(px(5.0))
                    .border_1()
                    .border_color(border)
                    .px_3()
                    .py_2()
                    .text_sm()
                    .cursor_pointer()
                    .hover(|style| style.bg(hover))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::toggle_provider))
                    .child(if self.provider_router.is_configured() {
                        "Provider"
                    } else {
                        "配置 Provider"
                    }),
            )
    }

    fn render_messages(&self, cx: &mut Context<Self>) -> Div {
        let background = rgb(0xfcfdfc);
        let muted = rgb(0x68736f);
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
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("开始一段对话"),
                        )
                        .child(div().text_sm().text_color(muted).child(
                            if self.provider_router.is_configured() {
                                self.model_editor.read(cx).text(cx)
                            } else {
                                "尚未配置 Provider".to_owned()
                            },
                        )),
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
        let text = rgb(0x1c2522);
        let muted = rgb(0x68736f);
        let user_surface = rgb(0xe8eeeb);
        let error = rgb(0xa13d32);
        match message.role {
            MessageRole::User => div().w_full().flex().justify_end().child(
                div()
                    .max_w(px(560.0))
                    .rounded(px(7.0))
                    .bg(user_surface)
                    .px_4()
                    .py_3()
                    .text_color(text)
                    .child(message.content.clone()),
            ),
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
                    .flex_col()
                    .gap_3()
                    .text_color(text)
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(muted)
                            .child("Cakify"),
                    );
                if !message.content.is_empty() {
                    body = body.child(render_markdown(&message.content));
                } else if message.state == MessageState::Sending
                    || message.state == MessageState::Streaming
                {
                    body = body.child(div().text_sm().text_color(muted).child("正在生成…"));
                }
                body = body.children(
                    run_tools
                        .into_iter()
                        .map(|call| self.render_tool_call(call, cx)),
                );
                if let Some(error_message) = &message.error {
                    body = body.child(
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
                    body = body.child(div().text_xs().text_color(muted).child("已停止"));
                }
                if let Some(usage) = &message.usage {
                    body = body.child(div().text_xs().text_color(muted).child(format_usage(usage)));
                }
                body
            }
        }
    }

    fn render_tool_call(&self, call: UiToolCall, cx: &mut Context<Self>) -> Div {
        let surface = rgb(0xf3f5f4);
        let border = rgb(0xd8dedb);
        let text = rgb(0x26312d);
        let muted = rgb(0x68736f);
        let accent = rgb(0x126b50);
        let danger = rgb(0xa13d32);
        let state = match call.state {
            ToolApprovalState::Streaming => "接收参数",
            ToolApprovalState::AwaitingApproval => "等待审批",
            ToolApprovalState::Approved => "已允许，等待执行器",
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
                    .max_h(px(140.0))
                    .overflow_scroll()
                    .text_xs()
                    .font_family("Cascadia Mono")
                    .text_color(muted)
                    .child(if call.arguments_json.is_empty() {
                        "{}".to_owned()
                    } else {
                        call.arguments_json.clone()
                    }),
            );
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
                            .id(("deny-tool", call.run_id.value(), call.index as usize))
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
                            .id(("approve-tool", call.run_id.value(), call.index as usize))
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
        let surface = rgb(0xfcfdfc);
        let field = rgb(0xffffff);
        let border = rgb(0xd6ddda);
        let accent = rgb(0x126b50);
        let muted = rgb(0x68736f);
        let focus_handle = self.composer.read(cx).focus_handle.clone();
        let send_label = if self.active_run.is_some() {
            "停止"
        } else {
            "↑"
        };
        div()
            .bg(surface)
            .border_t_1()
            .border_color(border)
            .px_5()
            .pt_3()
            .pb_3()
            .child(
                div()
                    .max_w(px(760.0))
                    .mx_auto()
                    .rounded(px(7.0))
                    .border_1()
                    .border_color(border)
                    .bg(field)
                    .p_2()
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
                            .h(px(72.0))
                            .overflow_hidden()
                            .p_1()
                            .line_height(px(21.0))
                            .text_size(px(14.0))
                            .child(self.composer.clone()),
                    )
                    .child(
                        div()
                            .id("send-message")
                            .w(px(34.0))
                            .h(px(34.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(5.0))
                            .bg(accent)
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .on_mouse_up(MouseButton::Left, cx.listener(Self::send_click))
                            .child(send_label),
                    ),
            )
            .child(
                div()
                    .max_w(px(760.0))
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
        let surface = rgb(0xf8faf9);
        let field = rgb(0xffffff);
        let border = rgb(0xd6ddda);
        let text = rgb(0x1c2522);
        let muted = rgb(0x68736f);
        let accent = rgb(0x126b50);
        div()
            .w(px(310.0))
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
                            .child("Provider"),
                    )
                    .child(div().text_xs().text_color(muted).child(
                        if self.provider_router.is_configured() {
                            "已配置"
                        } else {
                            "未配置"
                        },
                    )),
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
        let surface = rgb(0xf8faf9);
        let field = rgb(0xffffff);
        let border = rgb(0xd6ddda);
        let text = rgb(0x1c2522);
        let muted = rgb(0x68736f);
        let accent = rgb(0x126b50);
        let selected = rgb(0xe5eeea);
        div()
            .w(px(310.0))
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
                            .text_xs()
                            .text_color(muted)
                            .child(self.mcp_servers.len().to_string()),
                    ),
            )
            .children(self.mcp_servers.iter().enumerate().map(|(index, server)| {
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
                            .child(div().text_xs().text_color(muted).child(if server.enabled {
                                "已启用"
                            } else {
                                "未连接"
                            })),
                    )
                    .child(div().mt_1().text_xs().text_color(muted).child(format!(
                        "{} · {}",
                        match server.transport {
                            McpTransport::Stdio => "stdio",
                            McpTransport::Http => "HTTP",
                        },
                        server.target
                    )))
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
            .bg(rgb(0xfcfdfc))
            .text_color(rgb(0x1c2522))
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

fn labeled_input(
    label: &'static str,
    id: &'static str,
    editor: Entity<TextEditor>,
    background: gpui::Hsla,
    border: gpui::Hsla,
    cx: &mut Context<CakifyApp>,
) -> Div {
    let focus_handle = editor.read(cx).focus_handle.clone();
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_xs().text_color(rgb(0x68736f)).child(label))
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
    let text = rgb(0x1c2522);
    let muted = rgb(0x68736f);
    let code_surface = rgb(0xf0f3f1);
    let border = rgb(0xd8dedb);
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_3()
        .children(parse_markdown(source).into_iter().map(|block| {
            match block {
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
                            .max_h(px(360.0))
                            .overflow_scroll()
                            .p_3()
                            .font_family("Cascadia Mono")
                            .text_size(px(13.0))
                            .line_height(px(20.0))
                            .child(value),
                    ),
                MarkdownBlock::Quote(value) => div()
                    .w_full()
                    .border_l_2()
                    .border_color(rgb(0x8aa69b))
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
            }
        }))
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

fn unix_millis() -> Result<i64, String> {
    let millis = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    i64::try_from(millis).map_err(|_| "系统时间超出范围".to_owned())
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
