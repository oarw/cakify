CREATE TABLE app_settings (
    key TEXT PRIMARY KEY NOT NULL CHECK(length(trim(key)) > 0),
    value_json TEXT NOT NULL CHECK(json_valid(value_json)),
    updated_at INTEGER NOT NULL CHECK(updated_at >= 0)
) STRICT;

CREATE TABLE provider_profiles (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(trim(id)) > 0),
    kind TEXT NOT NULL CHECK(length(trim(kind)) > 0),
    endpoint TEXT,
    display_name TEXT NOT NULL CHECK(length(trim(display_name)) > 0),
    credential_ref TEXT UNIQUE,
    default_model TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json)),
    created_at INTEGER NOT NULL CHECK(created_at >= 0),
    updated_at INTEGER NOT NULL CHECK(updated_at >= created_at),
    disabled_at INTEGER CHECK(disabled_at IS NULL OR disabled_at >= created_at)
) STRICT;

CREATE TABLE provider_models (
    provider_id TEXT NOT NULL,
    model_id TEXT NOT NULL CHECK(length(trim(model_id)) > 0),
    display_name TEXT,
    capabilities_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(capabilities_json)),
    fetched_at INTEGER NOT NULL CHECK(fetched_at >= 0),
    PRIMARY KEY(provider_id, model_id),
    FOREIGN KEY(provider_id) REFERENCES provider_profiles(id) ON DELETE CASCADE
) STRICT;

CREATE TABLE attachments (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(trim(id)) > 0),
    sha256 TEXT NOT NULL UNIQUE CHECK(
        length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*'
    ),
    storage_name TEXT NOT NULL UNIQUE CHECK(length(trim(storage_name)) > 0),
    display_name TEXT NOT NULL CHECK(length(trim(display_name)) > 0),
    media_type TEXT,
    size_bytes INTEGER NOT NULL CHECK(size_bytes >= 0),
    created_at INTEGER NOT NULL CHECK(created_at >= 0)
) STRICT;

CREATE TABLE conversations (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(trim(id)) > 0),
    title TEXT NOT NULL,
    provider_id TEXT,
    model_id TEXT,
    provider_snapshot_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(provider_snapshot_json)),
    system_instruction TEXT,
    created_at INTEGER NOT NULL CHECK(created_at >= 0),
    updated_at INTEGER NOT NULL CHECK(updated_at >= created_at),
    archived_at INTEGER CHECK(archived_at IS NULL OR archived_at >= created_at),
    deleted_at INTEGER CHECK(deleted_at IS NULL OR deleted_at >= created_at),
    FOREIGN KEY(provider_id) REFERENCES provider_profiles(id) ON DELETE SET NULL
) STRICT;

CREATE TABLE messages (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(trim(id)) > 0),
    conversation_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('system', 'user', 'assistant', 'tool')),
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
    parent_message_id TEXT,
    edited_from_message_id TEXT,
    created_at INTEGER NOT NULL CHECK(created_at >= 0),
    deleted_at INTEGER CHECK(deleted_at IS NULL OR deleted_at >= created_at),
    UNIQUE(conversation_id, ordinal),
    FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
    FOREIGN KEY(parent_message_id) REFERENCES messages(id) ON DELETE SET NULL,
    FOREIGN KEY(edited_from_message_id) REFERENCES messages(id) ON DELETE SET NULL
) STRICT;

CREATE TABLE message_parts (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(trim(id)) > 0),
    message_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
    kind TEXT NOT NULL CHECK(kind IN (
        'text', 'reasoning_summary', 'image', 'file', 'tool_call',
        'tool_result', 'citation', 'error'
    )),
    text_content TEXT,
    content_json TEXT CHECK(content_json IS NULL OR json_valid(content_json)),
    attachment_id TEXT,
    created_at INTEGER NOT NULL CHECK(created_at >= 0),
    UNIQUE(message_id, ordinal),
    CHECK(text_content IS NOT NULL OR content_json IS NOT NULL OR attachment_id IS NOT NULL),
    FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE,
    FOREIGN KEY(attachment_id) REFERENCES attachments(id) ON DELETE SET NULL
) STRICT;

CREATE TABLE runs (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(trim(id)) > 0),
    conversation_id TEXT NOT NULL,
    assistant_message_id TEXT,
    status TEXT NOT NULL CHECK(status IN (
        'preparing', 'requesting', 'streaming', 'awaiting_approval',
        'tool_running', 'completed', 'failed', 'cancelled', 'interrupted'
    )),
    provider_snapshot_json TEXT NOT NULL CHECK(json_valid(provider_snapshot_json)),
    model_id TEXT NOT NULL CHECK(length(trim(model_id)) > 0),
    started_at INTEGER NOT NULL CHECK(started_at >= 0),
    updated_at INTEGER NOT NULL CHECK(updated_at >= started_at),
    finished_at INTEGER CHECK(finished_at IS NULL OR finished_at >= started_at),
    finish_reason TEXT,
    error_kind TEXT,
    error_message TEXT,
    usage_json TEXT CHECK(usage_json IS NULL OR json_valid(usage_json)),
    cancel_source TEXT,
    FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
    FOREIGN KEY(assistant_message_id) REFERENCES messages(id) ON DELETE SET NULL
) STRICT;

CREATE TABLE mcp_servers (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(trim(id)) > 0),
    display_name TEXT NOT NULL CHECK(length(trim(display_name)) > 0),
    transport TEXT NOT NULL CHECK(transport IN ('stdio', 'streamable_http')),
    config_json TEXT NOT NULL CHECK(json_valid(config_json)),
    enabled INTEGER NOT NULL DEFAULT 0 CHECK(enabled IN (0, 1)),
    capabilities_json TEXT CHECK(capabilities_json IS NULL OR json_valid(capabilities_json)),
    schema_hash TEXT,
    last_error TEXT,
    created_at INTEGER NOT NULL CHECK(created_at >= 0),
    updated_at INTEGER NOT NULL CHECK(updated_at >= created_at)
) STRICT;

CREATE TABLE tool_calls (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(trim(id)) > 0),
    run_id TEXT NOT NULL,
    message_part_id TEXT,
    tool_identity TEXT NOT NULL CHECK(length(trim(tool_identity)) > 0),
    arguments_json TEXT NOT NULL CHECK(json_valid(arguments_json)),
    state TEXT NOT NULL CHECK(state IN (
        'proposed', 'waiting_approval', 'running', 'completed',
        'failed', 'cancelled'
    )),
    approval TEXT NOT NULL DEFAULT 'pending' CHECK(approval IN (
        'pending', 'allowed_once', 'allowed_rule', 'denied'
    )),
    output_text TEXT,
    output_json TEXT CHECK(output_json IS NULL OR json_valid(output_json)),
    output_truncated INTEGER NOT NULL DEFAULT 0 CHECK(output_truncated IN (0, 1)),
    started_at INTEGER,
    finished_at INTEGER,
    error_message TEXT,
    FOREIGN KEY(run_id) REFERENCES runs(id) ON DELETE CASCADE,
    FOREIGN KEY(message_part_id) REFERENCES message_parts(id) ON DELETE SET NULL,
    CHECK(finished_at IS NULL OR started_at IS NULL OR finished_at >= started_at)
) STRICT;

CREATE TABLE permission_rules (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(trim(id)) > 0),
    tool_identity TEXT NOT NULL CHECK(length(trim(tool_identity)) > 0),
    server_id TEXT,
    decision TEXT NOT NULL CHECK(decision IN ('allow', 'deny')),
    schema_hash TEXT,
    created_at INTEGER NOT NULL CHECK(created_at >= 0),
    updated_at INTEGER NOT NULL CHECK(updated_at >= created_at),
    FOREIGN KEY(server_id) REFERENCES mcp_servers(id) ON DELETE CASCADE
) STRICT;

CREATE TABLE conversation_mcp_servers (
    conversation_id TEXT NOT NULL,
    server_id TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    PRIMARY KEY(conversation_id, server_id),
    FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
    FOREIGN KEY(server_id) REFERENCES mcp_servers(id) ON DELETE CASCADE
) STRICT;
