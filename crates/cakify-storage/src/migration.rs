use rusqlite::{params, Connection, TransactionBehavior};
use sha2::{Digest, Sha256};

use crate::StorageError;

pub const LATEST_SCHEMA_VERSION: i64 = 2;

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_domain_schema",
        sql: include_str!("../migrations/0001_initial_domain_schema.sql"),
    },
    Migration {
        version: 2,
        name: "initial_indexes",
        sql: include_str!("../migrations/0002_initial_indexes.sql"),
    },
];

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

struct AppliedMigration {
    version: i64,
    name: String,
    checksum: String,
}

pub(crate) fn apply_migrations(
    connection: &mut Connection,
    applied_at_ms: i64,
) -> Result<(), StorageError> {
    apply_available_migrations(connection, applied_at_ms, MIGRATIONS)
}

fn apply_available_migrations(
    connection: &mut Connection,
    applied_at_ms: i64,
    available: &[Migration],
) -> Result<(), StorageError> {
    let user_version = pragma_user_version(connection)?;
    let supported = available.last().map_or(0, |migration| migration.version);
    if user_version > supported {
        return Err(StorageError::UnsupportedSchema {
            found: user_version,
            supported,
        });
    }

    let history_exists = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_schema
            WHERE type = 'table' AND name = 'schema_migrations'
        )",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    if !history_exists && user_version != 0 {
        return Err(StorageError::SchemaVersionMismatch {
            user_version,
            history_version: 0,
        });
    }
    bootstrap_history(connection)?;

    let applied = read_history(connection)?;
    validate_history(&applied, available)?;

    let history_version = applied.last().map_or(0, |migration| migration.version);
    if user_version != history_version {
        return Err(StorageError::SchemaVersionMismatch {
            user_version,
            history_version,
        });
    }

    for migration in &available[applied.len()..] {
        let checksum = checksum(migration.sql);
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations(version, name, checksum, applied_at) \
             VALUES (?1, ?2, ?3, ?4)",
            params![migration.version, migration.name, checksum, applied_at_ms],
        )?;
        transaction.pragma_update(None, "user_version", migration.version)?;
        transaction.commit()?;
    }

    let final_version = current_schema_version(connection)?;
    if final_version != supported {
        return Err(StorageError::SchemaVersionMismatch {
            user_version: pragma_user_version(connection)?,
            history_version: final_version,
        });
    }

    Ok(())
}

fn bootstrap_history(connection: &Connection) -> Result<(), StorageError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY NOT NULL CHECK(version > 0),
            name TEXT NOT NULL UNIQUE CHECK(length(trim(name)) > 0),
            checksum TEXT NOT NULL CHECK(length(checksum) = 64),
            applied_at INTEGER NOT NULL CHECK(applied_at >= 0)
        ) STRICT;",
    )?;
    Ok(())
}

fn read_history(connection: &Connection) -> Result<Vec<AppliedMigration>, StorageError> {
    let mut statement = connection
        .prepare("SELECT version, name, checksum FROM schema_migrations ORDER BY version ASC")?;
    let rows = statement.query_map([], |row| {
        Ok(AppliedMigration {
            version: row.get(0)?,
            name: row.get(1)?,
            checksum: row.get(2)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn validate_history(
    applied: &[AppliedMigration],
    available: &[Migration],
) -> Result<(), StorageError> {
    for (index, actual) in applied.iter().enumerate() {
        let Some(expected) = available.get(index) else {
            return Err(StorageError::UnsupportedSchema {
                found: actual.version,
                supported: available.last().map_or(0, |migration| migration.version),
            });
        };

        if actual.version != expected.version {
            return Err(StorageError::MigrationSequence {
                expected: expected.version,
                found: actual.version,
            });
        }
        if actual.name != expected.name {
            return Err(StorageError::MigrationName {
                version: actual.version,
                expected: expected.name,
                actual: actual.name.clone(),
            });
        }

        let expected_checksum = checksum(expected.sql);
        if actual.checksum != expected_checksum {
            return Err(StorageError::MigrationChecksum {
                version: actual.version,
                expected: expected_checksum,
                actual: actual.checksum.clone(),
            });
        }
    }
    Ok(())
}

pub(crate) fn current_schema_version(connection: &Connection) -> Result<i64, StorageError> {
    let version = connection
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get::<_, Option<i64>>(0)
        })?
        .unwrap_or(0);
    Ok(version)
}

fn pragma_user_version(connection: &Connection) -> Result<i64, StorageError> {
    connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(Into::into)
}

fn checksum(sql: &str) -> String {
    let digest = Sha256::digest(sql.as_bytes());
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_from_first_revision_to_latest() {
        let mut connection = Connection::open_in_memory().expect("in-memory database");
        apply_available_migrations(&mut connection, 1_700_000_000_000, &MIGRATIONS[..1])
            .expect("first migration");
        assert_eq!(current_schema_version(&connection).expect("version one"), 1);

        apply_migrations(&mut connection, 1_700_000_000_001).expect("remaining migrations");
        assert_eq!(
            current_schema_version(&connection).expect("latest version"),
            LATEST_SCHEMA_VERSION
        );
        assert_eq!(pragma_user_version(&connection).expect("user version"), 2);
    }

    #[test]
    fn migration_checksums_are_stable_sha256_values() {
        for migration in MIGRATIONS {
            let value = checksum(migration.sql);
            assert_eq!(value.len(), 64);
            assert!(value.bytes().all(|byte| byte.is_ascii_hexdigit()));
        }
    }
}
