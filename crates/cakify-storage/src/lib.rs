//! SQLite persistence boundary for Cakify.
//!
//! A single actor thread owns the writer connection. The UI and Core never
//! receive a `rusqlite::Connection`, which keeps migrations and blocking I/O
//! away from the GPUI thread.

mod actor;
mod migration;
mod model;
mod repository;

use thiserror::Error;

pub use actor::{StorageActor, StorageConfig, StorageHandle, StorageHealth};
pub use migration::LATEST_SCHEMA_VERSION;
pub use model::{
    ConversationCursor, ConversationPage, ConversationQuery, ConversationRecord,
    ConversationThread, CrashRecoveryReport, DeletedProviderProfile, MessagePartKind,
    MessagePartRecord, MessageRecord, MessageRole, NewConversation, NewMessage, NewMessagePart,
    NewProviderModel, NewProviderProfile, NewRun, ProviderModelRecord, ProviderProfileRecord,
    ProviderProfileStatusUpdate, ProviderProfileUpdate, RunRecord, RunStatus, RunUpdate,
    TextCheckpoint,
};

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("SQLite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("failed to start storage actor thread: {0}")]
    ThreadStart(#[source] std::io::Error),
    #[error("storage actor stopped before replying")]
    ActorClosed,
    #[error("storage actor thread panicked")]
    ActorPanicked,
    #[error("system clock is before the Unix epoch: {0}")]
    ClockBeforeEpoch(#[source] std::time::SystemTimeError),
    #[error("system clock value is outside the SQLite timestamp range")]
    ClockOutOfRange,
    #[error("SQLite busy timeout is outside the supported millisecond range")]
    BusyTimeoutOutOfRange,
    #[error("database quick_check failed: {details}")]
    IntegrityCheck { details: String },
    #[error("SQLite setting {setting} expected {expected}, observed {actual}")]
    Configuration {
        setting: &'static str,
        expected: String,
        actual: String,
    },
    #[error("database schema version {found} is newer than supported version {supported}")]
    UnsupportedSchema { found: i64, supported: i64 },
    #[error("migration sequence expected version {expected}, found {found}")]
    MigrationSequence { expected: i64, found: i64 },
    #[error("migration {version} name mismatch: expected {expected}, database contains {actual}")]
    MigrationName {
        version: i64,
        expected: &'static str,
        actual: String,
    },
    #[error(
        "migration {version} checksum mismatch: expected {expected}, database contains {actual}"
    )]
    MigrationChecksum {
        version: i64,
        expected: String,
        actual: String,
    },
    #[error(
        "PRAGMA user_version is {user_version}, but migration history ends at {history_version}"
    )]
    SchemaVersionMismatch {
        user_version: i64,
        history_version: i64,
    },
    #[error("page limit {limit} is outside 1..={max}")]
    InvalidPageLimit { limit: usize, max: usize },
    #[error("{field} is outside the supported numeric range")]
    ValueOutOfRange { field: &'static str },
    #[error("invalid {field}: {reason}")]
    InvalidInput { field: &'static str, reason: String },
    #[error("{entity} {id} was not found")]
    NotFound { entity: &'static str, id: String },
    #[error("invalid relation {relation}: {details}")]
    InvalidRelation {
        relation: &'static str,
        details: String,
    },
    #[error("stored {field} contains unsupported value {value}")]
    InvalidStoredValue { field: &'static str, value: String },
    #[error("stale write rejected for {entity} {id}")]
    StaleWrite { entity: &'static str, id: String },
    #[error("run transition from {from} to {to} is not allowed")]
    InvalidRunTransition {
        from: &'static str,
        to: &'static str,
    },
    #[error(
        "stale checkpoint for part {part_id}: current revision {current_revision}, attempted {attempted_revision}"
    )]
    StaleCheckpoint {
        part_id: String,
        current_revision: i64,
        attempted_revision: i64,
    },
    #[error("crash recovery selected {selected} runs but updated {updated}")]
    RecoveryCountMismatch { selected: usize, updated: usize },
    #[error("{field} contains forbidden credential-bearing key {key}")]
    SensitiveJsonKey { field: &'static str, key: String },
    #[error("JSON parsing failed: {0}")]
    Json(#[from] serde_json::Error),
}
