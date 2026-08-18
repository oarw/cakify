use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, SyncSender},
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::Connection;

use crate::{migration, StorageError, LATEST_SCHEMA_VERSION};

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
        let (reply, receiver) = mpsc::sync_channel(1);
        self.commands
            .send(Command::Health { reply })
            .map_err(|_| StorageError::ActorClosed)?;
        receiver.recv().map_err(|_| StorageError::ActorClosed)?
    }
}

pub struct StorageActor {
    handle: StorageHandle,
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

        let startup = match ready_receiver.recv() {
            Ok(startup) => startup,
            Err(_) => {
                return match join.join() {
                    Ok(()) => Err(StorageError::ActorClosed),
                    Err(_) => Err(StorageError::ActorPanicked),
                };
            }
        };

        if let Err(error) = startup {
            return match join.join() {
                Ok(()) => Err(error),
                Err(_) => Err(StorageError::ActorPanicked),
            };
        }

        Ok(Self {
            handle: StorageHandle { commands },
            join: Some(join),
        })
    }

    pub fn handle(&self) -> StorageHandle {
        self.handle.clone()
    }

    pub fn health(&self) -> Result<StorageHealth, StorageError> {
        self.handle.health()
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
    Shutdown {
        acknowledge: SyncSender<()>,
    },
}

fn start_actor(
    config: StorageConfig,
    receiver: Receiver<Command>,
    ready: SyncSender<Result<StorageHealth, StorageError>>,
) {
    let startup = initialize(config);
    let (connection, health) = match startup {
        Ok(value) => value,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };

    if ready.send(Ok(health)).is_err() {
        return;
    }

    run_loop(connection, receiver);
}

fn initialize(config: StorageConfig) -> Result<(Connection, StorageHealth), StorageError> {
    let mut connection = Connection::open(config.database_path)?;
    configure_connection(&connection, config.busy_timeout)?;
    quick_check(&connection)?;
    migration::apply_migrations(&mut connection, unix_timestamp_ms()?)?;
    let health = read_health(&connection)?;
    validate_health(&health)?;
    Ok((connection, health))
}

fn run_loop(connection: Connection, receiver: Receiver<Command>) {
    while let Ok(command) = receiver.recv() {
        match command {
            Command::Health { reply } => {
                let _ = reply.send(read_health(&connection));
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
