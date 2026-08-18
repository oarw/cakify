//! Framework-neutral command/event boundary for the Cakify desktop client.

mod chat;
mod secret;

pub use chat::{
    ChatMessage, ChatProvider, ChatRequest, ChatRole, MissingProvider, ProviderError,
    ProviderErrorKind, ProviderStreamEvent, StreamSink, ToolDefinition, Usage,
};
pub use secret::{
    delete_reference_then_secret, put_then_commit_reference, SecretError, SecretId, SecretInput,
    SecretLifecycleError, SecretStore, SecretValue,
};

use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        atomic::{AtomicBool, Ordering},
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
        let (commands, command_receiver) = async_channel::bounded(COMMAND_CAPACITY);
        let (events, event_receiver) = async_channel::bounded(EVENT_CAPACITY);
        let emitter = Arc::new(EventEmitter::new(events));
        let join = thread::Builder::new()
            .name("cakify-core".to_owned())
            .spawn(move || run_loop(command_receiver, emitter, provider))
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
    worker: JoinHandle<()>,
}

fn run_loop(
    commands: Receiver<AppCommand>,
    emitter: Arc<EventEmitter>,
    provider: Arc<dyn ChatProvider>,
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
                let (messages, submitted_user) = match history.lock() {
                    Ok(mut history) => {
                        let messages = history.entry(conversation_id).or_default();
                        let submitted_user = ChatMessage::user(text);
                        messages.push(submitted_user.clone());
                        (messages.clone(), submitted_user)
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
                let worker = spawn_run(RunWorker {
                    provider: provider.clone(),
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
                    submitted_user,
                });
                active_runs.insert(
                    run_id,
                    ActiveRun {
                        cancellation,
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
                emitter.emit(AppEvent::ToolApprovalResolved {
                    run_id,
                    tool_call_id,
                    approved,
                    revision: 0,
                });
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
    request: ChatRequest,
    conversation_id: ConversationId,
    run_id: RunId,
    cancellation: Arc<AtomicBool>,
    emitter: Arc<EventEmitter>,
    history: Arc<Mutex<HashMap<ConversationId, Vec<ChatMessage>>>>,
    submitted_user: ChatMessage,
}

fn spawn_run(input: RunWorker) -> JoinHandle<()> {
    thread::Builder::new()
        .name(format!("cakify-run-{}", input.run_id.value()))
        .spawn(move || execute_run(input))
        .expect("start provider worker")
}

#[derive(Default)]
struct ToolCallBuffer {
    id: String,
    name: String,
    arguments_json: String,
}

fn execute_run(input: RunWorker) {
    let mut assistant_text = String::new();
    let mut pending_text = String::new();
    let mut last_flush = Instant::now();
    let mut tool_calls = BTreeMap::<u32, ToolCallBuffer>::new();
    let mut usage = None;
    let mut finish_reason = None;

    let result = input
        .provider
        .stream(input.request.clone(), input.cancellation.clone(), &mut |event| {
            if input.cancellation.load(Ordering::Acquire) {
                return false;
            }
            match event {
                ProviderStreamEvent::TextDelta(delta) => {
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
                    let call = tool_calls.entry(index).or_default();
                    if let Some(id) = &id {
                        call.id = id.clone();
                    }
                    if let Some(name) = &name {
                        call.name = name.clone();
                    }
                    call.arguments_json.push_str(&arguments_delta);
                    input.emitter.emit(AppEvent::ToolCallDelta {
                        run_id: input.run_id,
                        index,
                        id,
                        name,
                        arguments_delta,
                        revision: 0,
                    });
                }
                ProviderStreamEvent::Usage(value) => usage = Some(value),
                ProviderStreamEvent::Finished { reason } => finish_reason = reason,
            }
            true
        });
    flush_text(&input, &mut pending_text);

    if input.cancellation.load(Ordering::Acquire)
        || result
            .as_ref()
            .is_err_and(|error| error.kind() == ProviderErrorKind::Cancelled)
    {
        remove_failed_turn(&input);
        input.emitter.emit(AppEvent::RunCancelled {
            conversation_id: input.conversation_id,
            run_id: input.run_id,
            revision: 0,
        });
        return;
    }

    if let Err(error) = result {
        remove_failed_turn(&input);
        input.emitter.emit(AppEvent::RunFailed {
            conversation_id: input.conversation_id,
            run_id: input.run_id,
            kind: error.kind().as_str().to_owned(),
            message: error.public_message().to_owned(),
            revision: 0,
        });
        return;
    }

    if !assistant_text.is_empty() {
        if let Ok(mut history) = input.history.lock() {
            history
                .entry(input.conversation_id)
                .or_default()
                .push(ChatMessage::assistant(assistant_text));
        }
    }

    for (index, call) in tool_calls {
        let id = if call.id.is_empty() {
            format!("tool-{}-{index}", input.run_id.value())
        } else {
            call.id
        };
        input.emitter.emit(AppEvent::ToolApprovalRequested {
            run_id: input.run_id,
            call: ToolCall {
                index,
                id,
                name: call.name,
                arguments_json: call.arguments_json,
            },
            revision: 0,
        });
    }

    input.emitter.emit(AppEvent::RunCompleted {
        conversation_id: input.conversation_id,
        run_id: input.run_id,
        finish_reason,
        usage,
        revision: 0,
    });
}

fn remove_failed_turn(input: &RunWorker) {
    if let Ok(mut history) = input.history.lock() {
        let Some(messages) = history.get_mut(&input.conversation_id) else {
            return;
        };
        if messages.last() == Some(&input.submitted_user) {
            messages.pop();
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
}
