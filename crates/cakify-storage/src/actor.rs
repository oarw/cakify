use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, SyncSender},
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::Connection;

use crate::{
    migration, repository, ConversationPage, ConversationQuery, ConversationRecord,
    ConversationThread, CrashRecoveryReport, NewConversation, NewMessage, NewRun, RunRecord,
    RunUpdate, StorageError, TextCheckpoint, LATEST_SCHEMA_VERSION,
};

const COMMAND_CAPACITY: usize = 64;
const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_millis(2_500);

#[derive(Clone, Debug)]
pub struct StorageConfig {
    database_path: PathBuf,
    busy_timeout: Duration,
}

impl StorageConfig {
    pub fn new(database_path: impl Into<PathBuf>) -> Self {
        Self {
            database_path: database_path.into(),
            busy_timeout: DEFAULT_BUSY_TIMEOUT,
        }
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn busy_timeout(&self) -> Duration {
        self.busy_timeout
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageHealth {
    pub schema_version: i64,
    pub foreign_keys: bool,
    pub journal_mode: String,
    pub synchronous: i64,
    pub busy_timeout_ms: i64,
    pub temp_store: i64,
    pub quick_check: String,
}

#[derive(Clone)]
pub struct StorageHandle {
    commands: SyncSender<Command>,
}

impl StorageHandle {
    pub fn health(&self) -> Result<StorageHealth, StorageError> {
        self.request(|reply| Command::Health { reply })
    }

    pub fn create_conversation(
        &self,
        input: NewConversation,
    ) -> Result<ConversationRecord, StorageError> {
        self.request(|reply| Command::CreateConversation { input, reply })
    }

    pub fn get_conversation(&self, id: &str) -> Result<Option<ConversationRecord>, StorageError> {
        self.request(|reply| Command::GetConversation {
            id: id.to_owned(),
            reply,
        })
    }

    pub fn list_conversations(
        &self,
        query: ConversationQuery,
    ) -> Result<ConversationPage, StorageError> {
        self.request(|reply| Command::ListConversations { query, reply })
    }

    pub fn mark_conversation_deleted(
        &self,
        id: &str,
        deleted_at: i64,
    ) -> Result<ConversationRecord, StorageError> {
        self.request(|reply| Command::MarkConversationDeleted {
            id: id.to_owned(),
            deleted_at,
            reply,
        })
    }

    pub fn purge_conversation(&self, id: &str) -> Result<bool, StorageError> {
        self.request(|reply| Command::PurgeConversation {
            id: id.to_owned(),
            reply,
        })
    }

    pub fn append_message(&self, input: NewMessage) -> Result<(), StorageError> {
        self.request(|reply| Command::AppendMessage { input, reply })
    }

    pub fn load_thread(
        &self,
        conversation_id: &str,
    ) -> Result<Option<ConversationThread>, StorageError> {
        self.request(|reply| Command::LoadThread {
            conversation_id: conversation_id.to_owned(),
            reply,
        })
    }

    pub fn create_run(&self, input: NewRun) -> Result<RunRecord, StorageError> {
        self.request(|reply| Command::CreateRun { input, reply })
    }

    pub fn get_run(&self, id: &str) -> Result<Option<RunRecord>, StorageError> {
        self.request(|reply| Command::GetRun {
            id: id.to_owned(),
            reply,
        })
    }

    pub fn update_run(&self, input: RunUpdate) -> Result<RunRecord, StorageError> {
        self.request(|reply| Command::UpdateRun { input, reply })
    }

    pub fn checkpoint_text(&self, input: TextCheckpoint) -> Result<(), StorageError> {
        self.request(|reply| Command::CheckpointText { input, reply })
    }

    fn request<T: Send + 'static>(
        &self,
        command: impl FnOnce(SyncSender<Result<T, StorageError>>) -> Command,
    ) -> Result<T, StorageError> {
        let (reply, receiver) = mpsc::sync_channel(1);
        self.commands
            .send(command(reply))
            .map_err(|_| StorageError::ActorClosed)?;
        receiver.recv().map_err(|_| StorageError::ActorClosed)?
    }
}

pub struct StorageActor {
    handle: StorageHandle,
    startup_recovery: CrashRecoveryReport,
    join: Option<JoinHandle<()>>,
}

impl StorageActor {
    pub fn open(config: StorageConfig) -> Result<Self, StorageError> {
        let (commands, receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
        let (ready, ready_receiver) = mpsc::sync_channel(1);
        let join = thread::Builder::new()
            .name("cakify-storage".to_owned())
            .spawn(move || start_actor(config, receiver, ready))
            .map_err(StorageError::ThreadStart)?;

        let startup_recovery = match ready_receiver.recv() {
            Ok(startup) => startup,
            Err(_) => {
                return match join.join() {
                    Ok(()) => Err(StorageError::ActorClosed),
                    Err(_) => Err(StorageError::ActorPanicked),
                };
            }
        };

        let startup_recovery = match startup_recovery {
            Ok(report) => report,
            Err(error) => {
                return match join.join() {
                    Ok(()) => Err(error),
                    Err(_) => Err(StorageError::ActorPanicked),
                };
            }
        };

        Ok(Self {
            handle: StorageHandle { commands },
            startup_recovery,
            join: Some(join),
        })
    }

    pub fn handle(&self) -> StorageHandle {
        self.handle.clone()
    }

    pub fn health(&self) -> Result<StorageHealth, StorageError> {
        self.handle.health()
    }

    pub fn startup_recovery(&self) -> &CrashRecoveryReport {
        &self.startup_recovery
    }
}

impl Drop for StorageActor {
    fn drop(&mut self) {
        let Some(join) = self.join.take() else {
            return;
        };

        let (acknowledge, receiver) = mpsc::sync_channel(1);
        if self
            .handle
            .commands
            .send(Command::Shutdown { acknowledge })
            .is_ok()
        {
            let _ = receiver.recv();
        }
        let _ = join.join();
    }
}

enum Command {
    Health {
        reply: SyncSender<Result<StorageHealth, StorageError>>,
    },
    CreateConversation {
        input: NewConversation,
        reply: SyncSender<Result<ConversationRecord, StorageError>>,
    },
    GetConversation {
        id: String,
        reply: SyncSender<Result<Option<ConversationRecord>, StorageError>>,
    },
    ListConversations {
        query: ConversationQuery,
        reply: SyncSender<Result<ConversationPage, StorageError>>,
    },
    MarkConversationDeleted {
        id: String,
        deleted_at: i64,
        reply: SyncSender<Result<ConversationRecord, StorageError>>,
    },
    PurgeConversation {
        id: String,
        reply: SyncSender<Result<bool, StorageError>>,
    },
    AppendMessage {
        input: NewMessage,
        reply: SyncSender<Result<(), StorageError>>,
    },
    LoadThread {
        conversation_id: String,
        reply: SyncSender<Result<Option<ConversationThread>, StorageError>>,
    },
    CreateRun {
        input: NewRun,
        reply: SyncSender<Result<RunRecord, StorageError>>,
    },
    GetRun {
        id: String,
        reply: SyncSender<Result<Option<RunRecord>, StorageError>>,
    },
    UpdateRun {
        input: RunUpdate,
        reply: SyncSender<Result<RunRecord, StorageError>>,
    },
    CheckpointText {
        input: TextCheckpoint,
        reply: SyncSender<Result<(), StorageError>>,
    },
    Shutdown {
        acknowledge: SyncSender<()>,
    },
}

fn start_actor(
    config: StorageConfig,
    receiver: Receiver<Command>,
    ready: SyncSender<Result<CrashRecoveryReport, StorageError>>,
) {
    let startup = initialize(config);
    let (connection, startup_recovery) = match startup {
        Ok(value) => value,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };

    if ready.send(Ok(startup_recovery)).is_err() {
        return;
    }

    run_loop(connection, receiver);
}

fn initialize(config: StorageConfig) -> Result<(Connection, CrashRecoveryReport), StorageError> {
    let mut connection = Connection::open(config.database_path)?;
    configure_connection(&connection, config.busy_timeout)?;
    quick_check(&connection)?;
    migration::apply_migrations(&mut connection, unix_timestamp_ms()?)?;
    let startup_recovery = repository::recover_active_runs(&mut connection, unix_timestamp_ms()?)?;
    let health = read_health(&connection)?;
    validate_health(&health)?;
    Ok((connection, startup_recovery))
}

fn run_loop(mut connection: Connection, receiver: Receiver<Command>) {
    while let Ok(command) = receiver.recv() {
        match command {
            Command::Health { reply } => {
                let _ = reply.send(read_health(&connection));
            }
            Command::CreateConversation { input, reply } => {
                let _ = reply.send(repository::create_conversation(&mut connection, input));
            }
            Command::GetConversation { id, reply } => {
                let _ = reply.send(repository::get_conversation(&connection, &id));
            }
            Command::ListConversations { query, reply } => {
                let _ = reply.send(repository::list_conversations(&connection, query));
            }
            Command::MarkConversationDeleted {
                id,
                deleted_at,
                reply,
            } => {
                let _ = reply.send(repository::mark_conversation_deleted(
                    &mut connection,
                    &id,
                    deleted_at,
                ));
            }
            Command::PurgeConversation { id, reply } => {
                let _ = reply.send(repository::purge_conversation(&mut connection, &id));
            }
            Command::AppendMessage { input, reply } => {
                let _ = reply.send(repository::append_message(&mut connection, input));
            }
            Command::LoadThread {
                conversation_id,
                reply,
            } => {
                let _ = reply.send(repository::load_thread(&connection, &conversation_id));
            }
            Command::CreateRun { input, reply } => {
                let _ = reply.send(repository::create_run(&mut connection, input));
            }
            Command::GetRun { id, reply } => {
                let _ = reply.send(repository::get_run(&connection, &id));
            }
            Command::UpdateRun { input, reply } => {
                let _ = reply.send(repository::update_run(&mut connection, input));
            }
            Command::CheckpointText { input, reply } => {
                let _ = reply.send(repository::checkpoint_text(&mut connection, input));
            }
            Command::Shutdown { acknowledge } => {
                let _ = acknowledge.send(());
                break;
            }
        }
    }
}

fn configure_connection(
    connection: &Connection,
    busy_timeout: Duration,
) -> Result<(), StorageError> {
    let busy_timeout_ms =
        i64::try_from(busy_timeout.as_millis()).map_err(|_| StorageError::BusyTimeoutOutOfRange)?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.pragma_update(None, "busy_timeout", busy_timeout_ms)?;
    connection.pragma_update(None, "temp_store", "MEMORY")?;

    let journal_mode: String =
        connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(StorageError::Configuration {
            setting: "journal_mode",
            expected: "wal".to_owned(),
            actual: journal_mode,
        });
    }
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(())
}

fn read_health(connection: &Connection) -> Result<StorageHealth, StorageError> {
    let foreign_keys: i64 =
        connection.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
    let journal_mode: String =
        connection.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
    let synchronous: i64 = connection.pragma_query_value(None, "synchronous", |row| row.get(0))?;
    let busy_timeout_ms: i64 =
        connection.pragma_query_value(None, "busy_timeout", |row| row.get(0))?;
    let temp_store: i64 = connection.pragma_query_value(None, "temp_store", |row| row.get(0))?;
    let quick_check = quick_check(connection)?;

    Ok(StorageHealth {
        schema_version: migration::current_schema_version(connection)?,
        foreign_keys: foreign_keys == 1,
        journal_mode: journal_mode.to_ascii_lowercase(),
        synchronous,
        busy_timeout_ms,
        temp_store,
        quick_check,
    })
}

fn validate_health(health: &StorageHealth) -> Result<(), StorageError> {
    expect_setting(
        "schema_version",
        LATEST_SCHEMA_VERSION.to_string(),
        health.schema_version.to_string(),
    )?;
    expect_setting(
        "foreign_keys",
        "1".to_owned(),
        if health.foreign_keys { "1" } else { "0" }.to_owned(),
    )?;
    expect_setting(
        "journal_mode",
        "wal".to_owned(),
        health.journal_mode.clone(),
    )?;
    expect_setting(
        "synchronous",
        "1".to_owned(),
        health.synchronous.to_string(),
    )?;
    expect_setting(
        "busy_timeout",
        "2500".to_owned(),
        health.busy_timeout_ms.to_string(),
    )?;
    expect_setting("temp_store", "2".to_owned(), health.temp_store.to_string())?;
    Ok(())
}

fn expect_setting(
    setting: &'static str,
    expected: String,
    actual: String,
) -> Result<(), StorageError> {
    if expected == actual {
        Ok(())
    } else {
        Err(StorageError::Configuration {
            setting,
            expected,
            actual,
        })
    }
}

fn quick_check(connection: &Connection) -> Result<String, StorageError> {
    let mut statement = connection.prepare("PRAGMA quick_check")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    let results = rows.collect::<Result<Vec<_>, _>>()?;
    if results.len() == 1 && results[0].eq_ignore_ascii_case("ok") {
        Ok("ok".to_owned())
    } else {
        Err(StorageError::IntegrityCheck {
            details: results.join("; "),
        })
    }
}

fn unix_timestamp_ms() -> Result<i64, StorageError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(StorageError::ClockBeforeEpoch)?;
    i64::try_from(duration.as_millis()).map_err(|_| StorageError::ClockOutOfRange)
}
