use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension, Row, TransactionBehavior};
use serde_json::Value;
use url::Url;

use crate::{
    ConversationCursor, ConversationPage, ConversationQuery, ConversationRecord,
    ConversationThread, CrashRecoveryReport, DeletedProviderProfile, McpServerRecord,
    McpServerStatusUpdate, McpTransport, MessagePartKind, MessagePartRecord, MessageRecord,
    MessageRole, NewConversation, NewMcpServer, NewMessage, NewProviderModel, NewProviderProfile,
    NewRun, ProviderModelRecord, ProviderProfileRecord, ProviderProfileStatusUpdate,
    ProviderProfileUpdate, RunRecord, RunStatus, RunUpdate, StorageError, TextCheckpoint,
};

pub(crate) fn create_provider_profile(
    connection: &mut Connection,
    input: NewProviderProfile,
) -> Result<ProviderProfileRecord, StorageError> {
    validate_new_provider_profile(&input)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute(
        "INSERT INTO provider_profiles(
            id, kind, endpoint, display_name, credential_ref, default_model,
            metadata_json, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            &input.id,
            &input.kind,
            &input.endpoint,
            &input.display_name,
            &input.credential_ref,
            &input.default_model,
            &input.metadata_json,
            input.created_at,
        ],
    )?;
    replace_provider_models_in_transaction(&transaction, &input.id, &input.models)?;
    let profile =
        get_provider_profile(&transaction, &input.id)?.ok_or_else(|| StorageError::NotFound {
            entity: "provider profile",
            id: input.id.clone(),
        })?;
    transaction.commit()?;
    Ok(profile)
}

pub(crate) fn get_provider_profile(
    connection: &Connection,
    id: &str,
) -> Result<Option<ProviderProfileRecord>, StorageError> {
    let profile = connection
        .query_row(
            "SELECT id, kind, endpoint, display_name, credential_ref, default_model,
                    metadata_json, created_at, updated_at, disabled_at
             FROM provider_profiles WHERE id = ?1",
            params![id],
            map_provider_profile,
        )
        .optional()?;
    let Some(mut profile) = profile else {
        return Ok(None);
    };
    profile.models = load_provider_models(connection, &profile.id)?;
    Ok(Some(profile))
}

pub(crate) fn list_provider_profiles(
    connection: &Connection,
    include_disabled: bool,
) -> Result<Vec<ProviderProfileRecord>, StorageError> {
    let include_disabled = if include_disabled { 1_i64 } else { 0_i64 };
    let mut statement = connection.prepare(
        "SELECT id, kind, endpoint, display_name, credential_ref, default_model,
                metadata_json, created_at, updated_at, disabled_at
         FROM provider_profiles
         WHERE ?1 = 1 OR disabled_at IS NULL
         ORDER BY disabled_at IS NOT NULL, display_name COLLATE NOCASE, id",
    )?;
    let mut profiles = statement
        .query_map(params![include_disabled], map_provider_profile)?
        .collect::<Result<Vec<_>, _>>()?;
    for profile in &mut profiles {
        profile.models = load_provider_models(connection, &profile.id)?;
    }
    Ok(profiles)
}

pub(crate) fn update_provider_profile(
    connection: &mut Connection,
    input: ProviderProfileUpdate,
) -> Result<ProviderProfileRecord, StorageError> {
    validate_provider_profile_update(&input)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let changed = transaction.execute(
        "UPDATE provider_profiles
         SET kind = ?2,
             endpoint = ?3,
             display_name = ?4,
             credential_ref = ?5,
             default_model = ?6,
             metadata_json = ?7,
             updated_at = ?8
         WHERE id = ?1 AND updated_at = ?9",
        params![
            &input.id,
            &input.kind,
            &input.endpoint,
            &input.display_name,
            &input.credential_ref,
            &input.default_model,
            &input.metadata_json,
            input.updated_at,
            input.expected_updated_at,
        ],
    )?;
    if changed == 0 {
        return provider_write_failure(&transaction, &input.id);
    }
    if let Some(models) = &input.models {
        replace_provider_models_in_transaction(&transaction, &input.id, models)?;
    }
    let profile =
        get_provider_profile(&transaction, &input.id)?.ok_or_else(|| StorageError::NotFound {
            entity: "provider profile",
            id: input.id.clone(),
        })?;
    transaction.commit()?;
    Ok(profile)
}

pub(crate) fn set_provider_profile_disabled(
    connection: &mut Connection,
    input: ProviderProfileStatusUpdate,
) -> Result<ProviderProfileRecord, StorageError> {
    validate_monotonic_profile_update(&input.id, input.expected_updated_at, input.updated_at)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let current =
        get_provider_profile(&transaction, &input.id)?.ok_or_else(|| StorageError::NotFound {
            entity: "provider profile",
            id: input.id.clone(),
        })?;
    if current.updated_at != input.expected_updated_at {
        return Err(StorageError::StaleWrite {
            entity: "provider profile",
            id: input.id,
        });
    }
    if let Some(disabled_at) = input.disabled_at {
        if disabled_at < current.created_at || disabled_at > input.updated_at {
            return Err(StorageError::InvalidInput {
                field: "provider.disabled_at",
                reason: "must be between created_at and updated_at".to_owned(),
            });
        }
    }
    transaction.execute(
        "UPDATE provider_profiles
         SET disabled_at = ?2, updated_at = ?3
         WHERE id = ?1 AND updated_at = ?4",
        params![
            &current.id,
            input.disabled_at,
            input.updated_at,
            input.expected_updated_at,
        ],
    )?;
    let profile =
        get_provider_profile(&transaction, &current.id)?.ok_or_else(|| StorageError::NotFound {
            entity: "provider profile",
            id: current.id.clone(),
        })?;
    transaction.commit()?;
    Ok(profile)
}

pub(crate) fn replace_provider_models(
    connection: &mut Connection,
    provider_id: &str,
    models: Vec<NewProviderModel>,
) -> Result<Vec<ProviderModelRecord>, StorageError> {
    validate_provider_models(&models)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let exists = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM provider_profiles WHERE id = ?1)",
        params![provider_id],
        |row| row.get::<_, bool>(0),
    )?;
    if !exists {
        return Err(StorageError::NotFound {
            entity: "provider profile",
            id: provider_id.to_owned(),
        });
    }
    replace_provider_models_in_transaction(&transaction, provider_id, &models)?;
    let stored = load_provider_models(&transaction, provider_id)?;
    transaction.commit()?;
    Ok(stored)
}

pub(crate) fn delete_provider_profile(
    connection: &mut Connection,
    id: &str,
) -> Result<DeletedProviderProfile, StorageError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let credential_ref = transaction
        .query_row(
            "SELECT credential_ref FROM provider_profiles WHERE id = ?1",
            params![id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .ok_or_else(|| StorageError::NotFound {
            entity: "provider profile",
            id: id.to_owned(),
        })?;
    transaction.execute("DELETE FROM provider_profiles WHERE id = ?1", params![id])?;
    transaction.commit()?;
    Ok(DeletedProviderProfile {
        id: id.to_owned(),
        credential_ref,
    })
}

pub(crate) fn create_mcp_server(
    connection: &mut Connection,
    input: NewMcpServer,
) -> Result<McpServerRecord, StorageError> {
    validate_new_mcp_server(&input)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute(
        "INSERT INTO mcp_servers(
            id, display_name, transport, config_json, enabled, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![
            &input.id,
            &input.display_name,
            input.transport.as_str(),
            &input.config_json,
            input.enabled,
            input.created_at,
        ],
    )?;
    let server =
        get_mcp_server(&transaction, &input.id)?.ok_or_else(|| StorageError::NotFound {
            entity: "MCP server",
            id: input.id.clone(),
        })?;
    transaction.commit()?;
    Ok(server)
}

pub(crate) fn get_mcp_server(
    connection: &Connection,
    id: &str,
) -> Result<Option<McpServerRecord>, StorageError> {
    connection
        .query_row(
            "SELECT id, display_name, transport, config_json, enabled,
                    capabilities_json, schema_hash, last_error, created_at, updated_at
             FROM mcp_servers WHERE id = ?1",
            params![id],
            map_stored_mcp_server,
        )
        .optional()?
        .map(stored_mcp_server_into_record)
        .transpose()
}

pub(crate) fn list_mcp_servers(
    connection: &Connection,
) -> Result<Vec<McpServerRecord>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT id, display_name, transport, config_json, enabled,
                capabilities_json, schema_hash, last_error, created_at, updated_at
         FROM mcp_servers
         ORDER BY display_name COLLATE NOCASE, id",
    )?;
    let stored_servers = statement
        .query_map([], map_stored_mcp_server)?
        .collect::<Result<Vec<_>, _>>()?;
    stored_servers
        .into_iter()
        .map(stored_mcp_server_into_record)
        .collect()
}

pub(crate) fn set_mcp_server_enabled(
    connection: &mut Connection,
    input: McpServerStatusUpdate,
) -> Result<McpServerRecord, StorageError> {
    validate_required_text("mcp_server.id", &input.id, 128)?;
    if input.expected_updated_at < 0 || input.updated_at <= input.expected_updated_at {
        return Err(StorageError::InvalidInput {
            field: "mcp_server.updated_at",
            reason: "must be greater than the non-negative expected_updated_at".to_owned(),
        });
    }
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let changed = transaction.execute(
        "UPDATE mcp_servers
         SET enabled = ?2, updated_at = ?3, last_error = NULL
         WHERE id = ?1 AND updated_at = ?4",
        params![
            &input.id,
            input.enabled,
            input.updated_at,
            input.expected_updated_at,
        ],
    )?;
    if changed == 0 {
        return mcp_server_write_failure(&transaction, &input.id);
    }
    let server =
        get_mcp_server(&transaction, &input.id)?.ok_or_else(|| StorageError::NotFound {
            entity: "MCP server",
            id: input.id.clone(),
        })?;
    transaction.commit()?;
    Ok(server)
}

pub(crate) fn delete_mcp_server(connection: &mut Connection, id: &str) -> Result<(), StorageError> {
    validate_required_text("mcp_server.id", id, 128)?;
    let changed = connection.execute("DELETE FROM mcp_servers WHERE id = ?1", params![id])?;
    if changed == 0 {
        return Err(StorageError::NotFound {
            entity: "MCP server",
            id: id.to_owned(),
        });
    }
    Ok(())
}

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

fn map_provider_profile(row: &Row<'_>) -> rusqlite::Result<ProviderProfileRecord> {
    Ok(ProviderProfileRecord {
        id: row.get(0)?,
        kind: row.get(1)?,
        endpoint: row.get(2)?,
        display_name: row.get(3)?,
        credential_ref: row.get(4)?,
        default_model: row.get(5)?,
        metadata_json: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        disabled_at: row.get(9)?,
        models: Vec::new(),
    })
}

struct StoredMcpServer {
    id: String,
    display_name: String,
    transport: String,
    config_json: String,
    enabled: bool,
    capabilities_json: Option<String>,
    schema_hash: Option<String>,
    last_error: Option<String>,
    created_at: i64,
    updated_at: i64,
}

fn map_stored_mcp_server(row: &Row<'_>) -> rusqlite::Result<StoredMcpServer> {
    Ok(StoredMcpServer {
        id: row.get(0)?,
        display_name: row.get(1)?,
        transport: row.get(2)?,
        config_json: row.get(3)?,
        enabled: row.get(4)?,
        capabilities_json: row.get(5)?,
        schema_hash: row.get(6)?,
        last_error: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn stored_mcp_server_into_record(stored: StoredMcpServer) -> Result<McpServerRecord, StorageError> {
    Ok(McpServerRecord {
        id: stored.id,
        display_name: stored.display_name,
        transport: McpTransport::from_storage(stored.transport)?,
        config_json: stored.config_json,
        enabled: stored.enabled,
        capabilities_json: stored.capabilities_json,
        schema_hash: stored.schema_hash,
        last_error: stored.last_error,
        created_at: stored.created_at,
        updated_at: stored.updated_at,
    })
}

fn mcp_server_write_failure<T>(connection: &Connection, id: &str) -> Result<T, StorageError> {
    let exists = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM mcp_servers WHERE id = ?1)",
        params![id],
        |row| row.get::<_, bool>(0),
    )?;
    if exists {
        Err(StorageError::StaleWrite {
            entity: "MCP server",
            id: id.to_owned(),
        })
    } else {
        Err(StorageError::NotFound {
            entity: "MCP server",
            id: id.to_owned(),
        })
    }
}

fn load_provider_models(
    connection: &Connection,
    provider_id: &str,
) -> Result<Vec<ProviderModelRecord>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT provider_id, model_id, display_name, capabilities_json, fetched_at
         FROM provider_models
         WHERE provider_id = ?1
         ORDER BY model_id",
    )?;
    let models = statement
        .query_map(params![provider_id], |row| {
            Ok(ProviderModelRecord {
                provider_id: row.get(0)?,
                model_id: row.get(1)?,
                display_name: row.get(2)?,
                capabilities_json: row.get(3)?,
                fetched_at: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(models)
}

fn replace_provider_models_in_transaction(
    connection: &Connection,
    provider_id: &str,
    models: &[NewProviderModel],
) -> Result<(), StorageError> {
    connection.execute(
        "DELETE FROM provider_models WHERE provider_id = ?1",
        params![provider_id],
    )?;
    let mut statement = connection.prepare(
        "INSERT INTO provider_models(
            provider_id, model_id, display_name, capabilities_json, fetched_at
         ) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for model in models {
        statement.execute(params![
            provider_id,
            &model.model_id,
            &model.display_name,
            &model.capabilities_json,
            model.fetched_at,
        ])?;
    }
    Ok(())
}

fn provider_write_failure<T>(connection: &Connection, id: &str) -> Result<T, StorageError> {
    let exists = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM provider_profiles WHERE id = ?1)",
        params![id],
        |row| row.get::<_, bool>(0),
    )?;
    if exists {
        Err(StorageError::StaleWrite {
            entity: "provider profile",
            id: id.to_owned(),
        })
    } else {
        Err(StorageError::NotFound {
            entity: "provider profile",
            id: id.to_owned(),
        })
    }
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

fn validate_new_provider_profile(input: &NewProviderProfile) -> Result<(), StorageError> {
    validate_provider_profile_fields(
        &input.id,
        &input.kind,
        input.endpoint.as_deref(),
        &input.display_name,
        input.credential_ref.as_deref(),
        input.default_model.as_deref(),
        &input.metadata_json,
    )?;
    if input.created_at < 0 {
        return Err(StorageError::InvalidInput {
            field: "provider.created_at",
            reason: "must be a non-negative Unix millisecond timestamp".to_owned(),
        });
    }
    validate_provider_models(&input.models)
}

fn validate_new_mcp_server(input: &NewMcpServer) -> Result<(), StorageError> {
    validate_required_text("mcp_server.id", &input.id, 128)?;
    validate_required_text("mcp_server.display_name", &input.display_name, 200)?;
    if input.created_at < 0 {
        return Err(StorageError::InvalidInput {
            field: "mcp_server.created_at",
            reason: "must be a non-negative Unix millisecond timestamp".to_owned(),
        });
    }
    validate_non_secret_json_object("mcp_server.config_json", &input.config_json)?;
    let config: Value = serde_json::from_str(&input.config_json)?;
    match input.transport {
        McpTransport::Stdio => {
            let command = config
                .get("command")
                .and_then(Value::as_str)
                .ok_or_else(|| StorageError::InvalidInput {
                    field: "mcp_server.config_json.command",
                    reason: "stdio config requires a command string".to_owned(),
                })?;
            validate_required_text("mcp_server.config_json.command", command, 4_096)?;
            if let Some(args) = config.get("args") {
                let args = args.as_array().ok_or_else(|| StorageError::InvalidInput {
                    field: "mcp_server.config_json.args",
                    reason: "must be an array of strings".to_owned(),
                })?;
                if args.len() > 128
                    || args.iter().any(|value| {
                        value.as_str().is_none_or(|argument| {
                            argument.len() > 4_096 || argument.chars().any(char::is_control)
                        })
                    })
                {
                    return Err(StorageError::InvalidInput {
                        field: "mcp_server.config_json.args",
                        reason: "must contain at most 128 bounded string arguments".to_owned(),
                    });
                }
            }
        }
        McpTransport::StreamableHttp => {
            let endpoint = config.get("url").and_then(Value::as_str).ok_or_else(|| {
                StorageError::InvalidInput {
                    field: "mcp_server.config_json.url",
                    reason: "Streamable HTTP config requires a URL string".to_owned(),
                }
            })?;
            validate_required_text("mcp_server.config_json.url", endpoint, 2_048)?;
            let parsed = Url::parse(endpoint).map_err(|_| StorageError::InvalidInput {
                field: "mcp_server.config_json.url",
                reason: "must be an absolute HTTP or HTTPS URL".to_owned(),
            })?;
            let loopback = matches!(
                parsed.host_str(),
                Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
            );
            let secure_scheme =
                parsed.scheme() == "https" || (parsed.scheme() == "http" && loopback);
            if !parsed.has_host()
                || !secure_scheme
                || !parsed.username().is_empty()
                || parsed.password().is_some()
                || parsed.query().is_some()
                || parsed.fragment().is_some()
            {
                return Err(StorageError::InvalidInput {
                    field: "mcp_server.config_json.url",
                    reason: "remote endpoints require HTTPS and must not embed credentials, query, or fragment"
                        .to_owned(),
                });
            }
        }
    }
    Ok(())
}

fn validate_provider_profile_update(input: &ProviderProfileUpdate) -> Result<(), StorageError> {
    validate_provider_profile_fields(
        &input.id,
        &input.kind,
        input.endpoint.as_deref(),
        &input.display_name,
        input.credential_ref.as_deref(),
        input.default_model.as_deref(),
        &input.metadata_json,
    )?;
    validate_monotonic_profile_update(&input.id, input.expected_updated_at, input.updated_at)?;
    if let Some(models) = &input.models {
        validate_provider_models(models)?;
    }
    Ok(())
}

fn validate_provider_profile_fields(
    id: &str,
    kind: &str,
    endpoint: Option<&str>,
    display_name: &str,
    credential_ref: Option<&str>,
    default_model: Option<&str>,
    metadata_json: &str,
) -> Result<(), StorageError> {
    validate_required_text("provider.id", id, 128)?;
    validate_required_text("provider.kind", kind, 64)?;
    validate_required_text("provider.display_name", display_name, 200)?;
    if let Some(default_model) = default_model {
        validate_required_text("provider.default_model", default_model, 255)?;
    }
    validate_provider_endpoint(endpoint)?;
    validate_credential_reference(credential_ref)?;
    validate_non_secret_json_object("provider.metadata_json", metadata_json)
}

fn validate_monotonic_profile_update(
    id: &str,
    expected_updated_at: i64,
    updated_at: i64,
) -> Result<(), StorageError> {
    validate_required_text("provider.id", id, 128)?;
    if expected_updated_at < 0 || updated_at <= expected_updated_at {
        return Err(StorageError::InvalidInput {
            field: "provider.updated_at",
            reason: "must be greater than the non-negative expected_updated_at".to_owned(),
        });
    }
    Ok(())
}

fn validate_provider_endpoint(endpoint: Option<&str>) -> Result<(), StorageError> {
    let Some(endpoint) = endpoint else {
        return Ok(());
    };
    validate_required_text("provider.endpoint", endpoint, 2_048)?;
    let parsed = Url::parse(endpoint).map_err(|_| StorageError::InvalidInput {
        field: "provider.endpoint",
        reason: "must be an absolute HTTP or HTTPS URL".to_owned(),
    })?;
    if !matches!(parsed.scheme(), "http" | "https") || !parsed.has_host() {
        return Err(StorageError::InvalidInput {
            field: "provider.endpoint",
            reason: "must be an absolute HTTP or HTTPS URL with a host".to_owned(),
        });
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(StorageError::InvalidInput {
            field: "provider.endpoint",
            reason: "must not contain embedded credentials".to_owned(),
        });
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(StorageError::InvalidInput {
            field: "provider.endpoint",
            reason: "must not contain a query string or fragment".to_owned(),
        });
    }
    Ok(())
}

fn validate_credential_reference(credential_ref: Option<&str>) -> Result<(), StorageError> {
    let Some(credential_ref) = credential_ref else {
        return Ok(());
    };
    if credential_ref.len() > 512 || credential_ref.trim() != credential_ref {
        return Err(StorageError::InvalidInput {
            field: "provider.credential_ref",
            reason: "must be a bounded canonical Cakify credential target".to_owned(),
        });
    }
    let segments = credential_ref.split('/').collect::<Vec<_>>();
    let opaque_is_valid = segments.get(2).is_some_and(|opaque| {
        !opaque.is_empty()
            && opaque.len() <= 128
            && opaque
                .chars()
                .all(|value| value.is_ascii_alphanumeric() || matches!(value, '-' | '_'))
    });
    if segments.len() != 4
        || segments[0] != "Cakify"
        || segments[1] != "provider"
        || !opaque_is_valid
        || segments[3] != "api-key"
    {
        return Err(StorageError::InvalidInput {
            field: "provider.credential_ref",
            reason: "must match Cakify/provider/<opaque>/api-key".to_owned(),
        });
    }
    Ok(())
}

fn validate_provider_models(models: &[NewProviderModel]) -> Result<(), StorageError> {
    let mut model_ids = HashSet::with_capacity(models.len());
    for model in models {
        validate_required_text("provider_model.model_id", &model.model_id, 255)?;
        if !model_ids.insert(model.model_id.as_str()) {
            return Err(StorageError::InvalidInput {
                field: "provider.models",
                reason: "model_id values must be unique".to_owned(),
            });
        }
        if let Some(display_name) = &model.display_name {
            validate_required_text("provider_model.display_name", display_name, 200)?;
        }
        if model.fetched_at < 0 {
            return Err(StorageError::InvalidInput {
                field: "provider_model.fetched_at",
                reason: "must be a non-negative Unix millisecond timestamp".to_owned(),
            });
        }
        validate_non_secret_json_object(
            "provider_model.capabilities_json",
            &model.capabilities_json,
        )?;
    }
    Ok(())
}

fn validate_required_text(
    field: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), StorageError> {
    if value.is_empty() || value.trim() != value {
        return Err(StorageError::InvalidInput {
            field,
            reason: "must be non-empty and have no surrounding whitespace".to_owned(),
        });
    }
    if value.len() > max_bytes || value.chars().any(char::is_control) {
        return Err(StorageError::InvalidInput {
            field,
            reason: format!("must be at most {max_bytes} bytes and contain no control characters"),
        });
    }
    Ok(())
}

fn validate_non_secret_json_object(field: &'static str, json: &str) -> Result<(), StorageError> {
    if json.len() > 65_536 {
        return Err(StorageError::InvalidInput {
            field,
            reason: "must be at most 65536 bytes".to_owned(),
        });
    }
    let root = serde_json::from_str::<Value>(json)?;
    if !root.is_object() {
        return Err(StorageError::InvalidInput {
            field,
            reason: "must be a JSON object".to_owned(),
        });
    }
    validate_non_secret_json_value(field, &root)
}

fn validate_provider_snapshot(field: &'static str, json: &str) -> Result<(), StorageError> {
    let root = serde_json::from_str::<Value>(json)?;
    validate_non_secret_json_value(field, &root)
}

fn validate_non_secret_json_value(field: &'static str, root: &Value) -> Result<(), StorageError> {
    let mut pending = vec![root];
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
                            | "x_api_key"
                            | "password"
                            | "secret"
                            | "client_secret"
                            | "clientsecret"
                            | "token"
                            | "access_token"
                            | "accesstoken"
                            | "refresh_token"
                            | "refreshtoken"
                            | "bearer_token"
                            | "authorization"
                            | "proxy_authorization"
                            | "cookie"
                            | "set_cookie"
                            | "credential"
                            | "credential_ref"
                            | "credential_reference"
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
