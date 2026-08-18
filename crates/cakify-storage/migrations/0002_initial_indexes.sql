CREATE INDEX conversations_activity_idx
    ON conversations(deleted_at, archived_at, updated_at DESC);

CREATE INDEX messages_parent_idx
    ON messages(parent_message_id)
    WHERE parent_message_id IS NOT NULL;

CREATE INDEX message_parts_attachment_idx
    ON message_parts(attachment_id)
    WHERE attachment_id IS NOT NULL;

CREATE INDEX runs_conversation_status_idx
    ON runs(conversation_id, status, updated_at DESC);

CREATE INDEX tool_calls_run_state_idx
    ON tool_calls(run_id, state);

CREATE INDEX provider_models_fetched_idx
    ON provider_models(provider_id, fetched_at DESC);

CREATE UNIQUE INDEX permission_rules_builtin_identity_unique
    ON permission_rules(tool_identity)
    WHERE server_id IS NULL;

CREATE UNIQUE INDEX permission_rules_server_identity_unique
    ON permission_rules(server_id, tool_identity)
    WHERE server_id IS NOT NULL;
