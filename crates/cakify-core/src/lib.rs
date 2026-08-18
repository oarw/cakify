//! Framework-neutral command/event boundary for the Cakify desktop client.
//!
//! M0 deliberately contains a deterministic core loop rather than a provider,
//! database, or secret implementation. The boundary is stable enough for the
//! GPUI shell and for headless tests, while the effects are added in later
//! milestones.

mod secret;

pub use secret::{
    delete_reference_then_secret, put_then_commit_reference, SecretError, SecretId, SecretInput,
    SecretLifecycleError, SecretStore, SecretValue,
};

use std::thread::{self, JoinHandle};

use async_channel::{Receiver, Sender, TrySendError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const COMMAND_CAPACITY: usize = 256;
pub const EVENT_CAPACITY: usize = 1_024;

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

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
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
pub enum AppCommand {
    Bootstrap,
    CreateConversation {
        request_id: RequestId,
        title: String,
    },
    SubmitDraft {
        request_id: RequestId,
        conversation_id: ConversationId,
        text: String,
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
        let (commands, command_receiver) = async_channel::bounded(COMMAND_CAPACITY);
        let (events, event_receiver) = async_channel::bounded(EVENT_CAPACITY);
        let join = thread::Builder::new()
            .name("cakify-core".to_owned())
            .spawn(move || run_loop(command_receiver, events))
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

fn run_loop(commands: Receiver<AppCommand>, events: Sender<AppEvent>) {
    let mut revision = 0_u64;
    let mut next_conversation = 1_u64;
    let mut next_run = 1_u64;

    send_event(&events, &mut revision, AppEvent::CoreReady { revision: 0 });

    while let Ok(command) = commands.recv_blocking() {
        match command {
            AppCommand::Bootstrap => {
                send_event(
                    &events,
                    &mut revision,
                    AppEvent::Status {
                        message: "core bootstrap complete".to_owned(),
                        revision: 0,
                    },
                );
            }
            AppCommand::CreateConversation {
                request_id,
                title: _,
            } => {
                let conversation_id = ConversationId::new(next_conversation);
                next_conversation += 1;
                send_event(
                    &events,
                    &mut revision,
                    AppEvent::ConversationCreated {
                        request_id,
                        conversation_id,
                        revision: 0,
                    },
                );
            }
            AppCommand::SubmitDraft {
                request_id,
                conversation_id,
                text,
            } => {
                let run_id = RunId::new(next_run);
                next_run += 1;
                let message = format!("draft accepted ({} chars)", text.chars().count());
                send_event(
                    &events,
                    &mut revision,
                    AppEvent::DraftAccepted {
                        request_id,
                        conversation_id,
                        run_id,
                        revision: 0,
                    },
                );
                send_event(
                    &events,
                    &mut revision,
                    AppEvent::Status {
                        message,
                        revision: 0,
                    },
                );
            }
            AppCommand::Shutdown => {
                send_event(
                    &events,
                    &mut revision,
                    AppEvent::CoreStopped { revision: 0 },
                );
                break;
            }
        }
    }
}

fn send_event(events: &Sender<AppEvent>, revision: &mut u64, mut event: AppEvent) {
    *revision += 1;
    set_revision(&mut event, *revision);
    let _ = events.send_blocking(event);
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

    #[test]
    fn core_emits_ready_and_accepts_a_command() {
        let runtime = start_core().expect("core thread");
        let events = runtime.events();
        assert!(matches!(
            events.recv_blocking().expect("ready event"),
            AppEvent::CoreReady { revision: 1 }
        ));

        runtime
            .handle()
            .dispatch(AppCommand::CreateConversation {
                request_id: RequestId::new(7),
                title: "M0".to_owned(),
            })
            .expect("dispatch command");

        assert!(matches!(
            events.recv_blocking().expect("created event"),
            AppEvent::ConversationCreated {
                request_id: RequestId(7),
                conversation_id: ConversationId(1),
                revision: 2,
            }
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
