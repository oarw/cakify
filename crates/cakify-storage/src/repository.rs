use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension, Row, TransactionBehavior};
use serde_json::Value;

use crate::{
    ConversationCursor, ConversationPage, ConversationQuery, ConversationRecord,
    ConversationThread, CrashRecoveryReport, MessagePartKind, MessagePartRecord, MessageRecord,
    MessageRole, NewConversation, NewMessage, NewRun, RunRecord, RunStatus, RunUpdate,
    StorageError, TextCheckpoint,
};

pub(crate) fn create_conversation(
    connection: &mut Connection,
    input: NewConversation,
) -> Result<ConversationRecord, StorageError> {
    validate_provider_snapshot(
        "conversation.provider_snapshot_json",
        &input.provider_snapshot_json,
    )?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute(
        "INSERT INTO conversations(
            id, title, provider_id, model_id, provider_snapshot_json,
            system_instruction, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![
            input.id,
            input.title,
            input.provider_id,
            input.model_id,
            input.provider_snapshot_json,
            input.system_instruction,
            input.created_at,
        ],
    )?;
    let conversation =
        get_conversation(&transaction, &input.id)?.ok_or_else(|| StorageError::NotFound {
            entity: "conversation",
            id: input.id.clone(),
        })?;
    transaction.commit()?;
    Ok(conversation)
}

pub(crate) fn get_conversation(
    connection: &Connection,
    id: &str,
) -> Result<Option<ConversationRecord>, StorageError> {
    connection
        .query_row(
            "SELECT id, title, provider_id, model_id, provider_snapshot_json,
                    system_instruction, created_at, updated_at, archived_at, deleted_at
             FROM conversations WHERE id = ?1",
            params![id],
            map_conversation,
        )
        .optional()
        .map_err(Into::into)
}

pub(crate) fn list_conversations(
    connection: &Connection,
    query: ConversationQuery,
) -> Result<ConversationPage, StorageError> {
    if query.limit == 0 || query.limit > ConversationQuery::MAX_LIMIT {
        return Err(StorageError::InvalidPageLimit {
            limit: query.limit,
            max: ConversationQuery::MAX_LIMIT,
        });
    }

    let fetch_limit =
        i64::try_from(query.limit + 1).map_err(|_| StorageError::ValueOutOfRange {
            field: "conversation query limit",
        })?;
    let include_archived = if query.include_archived { 1_i64 } else { 0_i64 };
    let cursor_updated_at = query.cursor.as_ref().map(|cursor| cursor.updated_at);
    let cursor_id = query.cursor.as_ref().map(|cursor| cursor.id.as_str());
    let mut statement = connection.prepare(
        "SELECT id, title, provider_id, model_id, provider_snapshot_json,
                system_instruction, created_at, updated_at, archived_at, deleted_at
         FROM conversations
         WHERE deleted_at IS NULL
           AND (?1 = 1 OR archived_at IS NULL)
           AND (
               ?2 IS NULL
               OR updated_at < ?2
               OR (updated_at = ?2 AND id < ?3)
           )
         ORDER BY updated_at DESC, id DESC
         LIMIT ?4",
    )?;
    let mut items = statement
        .query_map(
            params![include_archived, cursor_updated_at, cursor_id, fetch_limit],
            map_conversation,
        )?
        .collect::<Result<Vec<_>, _>>()?;

    let has_more = items.len() > query.limit;
    if has_more {
        items.truncate(query.limit);
    }
    let next_cursor = if has_more {
        items.last().map(|conversation| ConversationCursor {
            updated_at: conversation.updated_at,
            id: conversation.id.clone(),
        })
    } else {
        None
    };

    Ok(ConversationPage { items, next_cursor })
}

pub(crate) fn mark_conversation_deleted(
    connection: &mut Connection,
    id: &str,
    deleted_at: i64,
) -> Result<ConversationRecord, StorageError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let changed = transaction.execute(
        "UPDATE conversations
         SET deleted_at = ?2, updated_at = max(updated_at, ?2)
         WHERE id = ?1",
        params![id, deleted_at],
    )?;
    if changed == 0 {
        return Err(StorageError::NotFound {
            entity: "conversation",
            id: id.to_owned(),
        });
    }
    let conversation =
        get_conversation(&transaction, id)?.ok_or_else(|| StorageError::NotFound {
            entity: "conversation",
            id: id.to_owned(),
        })?;
    transaction.commit()?;
    Ok(conversation)
}

pub(crate) fn purge_conversation(
    connection: &mut Connection,
    id: &str,
) -> Result<bool, StorageError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let changed = transaction.execute("DELETE FROM conversations WHERE id = ?1", params![id])?;
    transaction.commit()?;
    Ok(changed == 1)
}

pub(crate) fn append_message(
    connection: &mut Connection,
    input: NewMessage,
) -> Result<(), StorageError> {
    if input.parts.is_empty() {
        return Err(StorageError::InvalidInput {
            field: "message.parts",
            reason: "at least one part is required".to_owned(),
        });
    }

    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    validate_message_relation(
        &transaction,
        input.parent_message_id.as_deref(),
        &input.conversation_id,
        "parent_message_id",
    )?;
    validate_message_relation(
        &transaction,
        input.edited_from_message_id.as_deref(),
        &input.conversation_id,
        "edited_from_message_id",
    )?;
    transaction.execute(
        "INSERT INTO messages(
            id, conversation_id, role, ordinal, parent_message_id,
            edited_from_message_id, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            input.id,
            input.conversation_id,
            input.role.as_str(),
            input.ordinal,
            input.parent_message_id,
            input.edited_from_message_id,
            input.created_at,
        ],
    )?;

    for (ordinal, part) in input.parts.into_iter().enumerate() {
        let ordinal = i64::try_from(ordinal).map_err(|_| StorageError::ValueOutOfRange {
            field: "message part ordinal",
        })?;
        transaction.execute(
            "INSERT INTO message_parts(
                id, message_id, ordinal, kind, text_content, content_json,
                attachment_id, created_at, revision
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)",
            params![
                part.id,
                input.id,
                ordinal,
                part.kind.as_str(),
                part.text_content,
                part.content_json,
                part.attachment_id,
                part.created_at,
            ],
        )?;
    }

    let changed = transaction.execute(
        "UPDATE conversations
         SET updated_at = max(updated_at, ?2)
         WHERE id = ?1",
        params![input.conversation_id, input.created_at],
    )?;
    if changed == 0 {
        return Err(StorageError::NotFound {
            entity: "conversation",
            id: input.conversation_id,
        });
    }
    transaction.commit()?;
    Ok(())
}

pub(crate) fn load_thread(
    connection: &Connection,
    conversation_id: &str,
) -> Result<Option<ConversationThread>, StorageError> {
    let Some(conversation) = get_conversation(connection, conversation_id)? else {
        return Ok(None);
    };

    let mut message_statement = connection.prepare(
        "SELECT id, conversation_id, role, ordinal, parent_message_id,
                edited_from_message_id, created_at, deleted_at
         FROM messages
         WHERE conversation_id = ?1 AND deleted_at IS NULL
         ORDER BY ordinal ASC, id ASC",
    )?;
    let raw_messages = message_statement
        .query_map(params![conversation_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, Option<i64>>(7)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut messages = Vec::with_capacity(raw_messages.len());
    let mut message_index = HashMap::with_capacity(raw_messages.len());
    for (
        id,
        stored_conversation_id,
        role,
        ordinal,
        parent_message_id,
        edited_from_message_id,
        created_at,
        deleted_at,
    ) in raw_messages
    {
        message_index.insert(id.clone(), messages.len());
        messages.push(MessageRecord {
            id,
            conversation_id: stored_conversation_id,
            role: MessageRole::from_storage(role)?,
            ordinal,
            parent_message_id,
            edited_from_message_id,
            created_at,
            deleted_at,
            parts: Vec::new(),
        });
    }

    let mut part_statement = connection.prepare(
        "SELECT part.id, part.message_id, part.ordinal, part.kind,
                part.text_content, part.content_json, part.attachment_id,
                part.created_at, part.revision
         FROM message_parts AS part
         INNER JOIN messages AS message ON message.id = part.message_id
         WHERE message.conversation_id = ?1 AND message.deleted_at IS NULL
         ORDER BY message.ordinal ASC, message.id ASC, part.ordinal ASC, part.id ASC",
    )?;
    let raw_parts = part_statement
        .query_map(params![conversation_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, i64>(8)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    for (
        id,
        message_id,
        ordinal,
        kind,
        text_content,
        content_json,
        attachment_id,
        created_at,
        revision,
    ) in raw_parts
    {
        let Some(index) = message_index.get(&message_id).copied() else {
            return Err(StorageError::InvalidRelation {
                relation: "message_parts.message_id",
                details: format!("part {id} references unloaded message {message_id}"),
            });
        };
        messages[index].parts.push(MessagePartRecord {
            id,
            message_id,
            ordinal,
            kind: MessagePartKind::from_storage(kind)?,
            text_content,
            content_json,
            attachment_id,
            created_at,
            revision,
        });
    }

    Ok(Some(ConversationThread {
        conversation,
        messages,
    }))
}

pub(crate) fn create_run(
    connection: &mut Connection,
    input: NewRun,
) -> Result<RunRecord, StorageError> {
    if !input.status.is_active() {
        return Err(StorageError::InvalidInput {
            field: "run.status",
            reason: "a new run must start in an active state".to_owned(),
        });
    }
    validate_provider_snapshot("run.provider_snapshot_json", &input.provider_snapshot_json)?;

    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    validate_message_relation(
        &transaction,
        input.assistant_message_id.as_deref(),
        &input.conversation_id,
        "assistant_message_id",
    )?;
    transaction.execute(
        "INSERT INTO runs(
            id, conversation_id, assistant_message_id, status,
            provider_snapshot_json, model_id, started_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![
            input.id,
            input.conversation_id,
            input.assistant_message_id,
            input.status.as_str(),
            input.provider_snapshot_json,
            input.model_id,
            input.started_at,
        ],
    )?;
    transaction.execute(
        "UPDATE conversations
         SET updated_at = max(updated_at, ?2)
         WHERE id = ?1",
        params![input.conversation_id, input.started_at],
    )?;
    let run = get_run(&transaction, &input.id)?.ok_or_else(|| StorageError::NotFound {
        entity: "run",
        id: input.id.clone(),
    })?;
    transaction.commit()?;
    Ok(run)
}

pub(crate) fn get_run(
    connection: &Connection,
    id: &str,
) -> Result<Option<RunRecord>, StorageError> {
    let raw = connection
        .query_row(
            "SELECT id, conversation_id, assistant_message_id, status,
                    provider_snapshot_json, model_id, started_at, updated_at,
                    finished_at, finish_reason, error_kind, error_message,
                    usage_json, cancel_source
             FROM runs WHERE id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<String>>(10)?,
                    row.get::<_, Option<String>>(11)?,
                    row.get::<_, Option<String>>(12)?,
                    row.get::<_, Option<String>>(13)?,
                ))
            },
        )
        .optional()?;

    let Some((
        id,
        conversation_id,
        assistant_message_id,
        status,
        provider_snapshot_json,
        model_id,
        started_at,
        updated_at,
        finished_at,
        finish_reason,
        error_kind,
        error_message,
        usage_json,
        cancel_source,
    )) = raw
    else {
        return Ok(None);
    };

    Ok(Some(RunRecord {
        id,
        conversation_id,
        assistant_message_id,
        status: RunStatus::from_storage(status)?,
        provider_snapshot_json,
        model_id,
        started_at,
        updated_at,
        finished_at,
        finish_reason,
        error_kind,
        error_message,
        usage_json,
        cancel_source,
    }))
}

pub(crate) fn update_run(
    connection: &mut Connection,
    input: RunUpdate,
) -> Result<RunRecord, StorageError> {
    validate_run_update(&input)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let existing = get_run(&transaction, &input.id)?.ok_or_else(|| StorageError::NotFound {
        entity: "run",
        id: input.id.clone(),
    })?;
    if input.updated_at < existing.updated_at {
        return Err(StorageError::StaleWrite {
            entity: "run",
            id: input.id,
        });
    }
    if existing.status.is_terminal() && input.status != existing.status {
        return Err(StorageError::InvalidRunTransition {
            from: existing.status.as_str(),
            to: input.status.as_str(),
        });
    }

    transaction.execute(
        "UPDATE runs
         SET status = ?2, updated_at = ?3, finished_at = ?4,
             finish_reason = ?5, error_kind = ?6, error_message = ?7,
             usage_json = ?8, cancel_source = ?9
         WHERE id = ?1",
        params![
            input.id,
            input.status.as_str(),
            input.updated_at,
            input.finished_at,
            input.finish_reason,
            input.error_kind,
            input.error_message,
            input.usage_json,
            input.cancel_source,
        ],
    )?;
    transaction.execute(
        "UPDATE conversations
         SET updated_at = max(updated_at, ?2)
         WHERE id = ?1",
        params![existing.conversation_id, input.updated_at],
    )?;
    let run = get_run(&transaction, &input.id)?.ok_or_else(|| StorageError::NotFound {
        entity: "run",
        id: input.id.clone(),
    })?;
    transaction.commit()?;
    Ok(run)
}

pub(crate) fn checkpoint_text(
    connection: &mut Connection,
    input: TextCheckpoint,
) -> Result<(), StorageError> {
    if input.revision <= 0 {
        return Err(StorageError::InvalidInput {
            field: "message_part.revision",
            reason: "checkpoint revisions must be positive".to_owned(),
        });
    }

    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let stored = transaction
        .query_row(
            "SELECT part.revision, part.kind, part.text_content, message.conversation_id
             FROM message_parts AS part
             INNER JOIN messages AS message ON message.id = part.message_id
             WHERE part.id = ?1 AND part.message_id = ?2",
            params![input.part_id, input.message_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((current_revision, kind, current_text, stored_conversation_id)) = stored else {
        return Err(StorageError::NotFound {
            entity: "message part",
            id: input.part_id,
        });
    };
    if stored_conversation_id != input.conversation_id {
        return Err(StorageError::InvalidRelation {
            relation: "message_parts.message_id -> messages.conversation_id",
            details: format!(
                "part {} belongs to conversation {}, not {}",
                input.part_id, stored_conversation_id, input.conversation_id
            ),
        });
    }
    let kind = MessagePartKind::from_storage(kind)?;
    if !kind.supports_text_checkpoint() {
        return Err(StorageError::InvalidInput {
            field: "message_part.kind",
            reason: format!("{} parts cannot receive text checkpoints", kind.as_str()),
        });
    }
    if input.revision < current_revision
        || (input.revision == current_revision
            && current_text.as_deref() != Some(input.text_content.as_str()))
    {
        return Err(StorageError::StaleCheckpoint {
            part_id: input.part_id,
            current_revision,
            attempted_revision: input.revision,
        });
    }
    if input.revision == current_revision {
        return Ok(());
    }

    let changed = transaction.execute(
        "UPDATE message_parts
         SET text_content = ?2, revision = ?3
         WHERE id = ?1 AND revision = ?4",
        params![
            input.part_id,
            input.text_content,
            input.revision,
            current_revision,
        ],
    )?;
    if changed != 1 {
        return Err(StorageError::StaleCheckpoint {
            part_id: input.part_id,
            current_revision,
            attempted_revision: input.revision,
        });
    }
    transaction.execute(
        "UPDATE conversations
         SET updated_at = max(updated_at, ?2)
         WHERE id = ?1",
        params![input.conversation_id, input.updated_at],
    )?;
    transaction.commit()?;
    Ok(())
}

pub(crate) fn recover_active_runs(
    connection: &mut Connection,
    recovered_at: i64,
) -> Result<CrashRecoveryReport, StorageError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let interrupted_run_ids = {
        let mut statement = transaction.prepare(
            "SELECT id FROM runs
             WHERE status IN (
                'preparing', 'requesting', 'streaming',
                'awaiting_approval', 'tool_running'
             )
             ORDER BY id ASC",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let changed = transaction.execute(
        "UPDATE runs
         SET status = 'interrupted',
             updated_at = max(updated_at, ?1),
             finished_at = max(started_at, ?1),
             finish_reason = coalesce(finish_reason, 'app_restart'),
             error_kind = coalesce(error_kind, 'interrupted'),
             error_message = coalesce(
                error_message,
                'Cakify stopped before this run completed'
             )
         WHERE status IN (
            'preparing', 'requesting', 'streaming',
            'awaiting_approval', 'tool_running'
         )",
        params![recovered_at],
    )?;
    if changed != interrupted_run_ids.len() {
        return Err(StorageError::RecoveryCountMismatch {
            selected: interrupted_run_ids.len(),
            updated: changed,
        });
    }
    transaction.commit()?;

    Ok(CrashRecoveryReport {
        recovered_at,
        interrupted_run_ids,
    })
}

fn map_conversation(row: &Row<'_>) -> rusqlite::Result<ConversationRecord> {
    Ok(ConversationRecord {
        id: row.get(0)?,
        title: row.get(1)?,
        provider_id: row.get(2)?,
        model_id: row.get(3)?,
        provider_snapshot_json: row.get(4)?,
        system_instruction: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        archived_at: row.get(8)?,
        deleted_at: row.get(9)?,
    })
}

fn validate_message_relation(
    connection: &Connection,
    message_id: Option<&str>,
    conversation_id: &str,
    relation: &'static str,
) -> Result<(), StorageError> {
    let Some(message_id) = message_id else {
        return Ok(());
    };
    let stored_conversation_id = connection
        .query_row(
            "SELECT conversation_id FROM messages WHERE id = ?1",
            params![message_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    match stored_conversation_id {
        Some(stored) if stored == conversation_id => Ok(()),
        Some(stored) => Err(StorageError::InvalidRelation {
            relation,
            details: format!(
                "message {message_id} belongs to conversation {stored}, not {conversation_id}"
            ),
        }),
        None => Err(StorageError::NotFound {
            entity: "message",
            id: message_id.to_owned(),
        }),
    }
}

fn validate_run_update(input: &RunUpdate) -> Result<(), StorageError> {
    if input.status.is_active() && input.finished_at.is_some() {
        return Err(StorageError::InvalidInput {
            field: "run.finished_at",
            reason: "an active run cannot have a finish timestamp".to_owned(),
        });
    }
    if input.status.is_terminal() && input.finished_at.is_none() {
        return Err(StorageError::InvalidInput {
            field: "run.finished_at",
            reason: "a terminal run requires a finish timestamp".to_owned(),
        });
    }
    Ok(())
}

fn validate_provider_snapshot(field: &'static str, json: &str) -> Result<(), StorageError> {
    let root = serde_json::from_str::<Value>(json)?;
    let mut pending = vec![&root];
    while let Some(value) = pending.pop() {
        match value {
            Value::Object(entries) => {
                for (key, child) in entries {
                    let normalized = key
                        .chars()
                        .map(|character| match character {
                            '-' | ' ' => '_',
                            value => value.to_ascii_lowercase(),
                        })
                        .collect::<String>();
                    if matches!(
                        normalized.as_str(),
                        "api_key"
                            | "apikey"
                            | "password"
                            | "secret"
                            | "client_secret"
                            | "token"
                            | "access_token"
                            | "refresh_token"
                            | "bearer_token"
                            | "authorization"
                            | "credential"
                            | "credential_ref"
                    ) {
                        return Err(StorageError::SensitiveJsonKey {
                            field,
                            key: key.clone(),
                        });
                    }
                    pending.push(child);
                }
            }
            Value::Array(items) => pending.extend(items),
            _ => {}
        }
    }
    Ok(())
}
