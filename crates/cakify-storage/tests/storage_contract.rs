use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use cakify_storage::{
    StorageActor, StorageConfig, StorageError, StorageHealth, LATEST_SCHEMA_VERSION,
};
use rusqlite::Connection;

static NEXT_DATABASE: AtomicU64 = AtomicU64::new(1);

const REQUIRED_TABLES: &[&str] = &[
    "app_settings",
    "attachments",
    "conversation_mcp_servers",
    "conversations",
    "mcp_servers",
    "message_parts",
    "messages",
    "permission_rules",
    "provider_models",
    "provider_profiles",
    "runs",
    "schema_migrations",
    "tool_calls",
];

struct TestDatabase {
    path: PathBuf,
}

impl TestDatabase {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock")
            .as_nanos();
        let sequence = NEXT_DATABASE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "cakify-storage-{label}-{}-{nonce}-{sequence}.db",
            std::process::id()
        ));
        Self { path }
    }

    fn config(&self) -> StorageConfig {
        StorageConfig::new(&self.path)
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_file(sidecar_path(&self.path, "-wal"));
        let _ = fs::remove_file(sidecar_path(&self.path, "-shm"));
    }
}

fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn assert_expected_health(health: &StorageHealth) {
    assert_eq!(health.schema_version, LATEST_SCHEMA_VERSION);
    assert!(health.foreign_keys);
    assert_eq!(health.journal_mode, "wal");
    assert_eq!(health.synchronous, 1);
    assert_eq!(health.busy_timeout_ms, 2_500);
    assert_eq!(health.temp_store, 2);
    assert_eq!(health.quick_check, "ok");
}

#[test]
fn actor_initializes_required_schema_and_connection_pragmas() {
    let database = TestDatabase::new("initialize");
    let actor = StorageActor::open(database.config()).expect("open storage actor");
    assert_expected_health(&actor.health().expect("actor health"));
    assert_expected_health(&actor.handle().health().expect("cloned handle health"));
    drop(actor);

    let connection = Connection::open(&database.path).expect("inspect database");
    let mut statement = connection
        .prepare(
            "SELECT name FROM sqlite_schema \
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .expect("prepare table inventory");
    let actual_tables = statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query tables")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect tables");
    let expected_tables = REQUIRED_TABLES
        .iter()
        .map(|table| (*table).to_owned())
        .collect::<Vec<_>>();
    assert_eq!(actual_tables, expected_tables);

    let mut credential_reference_columns = Vec::new();
    for table in REQUIRED_TABLES {
        let pragma = format!("PRAGMA table_info('{table}')");
        let mut columns = connection.prepare(&pragma).expect("prepare table_info");
        let names = columns
            .query_map([], |row| row.get::<_, String>(1))
            .expect("query table_info")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect columns");
        for name in names {
            let normalized = name.to_ascii_lowercase();
            assert!(!normalized.contains("api_key"), "unexpected {table}.{name}");
            assert!(
                !normalized.contains("password"),
                "unexpected {table}.{name}"
            );
            assert!(!normalized.contains("token"), "unexpected {table}.{name}");
            assert!(!normalized.contains("secret"), "unexpected {table}.{name}");
            if normalized == "credential_ref" {
                credential_reference_columns.push(format!("{table}.{name}"));
            }
        }
    }
    assert_eq!(
        credential_reference_columns,
        vec!["provider_profiles.credential_ref".to_owned()]
    );
}

#[test]
fn reopening_database_is_idempotent() {
    let database = TestDatabase::new("reopen");
    drop(StorageActor::open(database.config()).expect("first open"));
    let actor = StorageActor::open(database.config()).expect("second open");
    assert_expected_health(&actor.health().expect("health after reopen"));
    drop(actor);

    let connection = Connection::open(&database.path).expect("inspect database");
    let migration_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .expect("migration count");
    assert_eq!(migration_count, LATEST_SCHEMA_VERSION);
}

#[test]
fn foreign_keys_reject_orphan_messages() {
    let database = TestDatabase::new("foreign-key");
    drop(StorageActor::open(database.config()).expect("initialize"));

    let connection = Connection::open(&database.path).expect("open database");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("enable foreign keys for inspection connection");
    let error = connection
        .execute(
            "INSERT INTO messages(
                id, conversation_id, role, ordinal, created_at
             ) VALUES ('message-1', 'missing-conversation', 'user', 0, 1)",
            [],
        )
        .expect_err("orphan message must fail");
    assert!(error.sqlite_error().is_some());
}

#[test]
fn changed_migration_checksum_is_rejected() {
    let database = TestDatabase::new("checksum");
    drop(StorageActor::open(database.config()).expect("initialize"));

    let connection = Connection::open(&database.path).expect("open database");
    connection
        .execute(
            "UPDATE schema_migrations SET checksum = ?1 WHERE version = 1",
            rusqlite::params!["0".repeat(64)],
        )
        .expect("tamper migration history");
    drop(connection);

    let error = match StorageActor::open(database.config()) {
        Ok(_) => panic!("tampered migration must not open"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        StorageError::MigrationChecksum { version: 1, .. }
    ));
}

#[test]
fn newer_schema_is_rejected() {
    let database = TestDatabase::new("newer-schema");
    drop(StorageActor::open(database.config()).expect("initialize"));

    let connection = Connection::open(&database.path).expect("open database");
    connection
        .pragma_update(None, "user_version", LATEST_SCHEMA_VERSION + 1)
        .expect("mark database newer");
    drop(connection);

    let error = match StorageActor::open(database.config()) {
        Ok(_) => panic!("newer schema must not open"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        StorageError::UnsupportedSchema {
            found,
            supported
        } if found == LATEST_SCHEMA_VERSION + 1 && supported == LATEST_SCHEMA_VERSION
    ));
}
