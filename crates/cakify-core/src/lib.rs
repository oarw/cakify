//! Framework-neutral command/event boundary for the Cakify desktop client.

mod chat;
mod secret;
mod tool;

pub use chat::{
    ChatMessage, ChatProvider, ChatRequest, ChatRole, ChatToolCall, ChatToolFunction,
    MissingProvider, ProviderError, ProviderErrorKind, ProviderStreamEvent, StreamSink,
    ToolDefinition, Usage,
};
pub use secret::{
    delete_reference_then_secret, put_then_commit_reference, SecretError, SecretId, SecretInput,
    SecretLifecycleError, SecretStore, SecretValue,
};
pub use tool::{
    builtin_tool_definitions, BuiltinToolExecutor, ToolExecutionError, ToolExecutor,
    CURRENT_TIME_TOOL_NAME,
};

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, RecvTimeoutError, SyncSender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use async_channel::{Receiver, Sender, TrySendError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const COMMAND_CAPACITY: usize = 256;
pub const EVENT_CAPACITY: usize = 1_024;
const DELTA_FLUSH_INTERVAL: Duration = Duration::from_millis(24);
const DELTA_FLUSH_BYTES: usize = 1_024;
const APPROVAL_POLL_INTERVAL: Duration = Duration::from_millis(50);
const APPROVAL_QUEUE_CAPACITY: usize = 64;
const MAX_ASSISTANT_BYTES: usize = 8 * 1_024 * 1_024;
const MAX_TOOL_CALLS_PER_ROUND: usize = 16;
const MAX_TOOL_CALL_ID_BYTES: usize = 512;
const MAX_TOOL_NAME_BYTES: usize = 128;
const MAX_TOOL_ARGUMENT_BYTES: usize = 64 * 1_024;
const MAX_TOOL_OUTPUT_BYTES: usize = 64 * 1_024;
const MAX_TOOL_ROUNDS: usize = 4;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct RequestId(u64);

impl RequestId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ConversationId(u64);

impl ConversationId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RunId(u64);

impl RunId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolCall {
    pub index: u32,
    pub id: String,
    pub name: String,
    pub arguments_json: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum AppCommand {
    Bootstrap,
    CreateConversation {
        request_id: RequestId,
        title: String,
    },
    SubmitDraft {
        request_id: RequestId,
        conversation_id: ConversationId,
        model: String,
        text: String,
        tools: Vec<ToolDefinition>,
        temperature: Option<f32>,
    },
    CancelRun {
        run_id: RunId,
    },
    ResolveToolApproval {
        run_id: RunId,
        tool_call_id: String,
        approved: bool,
    },
    Shutdown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AppEvent {
    CoreReady {
        revision: u64,
    },
    ConversationCreated {
        request_id: RequestId,
        conversation_id: ConversationId,
        revision: u64,
    },
    DraftAccepted {
        request_id: RequestId,
        conversation_id: ConversationId,
        run_id: RunId,
        revision: u64,
    },
    AssistantDelta {
        conversation_id: ConversationId,
        run_id: RunId,
        delta: String,
        revision: u64,
    },
    ToolCallDelta {
        run_id: RunId,
        index: u32,
        id: Option<String>,
        name: Option<String>,
        arguments_delta: String,
        revision: u64,
    },
    ToolApprovalRequested {
        run_id: RunId,
        call: ToolCall,
        revision: u64,
    },
    ToolApprovalResolved {
        run_id: RunId,
        tool_call_id: String,
        approved: bool,
        revision: u64,
    },
    ToolExecutionStarted {
        run_id: RunId,
        tool_call_id: String,
        revision: u64,
    },
    ToolExecutionCompleted {
        run_id: RunId,
        tool_call_id: String,
        output: String,
        revision: u64,
    },
    ToolExecutionFailed {
        run_id: RunId,
        tool_call_id: String,
        message: String,
        revision: u64,
    },
    RunCompleted {
        conversation_id: ConversationId,
        run_id: RunId,
        finish_reason: Option<String>,
        usage: Option<Usage>,
        revision: u64,
    },
    RunFailed {
        conversation_id: ConversationId,
        run_id: RunId,
        kind: String,
        message: String,
        revision: u64,
    },
    RunCancelled {
        conversation_id: ConversationId,
        run_id: RunId,
        revision: u64,
    },
    Status {
        message: String,
        revision: u64,
    },
    CoreStopped {
        revision: u64,
    },
}

#[derive(Debug, Error)]
pub enum CoreStartError {
    #[error("failed to start core thread: {0}")]
    Thread(#[source] std::io::Error),
}

#[derive(Debug, Error)]
pub enum DispatchError {
    #[error("core command queue is full")]
    Full(AppCommand),
    #[error("core command queue is closed")]
    Closed(AppCommand),
}

#[derive(Clone)]
pub struct CoreHandle {
    commands: Sender<AppCommand>,
}

impl CoreHandle {
    pub fn try_dispatch(&self, command: AppCommand) -> Result<(), DispatchError> {
        self.commands
            .try_send(command)
            .map_err(|error| match error {
                TrySendError::Full(command) => DispatchError::Full(command),
                TrySendError::Closed(command) => DispatchError::Closed(command),
            })
    }

    pub fn dispatch(&self, command: AppCommand) -> Result<(), DispatchError> {
        self.commands
            .send_blocking(command)
            .map_err(|error| DispatchError::Closed(error.into_inner()))
    }

    pub fn shutdown(&self) -> Result<(), DispatchError> {
        self.dispatch(AppCommand::Shutdown)
    }
}

#[derive(Clone)]
pub struct CoreEvents {
    events: Receiver<AppEvent>,
}

impl CoreEvents {
    pub fn try_recv(&self) -> Result<AppEvent, async_channel::TryRecvError> {
        self.events.try_recv()
    }

    pub fn receiver(&self) -> Receiver<AppEvent> {
        self.events.clone()
    }

    pub fn recv_blocking(&self) -> Result<AppEvent, async_channel::RecvError> {
        self.events.recv_blocking()
    }
}

pub struct CoreRuntime {
    handle: CoreHandle,
    events: CoreEvents,
    join: Option<JoinHandle<()>>,
}

impl CoreRuntime {
    pub fn start() -> Result<Self, CoreStartError> {
        Self::start_with_provider(Arc::new(MissingProvider))
    }

    pub fn start_with_provider(provider: Arc<dyn ChatProvider>) -> Result<Self, CoreStartError> {
        Self::start_with_provider_and_tools(provider, Arc::new(tool::DisabledToolExecutor))
    }

    pub fn start_with_provider_and_tools(
        provider: Arc<dyn ChatProvider>,
        tool_executor: Arc<dyn ToolExecutor>,
    ) -> Result<Self, CoreStartError> {
        let (commands, command_receiver) = async_channel::bounded(COMMAND_CAPACITY);
        let (events, event_receiver) = async_channel::bounded(EVENT_CAPACITY);
        let emitter = Arc::new(EventEmitter::new(events));
        let join = thread::Builder::new()
            .name("cakify-core".to_owned())
            .spawn(move || run_loop(command_receiver, emitter, provider, tool_executor))
            .map_err(CoreStartError::Thread)?;

        Ok(Self {
            handle: CoreHandle { commands },
            events: CoreEvents {
                events: event_receiver,
            },
            join: Some(join),
        })
    }

    pub fn handle(&self) -> CoreHandle {
        self.handle.clone()
    }

    pub fn events(&self) -> CoreEvents {
        self.events.clone()
    }
}

impl Drop for CoreRuntime {
    fn drop(&mut self) {
        let _ = self.handle.shutdown();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

struct EventEmitter {
    events: Sender<AppEvent>,
    revision: Mutex<u64>,
}

impl EventEmitter {
    fn new(events: Sender<AppEvent>) -> Self {
        Self {
            events,
            revision: Mutex::new(0),
        }
    }

    fn emit(&self, mut event: AppEvent) -> bool {
        let Ok(mut revision) = self.revision.lock() else {
            return false;
        };
        *revision += 1;
        set_revision(&mut event, *revision);
        self.events.send_blocking(event).is_ok()
    }
}

struct ActiveRun {
    cancellation: Arc<AtomicBool>,
    approvals: SyncSender<ToolApproval>,
    worker: JoinHandle<()>,
}

struct ToolApproval {
    tool_call_id: String,
    approved: bool,
}

fn run_loop(
    commands: Receiver<AppCommand>,
    emitter: Arc<EventEmitter>,
    provider: Arc<dyn ChatProvider>,
    tool_executor: Arc<dyn ToolExecutor>,
) {
    let history = Arc::new(Mutex::new(
        HashMap::<ConversationId, Vec<ChatMessage>>::new(),
    ));
    let mut active_runs = HashMap::<RunId, ActiveRun>::new();
    let mut next_conversation = 1_u64;
    let mut next_run = 1_u64;

    emitter.emit(AppEvent::CoreReady { revision: 0 });

    while let Ok(command) = commands.recv_blocking() {
        prune_workers(&mut active_runs);
        match command {
            AppCommand::Bootstrap => {
                emitter.emit(AppEvent::Status {
                    message: "聊天核心已就绪".to_owned(),
                    revision: 0,
                });
            }
            AppCommand::CreateConversation {
                request_id,
                title: _,
            } => {
                let conversation_id = ConversationId::new(next_conversation);
                next_conversation += 1;
                if let Ok(mut history) = history.lock() {
                    history.insert(conversation_id, Vec::new());
                }
                emitter.emit(AppEvent::ConversationCreated {
                    request_id,
                    conversation_id,
                    revision: 0,
                });
            }
            AppCommand::SubmitDraft {
                request_id,
                conversation_id,
                model,
                text,
                tools,
                temperature,
            } => {
                if text.trim().is_empty() {
                    emitter.emit(AppEvent::Status {
                        message: "消息不能为空".to_owned(),
                        revision: 0,
                    });
                    continue;
                }

                let run_id = RunId::new(next_run);
                next_run += 1;
                let (messages, base_history_len) = match history.lock() {
                    Ok(mut history) => {
                        let messages = history.entry(conversation_id).or_default();
                        let base_history_len = messages.len();
                        messages.push(ChatMessage::user(text));
                        (messages.clone(), base_history_len)
                    }
                    Err(_) => {
                        emitter.emit(AppEvent::RunFailed {
                            conversation_id,
                            run_id,
                            kind: "internal".to_owned(),
                            message: "对话历史暂时不可用".to_owned(),
                            revision: 0,
                        });
                        continue;
                    }
                };

                emitter.emit(AppEvent::DraftAccepted {
                    request_id,
                    conversation_id,
                    run_id,
                    revision: 0,
                });

                let cancellation = Arc::new(AtomicBool::new(false));
                let (approvals, approval_receiver) = mpsc::sync_channel(APPROVAL_QUEUE_CAPACITY);
                let worker = spawn_run(RunWorker {
                    provider: provider.clone(),
                    tool_executor: tool_executor.clone(),
                    request: ChatRequest {
                        model,
                        messages,
                        tools,
                        temperature,
                    },
                    conversation_id,
                    run_id,
                    cancellation: cancellation.clone(),
                    emitter: emitter.clone(),
                    history: history.clone(),
                    base_history_len,
                    approvals: approval_receiver,
                });
                active_runs.insert(
                    run_id,
                    ActiveRun {
                        cancellation,
                        approvals,
                        worker,
                    },
                );
            }
            AppCommand::CancelRun { run_id } => {
                if let Some(run) = active_runs.get(&run_id) {
                    run.cancellation.store(true, Ordering::Release);
                }
            }
            AppCommand::ResolveToolApproval {
                run_id,
                tool_call_id,
                approved,
            } => {
                let Some(run) = active_runs.get(&run_id) else {
                    emitter.emit(AppEvent::Status {
                        message: "工具调用已经结束".to_owned(),
                        revision: 0,
                    });
                    continue;
                };
                if run
                    .approvals
                    .try_send(ToolApproval {
                        tool_call_id,
                        approved,
                    })
                    .is_err()
                {
                    emitter.emit(AppEvent::Status {
                        message: "工具审批队列暂时不可用".to_owned(),
                        revision: 0,
                    });
                }
            }
            AppCommand::Shutdown => {
                for run in active_runs.values() {
                    run.cancellation.store(true, Ordering::Release);
                }
                for (_, run) in active_runs {
                    let _ = run.worker.join();
                }
                emitter.emit(AppEvent::CoreStopped { revision: 0 });
                break;
            }
        }
    }
}

fn prune_workers(active_runs: &mut HashMap<RunId, ActiveRun>) {
    let completed = active_runs
        .iter()
        .filter_map(|(run_id, active)| active.worker.is_finished().then_some(*run_id))
        .collect::<Vec<_>>();
    for run_id in completed {
        if let Some(active) = active_runs.remove(&run_id) {
            let _ = active.worker.join();
        }
    }
}

struct RunWorker {
    provider: Arc<dyn ChatProvider>,
    tool_executor: Arc<dyn ToolExecutor>,
    request: ChatRequest,
    conversation_id: ConversationId,
    run_id: RunId,
    cancellation: Arc<AtomicBool>,
    emitter: Arc<EventEmitter>,
    history: Arc<Mutex<HashMap<ConversationId, Vec<ChatMessage>>>>,
    base_history_len: usize,
    approvals: mpsc::Receiver<ToolApproval>,
}

fn spawn_run(input: RunWorker) -> JoinHandle<()> {
    thread::Builder::new()
        .name(format!("cakify-run-{}", input.run_id.value()))
        .spawn(move || execute_run(input))
        .expect("start provider worker")
}

struct ToolCallBuffer {
    ui_index: u32,
    id: String,
    name: String,
    arguments_json: String,
}

fn execute_run(input: RunWorker) {
    let mut request = input.request.clone();
    let allowed_tools = request
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect::<HashSet<_>>();
    let mut aggregate_usage = None;
    let mut next_ui_index = 0_u32;

    for round in 0..=MAX_TOOL_ROUNDS {
        let mut assistant_text = String::new();
        let mut pending_text = String::new();
        let mut last_flush = Instant::now();
        let mut tool_calls = BTreeMap::<u32, ToolCallBuffer>::new();
        let mut round_usage = None;
        let mut finish_reason = None;
        let mut protocol_issue = None;

        let result =
            input
                .provider
                .stream(request.clone(), input.cancellation.clone(), &mut |event| {
                    if input.cancellation.load(Ordering::Acquire) {
                        return false;
                    }
                    if protocol_issue.is_some() {
                        return false;
                    }
                    match event {
                        ProviderStreamEvent::TextDelta(delta) => {
                            if assistant_text.len().saturating_add(delta.len())
                                > MAX_ASSISTANT_BYTES
                            {
                                protocol_issue = Some("模型输出超过安全上限");
                                return false;
                            }
                            assistant_text.push_str(&delta);
                            pending_text.push_str(&delta);
                            if pending_text.len() >= DELTA_FLUSH_BYTES
                                || last_flush.elapsed() >= DELTA_FLUSH_INTERVAL
                            {
                                flush_text(&input, &mut pending_text);
                                last_flush = Instant::now();
                            }
                        }
                        ProviderStreamEvent::ToolCallDelta {
                            index,
                            id,
                            name,
                            arguments_delta,
                        } => {
                            flush_text(&input, &mut pending_text);
                            if !tool_calls.contains_key(&index)
                                && tool_calls.len() >= MAX_TOOL_CALLS_PER_ROUND
                            {
                                protocol_issue = Some("单轮工具调用数量超过安全上限");
                                return false;
                            }
                            let call = tool_calls.entry(index).or_insert_with(|| {
                                let ui_index = next_ui_index;
                                next_ui_index = next_ui_index.saturating_add(1);
                                ToolCallBuffer {
                                    ui_index,
                                    id: String::new(),
                                    name: String::new(),
                                    arguments_json: String::new(),
                                }
                            });
                            if let Some(id) = &id {
                                if id.len() > MAX_TOOL_CALL_ID_BYTES {
                                    protocol_issue = Some("工具调用 ID 超过安全上限");
                                    return false;
                                }
                                call.id = id.clone();
                            }
                            if let Some(name) = &name {
                                if name.len() > MAX_TOOL_NAME_BYTES {
                                    protocol_issue = Some("工具名称超过安全上限");
                                    return false;
                                }
                                call.name = name.clone();
                            }
                            if call
                                .arguments_json
                                .len()
                                .saturating_add(arguments_delta.len())
                                > MAX_TOOL_ARGUMENT_BYTES
                            {
                                protocol_issue = Some("工具参数超过安全上限");
                                return false;
                            }
                            call.arguments_json.push_str(&arguments_delta);
                            input.emitter.emit(AppEvent::ToolCallDelta {
                                run_id: input.run_id,
                                index: call.ui_index,
                                id,
                                name,
                                arguments_delta,
                                revision: 0,
                            });
                        }
                        ProviderStreamEvent::Usage(value) => round_usage = Some(value),
                        ProviderStreamEvent::Finished { reason } => finish_reason = reason,
                    }
                    true
                });
        flush_text(&input, &mut pending_text);
        merge_usage(&mut aggregate_usage, round_usage);

        if let Some(message) = protocol_issue {
            fail_run(
                &input,
                ProviderError::new(ProviderErrorKind::Protocol, message),
            );
            return;
        }

        if input.cancellation.load(Ordering::Acquire)
            || result
                .as_ref()
                .is_err_and(|error| error.kind() == ProviderErrorKind::Cancelled)
        {
            cancel_run(&input);
            return;
        }
        if let Err(error) = result {
            fail_run(&input, error);
            return;
        }

        if tool_calls.is_empty() {
            if !assistant_text.is_empty() {
                append_history(&input, vec![ChatMessage::assistant(assistant_text)]);
            }
            input.emitter.emit(AppEvent::RunCompleted {
                conversation_id: input.conversation_id,
                run_id: input.run_id,
                finish_reason,
                usage: aggregate_usage,
                revision: 0,
            });
            return;
        }
        if round == MAX_TOOL_ROUNDS {
            fail_run(
                &input,
                ProviderError::new(ProviderErrorKind::Protocol, "工具调用轮数超过安全上限"),
            );
            return;
        }

        let calls = match finalize_tool_calls(&input, tool_calls, &allowed_tools) {
            Ok(calls) => calls,
            Err(error) => {
                fail_run(&input, error);
                return;
            }
        };
        let assistant_message = ChatMessage::assistant_with_tool_calls(
            assistant_text,
            calls
                .iter()
                .map(|call| {
                    ChatToolCall::function(
                        call.id.clone(),
                        call.name.clone(),
                        call.arguments_json.clone(),
                    )
                })
                .collect(),
        );
        append_history(&input, vec![assistant_message]);
        for call in &calls {
            input.emitter.emit(AppEvent::ToolApprovalRequested {
                run_id: input.run_id,
                call: call.clone(),
                revision: 0,
            });
        }

        let decisions = match wait_for_approvals(&input, &calls) {
            Ok(decisions) => decisions,
            Err(error) if error.kind() == ProviderErrorKind::Cancelled => {
                cancel_run(&input);
                return;
            }
            Err(error) => {
                fail_run(&input, error);
                return;
            }
        };
        let mut tool_messages = Vec::with_capacity(calls.len());
        for call in calls {
            let approved = decisions.get(&call.id).copied().unwrap_or(false);
            let output = if approved {
                input.emitter.emit(AppEvent::ToolExecutionStarted {
                    run_id: input.run_id,
                    tool_call_id: call.id.clone(),
                    revision: 0,
                });
                match input.tool_executor.execute(
                    &call.name,
                    &call.arguments_json,
                    input.cancellation.clone(),
                ) {
                    Ok(output) if output.len() <= MAX_TOOL_OUTPUT_BYTES => {
                        input.emitter.emit(AppEvent::ToolExecutionCompleted {
                            run_id: input.run_id,
                            tool_call_id: call.id.clone(),
                            output: output.clone(),
                            revision: 0,
                        });
                        output
                    }
                    Ok(_) => {
                        let message = "工具输出超过安全上限".to_owned();
                        input.emitter.emit(AppEvent::ToolExecutionFailed {
                            run_id: input.run_id,
                            tool_call_id: call.id.clone(),
                            message: message.clone(),
                            revision: 0,
                        });
                        serde_json::json!({ "error": message }).to_string()
                    }
                    Err(error) => {
                        if input.cancellation.load(Ordering::Acquire) {
                            cancel_run(&input);
                            return;
                        }
                        let message = error.public_message().to_owned();
                        input.emitter.emit(AppEvent::ToolExecutionFailed {
                            run_id: input.run_id,
                            tool_call_id: call.id.clone(),
                            message: message.clone(),
                            revision: 0,
                        });
                        serde_json::json!({ "error": message }).to_string()
                    }
                }
            } else {
                serde_json::json!({ "error": "user_denied" }).to_string()
            };
            tool_messages.push(ChatMessage::tool(call.id, output));
        }
        append_history(&input, tool_messages);
        request.messages = match input.history.lock() {
            Ok(history) => history
                .get(&input.conversation_id)
                .cloned()
                .unwrap_or_default(),
            Err(_) => {
                fail_run(
                    &input,
                    ProviderError::new(ProviderErrorKind::Protocol, "对话历史暂时不可用"),
                );
                return;
            }
        };
    }
}

fn finalize_tool_calls(
    input: &RunWorker,
    tool_calls: BTreeMap<u32, ToolCallBuffer>,
    allowed_tools: &HashSet<String>,
) -> Result<Vec<ToolCall>, ProviderError> {
    let mut ids = HashSet::new();
    tool_calls
        .into_values()
        .map(|mut call| {
            if call.id.is_empty() {
                call.id = format!("tool-{}-{}", input.run_id.value(), call.ui_index);
            }
            if call.name.is_empty() || !allowed_tools.contains(&call.name) {
                return Err(ProviderError::new(
                    ProviderErrorKind::Protocol,
                    "模型返回了未授权的工具调用",
                ));
            }
            if !ids.insert(call.id.clone()) {
                return Err(ProviderError::new(
                    ProviderErrorKind::Protocol,
                    "模型返回了重复的工具调用 ID",
                ));
            }
            if call.arguments_json.is_empty() {
                call.arguments_json = "{}".to_owned();
            }
            if call.arguments_json.len() > MAX_TOOL_ARGUMENT_BYTES
                || !serde_json::from_str::<serde_json::Value>(&call.arguments_json)
                    .is_ok_and(|value| value.is_object())
            {
                return Err(ProviderError::new(
                    ProviderErrorKind::Protocol,
                    "模型返回了无效或过大的工具参数",
                ));
            }
            Ok(ToolCall {
                index: call.ui_index,
                id: call.id,
                name: call.name,
                arguments_json: call.arguments_json,
            })
        })
        .collect()
}

fn wait_for_approvals(
    input: &RunWorker,
    calls: &[ToolCall],
) -> Result<HashMap<String, bool>, ProviderError> {
    let pending = calls
        .iter()
        .map(|call| call.id.as_str())
        .collect::<HashSet<_>>();
    let mut decisions = HashMap::new();
    while decisions.len() < pending.len() {
        if input.cancellation.load(Ordering::Acquire) {
            return Err(ProviderError::new(
                ProviderErrorKind::Cancelled,
                "请求已取消",
            ));
        }
        match input.approvals.recv_timeout(APPROVAL_POLL_INTERVAL) {
            Ok(approval)
                if pending.contains(approval.tool_call_id.as_str())
                    && !decisions.contains_key(&approval.tool_call_id) =>
            {
                input.emitter.emit(AppEvent::ToolApprovalResolved {
                    run_id: input.run_id,
                    tool_call_id: approval.tool_call_id.clone(),
                    approved: approval.approved,
                    revision: 0,
                });
                decisions.insert(approval.tool_call_id, approval.approved);
            }
            Ok(_) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                return Err(ProviderError::new(
                    ProviderErrorKind::Protocol,
                    "工具审批通道已关闭",
                ));
            }
        }
    }
    Ok(decisions)
}

fn merge_usage(aggregate: &mut Option<Usage>, next: Option<Usage>) {
    let Some(next) = next else {
        return;
    };
    let current = aggregate.get_or_insert(Usage {
        input_tokens: None,
        output_tokens: None,
        total_tokens: None,
    });
    current.input_tokens = sum_optional(current.input_tokens, next.input_tokens);
    current.output_tokens = sum_optional(current.output_tokens, next.output_tokens);
    current.total_tokens = sum_optional(current.total_tokens, next.total_tokens);
}

fn sum_optional(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.saturating_add(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn append_history(input: &RunWorker, new_messages: Vec<ChatMessage>) {
    if let Ok(mut history) = input.history.lock() {
        history
            .entry(input.conversation_id)
            .or_default()
            .extend(new_messages);
    }
}

fn fail_run(input: &RunWorker, error: ProviderError) {
    rollback_turn(input);
    input.emitter.emit(AppEvent::RunFailed {
        conversation_id: input.conversation_id,
        run_id: input.run_id,
        kind: error.kind().as_str().to_owned(),
        message: error.public_message().to_owned(),
        revision: 0,
    });
}

fn cancel_run(input: &RunWorker) {
    rollback_turn(input);
    input.emitter.emit(AppEvent::RunCancelled {
        conversation_id: input.conversation_id,
        run_id: input.run_id,
        revision: 0,
    });
}

fn rollback_turn(input: &RunWorker) {
    if let Ok(mut history) = input.history.lock() {
        let Some(messages) = history.get_mut(&input.conversation_id) else {
            return;
        };
        if messages.len() >= input.base_history_len {
            messages.truncate(input.base_history_len);
        }
    }
}

fn flush_text(input: &RunWorker, pending_text: &mut String) {
    if pending_text.is_empty() {
        return;
    }
    let delta = std::mem::take(pending_text);
    input.emitter.emit(AppEvent::AssistantDelta {
        conversation_id: input.conversation_id,
        run_id: input.run_id,
        delta,
        revision: 0,
    });
}

fn set_revision(event: &mut AppEvent, revision: u64) {
    match event {
        AppEvent::CoreReady { revision: value }
        | AppEvent::ConversationCreated {
            revision: value, ..
        }
        | AppEvent::DraftAccepted {
            revision: value, ..
        }
        | AppEvent::AssistantDelta {
            revision: value, ..
        }
        | AppEvent::ToolCallDelta {
            revision: value, ..
        }
        | AppEvent::ToolApprovalRequested {
            revision: value, ..
        }
        | AppEvent::ToolApprovalResolved {
            revision: value, ..
        }
        | AppEvent::ToolExecutionStarted {
            revision: value, ..
        }
        | AppEvent::ToolExecutionCompleted {
            revision: value, ..
        }
        | AppEvent::ToolExecutionFailed {
            revision: value, ..
        }
        | AppEvent::RunCompleted {
            revision: value, ..
        }
        | AppEvent::RunFailed {
            revision: value, ..
        }
        | AppEvent::RunCancelled {
            revision: value, ..
        }
        | AppEvent::Status {
            revision: value, ..
        }
        | AppEvent::CoreStopped { revision: value } => *value = revision,
    }
}

pub fn start_core() -> Result<CoreRuntime, CoreStartError> {
    CoreRuntime::start()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use super::*;

    struct FixtureProvider;

    impl ChatProvider for FixtureProvider {
        fn stream(
            &self,
            request: ChatRequest,
            _cancellation: Arc<AtomicBool>,
            sink: &mut StreamSink<'_>,
        ) -> Result<(), ProviderError> {
            assert_eq!(request.messages.last().expect("message").content, "hello");
            assert!(sink(ProviderStreamEvent::TextDelta("hel".to_owned())));
            assert!(sink(ProviderStreamEvent::TextDelta("lo".to_owned())));
            assert!(sink(ProviderStreamEvent::Finished {
                reason: Some("stop".to_owned()),
            }));
            Ok(())
        }
    }

    #[test]
    fn core_streams_provider_events_with_monotonic_revisions() {
        let runtime =
            CoreRuntime::start_with_provider(Arc::new(FixtureProvider)).expect("core thread");
        let events = runtime.events();
        assert!(matches!(
            events.recv_blocking().expect("ready event"),
            AppEvent::CoreReady { revision: 1 }
        ));

        runtime
            .handle()
            .dispatch(AppCommand::CreateConversation {
                request_id: RequestId::new(7),
                title: "test".to_owned(),
            })
            .expect("create conversation");
        let conversation_id = match events.recv_blocking().expect("created event") {
            AppEvent::ConversationCreated {
                conversation_id,
                revision: 2,
                ..
            } => conversation_id,
            event => panic!("unexpected event: {event:?}"),
        };

        runtime
            .handle()
            .dispatch(AppCommand::SubmitDraft {
                request_id: RequestId::new(8),
                conversation_id,
                model: "fixture".to_owned(),
                text: "hello".to_owned(),
                tools: Vec::new(),
                temperature: None,
            })
            .expect("submit draft");

        assert!(matches!(
            events.recv_blocking().expect("accepted"),
            AppEvent::DraftAccepted { revision: 3, .. }
        ));
        assert!(matches!(
            events.recv_blocking().expect("delta"),
            AppEvent::AssistantDelta {
                delta,
                revision: 4,
                ..
            } if delta == "hello"
        ));
        assert!(matches!(
            events.recv_blocking().expect("completed"),
            AppEvent::RunCompleted {
                finish_reason: Some(reason),
                revision: 5,
                ..
            } if reason == "stop"
        ));
    }

    #[test]
    fn try_dispatch_reports_a_full_queue() {
        let (sender, receiver) = async_channel::bounded::<AppCommand>(1);
        sender
            .try_send(AppCommand::Bootstrap)
            .expect("first command fits");
        let error = sender
            .try_send(AppCommand::Bootstrap)
            .expect_err("second command must observe backpressure");
        assert!(matches!(error, TrySendError::Full(AppCommand::Bootstrap)));
        drop(receiver);
    }

    struct ToolLoopProvider {
        calls: AtomicUsize,
    }

    impl ChatProvider for ToolLoopProvider {
        fn stream(
            &self,
            request: ChatRequest,
            _cancellation: Arc<AtomicBool>,
            sink: &mut StreamSink<'_>,
        ) -> Result<(), ProviderError> {
            match self.calls.fetch_add(1, Ordering::AcqRel) {
                0 => {
                    assert_eq!(request.tools, builtin_tool_definitions());
                    assert_eq!(request.messages.len(), 1);
                    assert!(sink(ProviderStreamEvent::ToolCallDelta {
                        index: 0,
                        id: Some("call-clock".to_owned()),
                        name: Some(CURRENT_TIME_TOOL_NAME.to_owned()),
                        arguments_delta: "{}".to_owned(),
                    }));
                    assert!(sink(ProviderStreamEvent::Finished {
                        reason: Some("tool_calls".to_owned()),
                    }));
                }
                1 => {
                    assert_eq!(request.messages.len(), 3);
                    let assistant = &request.messages[1];
                    assert_eq!(assistant.role, ChatRole::Assistant);
                    assert_eq!(assistant.tool_calls.len(), 1);
                    assert_eq!(assistant.tool_calls[0].id, "call-clock");
                    let tool = &request.messages[2];
                    assert_eq!(tool.role, ChatRole::Tool);
                    assert_eq!(tool.tool_call_id.as_deref(), Some("call-clock"));
                    let output: serde_json::Value =
                        serde_json::from_str(&tool.content).expect("tool output JSON");
                    assert_eq!(output["timezone"], "UTC");
                    assert!(sink(ProviderStreamEvent::TextDelta(
                        "工具结果已回填".to_owned(),
                    )));
                    assert!(sink(ProviderStreamEvent::Finished {
                        reason: Some("stop".to_owned()),
                    }));
                }
                call => panic!("unexpected provider call {call}"),
            }
            Ok(())
        }
    }

    #[test]
    fn approved_tool_executes_and_continues_the_model_turn() {
        let provider = Arc::new(ToolLoopProvider {
            calls: AtomicUsize::new(0),
        });
        let runtime = CoreRuntime::start_with_provider_and_tools(
            provider.clone(),
            Arc::new(BuiltinToolExecutor),
        )
        .expect("core thread");
        let events = runtime.events();
        assert!(matches!(
            events.recv_blocking().expect("ready"),
            AppEvent::CoreReady { .. }
        ));
        runtime
            .handle()
            .dispatch(AppCommand::CreateConversation {
                request_id: RequestId::new(10),
                title: "tools".to_owned(),
            })
            .expect("create conversation");
        let conversation_id = match events.recv_blocking().expect("created") {
            AppEvent::ConversationCreated {
                conversation_id, ..
            } => conversation_id,
            event => panic!("unexpected event: {event:?}"),
        };
        runtime
            .handle()
            .dispatch(AppCommand::SubmitDraft {
                request_id: RequestId::new(11),
                conversation_id,
                model: "fixture".to_owned(),
                text: "现在几点".to_owned(),
                tools: builtin_tool_definitions(),
                temperature: None,
            })
            .expect("submit draft");

        let run_id = match events.recv_blocking().expect("accepted") {
            AppEvent::DraftAccepted { run_id, .. } => run_id,
            event => panic!("unexpected event: {event:?}"),
        };
        assert!(matches!(
            events.recv_blocking().expect("tool delta"),
            AppEvent::ToolCallDelta { .. }
        ));
        assert!(matches!(
            events.recv_blocking().expect("approval requested"),
            AppEvent::ToolApprovalRequested { ref call, .. } if call.id == "call-clock"
        ));
        runtime
            .handle()
            .dispatch(AppCommand::ResolveToolApproval {
                run_id,
                tool_call_id: "call-clock".to_owned(),
                approved: true,
            })
            .expect("approve tool");
        assert!(matches!(
            events.recv_blocking().expect("approval resolved"),
            AppEvent::ToolApprovalResolved { approved: true, .. }
        ));
        assert!(matches!(
            events.recv_blocking().expect("execution started"),
            AppEvent::ToolExecutionStarted { .. }
        ));
        assert!(matches!(
            events.recv_blocking().expect("execution completed"),
            AppEvent::ToolExecutionCompleted { ref output, .. }
                if output.contains("unix_milliseconds")
        ));
        assert!(matches!(
            events.recv_blocking().expect("continued delta"),
            AppEvent::AssistantDelta { ref delta, .. } if delta == "工具结果已回填"
        ));
        assert!(matches!(
            events.recv_blocking().expect("completed"),
            AppEvent::RunCompleted {
                finish_reason: Some(ref reason),
                ..
            } if reason == "stop"
        ));
        assert_eq!(provider.calls.load(Ordering::Acquire), 2);
    }

    struct TooManyToolsProvider;

    impl ChatProvider for TooManyToolsProvider {
        fn stream(
            &self,
            _request: ChatRequest,
            _cancellation: Arc<AtomicBool>,
            sink: &mut StreamSink<'_>,
        ) -> Result<(), ProviderError> {
            for index in 0..=MAX_TOOL_CALLS_PER_ROUND as u32 {
                if !sink(ProviderStreamEvent::ToolCallDelta {
                    index,
                    id: Some(format!("call-{index}")),
                    name: Some(CURRENT_TIME_TOOL_NAME.to_owned()),
                    arguments_delta: "{}".to_owned(),
                }) {
                    return Ok(());
                }
            }
            panic!("core accepted more tool calls than the per-round limit");
        }
    }

    #[test]
    fn excessive_tool_calls_fail_before_approval_or_execution() {
        let runtime =
            CoreRuntime::start_with_provider(Arc::new(TooManyToolsProvider)).expect("core thread");
        let events = runtime.events();
        assert!(matches!(
            events.recv_blocking().expect("ready"),
            AppEvent::CoreReady { .. }
        ));
        runtime
            .handle()
            .dispatch(AppCommand::CreateConversation {
                request_id: RequestId::new(20),
                title: "tool limit".to_owned(),
            })
            .expect("create conversation");
        let conversation_id = match events.recv_blocking().expect("created") {
            AppEvent::ConversationCreated {
                conversation_id, ..
            } => conversation_id,
            event => panic!("unexpected event: {event:?}"),
        };
        runtime
            .handle()
            .dispatch(AppCommand::SubmitDraft {
                request_id: RequestId::new(21),
                conversation_id,
                model: "fixture".to_owned(),
                text: "call tools".to_owned(),
                tools: builtin_tool_definitions(),
                temperature: None,
            })
            .expect("submit draft");
        assert!(matches!(
            events.recv_blocking().expect("accepted"),
            AppEvent::DraftAccepted { .. }
        ));
        for _ in 0..MAX_TOOL_CALLS_PER_ROUND {
            assert!(matches!(
                events.recv_blocking().expect("bounded tool delta"),
                AppEvent::ToolCallDelta { .. }
            ));
        }
        assert!(matches!(
            events.recv_blocking().expect("protocol failure"),
            AppEvent::RunFailed { kind, message, .. }
                if kind == "protocol" && message.contains("数量超过安全上限")
        ));
    }

    struct DeniedToolProvider {
        calls: AtomicUsize,
    }

    impl ChatProvider for DeniedToolProvider {
        fn stream(
            &self,
            request: ChatRequest,
            _cancellation: Arc<AtomicBool>,
            sink: &mut StreamSink<'_>,
        ) -> Result<(), ProviderError> {
            if self.calls.fetch_add(1, Ordering::AcqRel) == 0 {
                assert!(sink(ProviderStreamEvent::ToolCallDelta {
                    index: 0,
                    id: Some("call-denied".to_owned()),
                    name: Some(CURRENT_TIME_TOOL_NAME.to_owned()),
                    arguments_delta: "{}".to_owned(),
                }));
                return Ok(());
            }
            let tool_result = request.messages.last().expect("denied tool result");
            assert_eq!(tool_result.role, ChatRole::Tool);
            assert_eq!(tool_result.tool_call_id.as_deref(), Some("call-denied"));
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&tool_result.content)
                    .expect("denial JSON")["error"],
                "user_denied"
            );
            assert!(sink(ProviderStreamEvent::TextDelta(
                "已跳过工具".to_owned()
            )));
            assert!(sink(ProviderStreamEvent::Finished {
                reason: Some("stop".to_owned()),
            }));
            Ok(())
        }
    }

    #[test]
    fn denied_tool_is_returned_to_model_without_execution() {
        let provider = Arc::new(DeniedToolProvider {
            calls: AtomicUsize::new(0),
        });
        let runtime = CoreRuntime::start_with_provider_and_tools(
            provider.clone(),
            Arc::new(BuiltinToolExecutor),
        )
        .expect("core thread");
        let events = runtime.events();
        assert!(matches!(
            events.recv_blocking().expect("ready"),
            AppEvent::CoreReady { .. }
        ));
        runtime
            .handle()
            .dispatch(AppCommand::CreateConversation {
                request_id: RequestId::new(30),
                title: "deny tool".to_owned(),
            })
            .expect("create conversation");
        let conversation_id = match events.recv_blocking().expect("created") {
            AppEvent::ConversationCreated {
                conversation_id, ..
            } => conversation_id,
            event => panic!("unexpected event: {event:?}"),
        };
        runtime
            .handle()
            .dispatch(AppCommand::SubmitDraft {
                request_id: RequestId::new(31),
                conversation_id,
                model: "fixture".to_owned(),
                text: "do not call".to_owned(),
                tools: builtin_tool_definitions(),
                temperature: None,
            })
            .expect("submit draft");
        let run_id = match events.recv_blocking().expect("accepted") {
            AppEvent::DraftAccepted { run_id, .. } => run_id,
            event => panic!("unexpected event: {event:?}"),
        };
        assert!(matches!(
            events.recv_blocking().expect("tool delta"),
            AppEvent::ToolCallDelta { .. }
        ));
        assert!(matches!(
            events.recv_blocking().expect("approval requested"),
            AppEvent::ToolApprovalRequested { .. }
        ));
        runtime
            .handle()
            .dispatch(AppCommand::ResolveToolApproval {
                run_id,
                tool_call_id: "call-denied".to_owned(),
                approved: false,
            })
            .expect("deny tool");
        assert!(matches!(
            events.recv_blocking().expect("denial resolved"),
            AppEvent::ToolApprovalResolved {
                approved: false,
                ..
            }
        ));
        assert!(matches!(
            events.recv_blocking().expect("continued model output"),
            AppEvent::AssistantDelta { ref delta, .. } if delta == "已跳过工具"
        ));
        assert!(matches!(
            events.recv_blocking().expect("completed"),
            AppEvent::RunCompleted { .. }
        ));
        assert_eq!(provider.calls.load(Ordering::Acquire), 2);
    }
}
