use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use cakify_storage::{
    ConversationQuery, MessagePartKind, MessageRole, NewConversation, NewMessage, NewMessagePart,
    NewRun, RunStatus, RunUpdate, StorageActor, StorageConfig, StorageError, TextCheckpoint,
};

static NEXT_DATABASE: AtomicU64 = AtomicU64::new(1);

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
            "cakify-repository-{label}-{}-{nonce}-{sequence}.db",
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

#[test]
fn conversation_pagination_is_stable_and_excludes_soft_deleted_rows() {
    let database = TestDatabase::new("pagination");
    let actor = StorageActor::open(database.config()).expect("open actor");
    let handle = actor.handle();

    let mut sensitive_snapshot = NewConversation::new("rejected", "Rejected", 99);
    let forbidden_key = ["api", "key"].join("_");
    sensitive_snapshot.provider_snapshot_json =
        format!("{{\"{forbidden_key}\":\"synthetic-value\"}}");
    assert!(matches!(
        handle
            .create_conversation(sensitive_snapshot)
            .expect_err("credential-bearing snapshot must fail"),
        StorageError::SensitiveJsonKey { .. }
    ));
    assert!(
        handle
            .get_conversation("rejected")
            .expect("look up rejected conversation")
            .is_none()
    );

    for id in ["conversation-1", "conversation-2", "conversation-3"] {
        handle
            .create_conversation(NewConversation::new(id, id, 100))
            .expect("create conversation");
    }

    let first = handle
        .list_conversations(ConversationQuery::new(2))
        .expect("first page");
    assert_eq!(
        first
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["conversation-3", "conversation-2"]
    );
    let mut next_query = ConversationQuery::new(2);
    next_query.cursor = first.next_cursor;
    let second = handle
        .list_conversations(next_query)
        .expect("second page");
    assert_eq!(
        second
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["conversation-1"]
    );
    assert!(second.next_cursor.is_none());

    let deleted = handle
        .mark_conversation_deleted("conversation-2", 101)
        .expect("soft delete");
    assert_eq!(deleted.deleted_at, Some(101));
    let visible = handle
        .list_conversations(ConversationQuery::new(10))
        .expect("visible conversations");
    assert_eq!(
        visible
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["conversation-3", "conversation-1"]
    );

    let error = handle
        .list_conversations(ConversationQuery::new(0))
        .expect_err("zero page size must fail");
    assert!(matches!(error, StorageError::InvalidPageLimit { limit: 0, .. }));
}

#[test]
fn message_and_parts_commit_atomically_and_load_in_ordinal_order() {
    let database = TestDatabase::new("message-transaction");
    let actor = StorageActor::open(database.config()).expect("open actor");
    let handle = actor.handle();
    handle
        .create_conversation(NewConversation::new("conversation-1", "Atomic", 1))
        .expect("create conversation");

    let first = NewMessage::new(
        "message-1",
        "conversation-1",
        MessageRole::User,
        0,
        2,
        vec![
            NewMessagePart::text("part-1", "hello", 2),
            NewMessagePart {
                id: "part-2".to_owned(),
                kind: MessagePartKind::Citation,
                text_content: None,
                content_json: Some("{\"url\":\"https://example.invalid\"}".to_owned()),
                attachment_id: None,
                created_at: 2,
            },
        ],
    );
    handle.append_message(first).expect("append first message");

    let invalid = NewMessage::new(
        "message-invalid",
        "conversation-1",
        MessageRole::Assistant,
        1,
        3,
        vec![NewMessagePart {
            id: "part-invalid".to_owned(),
            kind: MessagePartKind::ToolResult,
            text_content: None,
            content_json: Some("{".to_owned()),
            attachment_id: None,
            created_at: 3,
        }],
    );
    assert!(matches!(
        handle
            .append_message(invalid)
            .expect_err("invalid JSON must roll back aggregate"),
        StorageError::Sqlite(_)
    ));

    handle
        .append_message(NewMessage::new(
            "message-2",
            "conversation-1",
            MessageRole::Assistant,
            1,
            4,
            vec![NewMessagePart::text("part-3", "world", 4)],
        ))
        .expect("append second message");

    let thread = handle
        .load_thread("conversation-1")
        .expect("load thread")
        .expect("conversation exists");
    assert_eq!(
        thread
            .messages
            .iter()
            .map(|message| message.id.as_str())
            .collect::<Vec<_>>(),
        vec!["message-1", "message-2"]
    );
    assert_eq!(thread.messages[0].parts.len(), 2);
    assert_eq!(thread.messages[0].parts[0].ordinal, 0);
    assert_eq!(thread.messages[0].parts[1].ordinal, 1);
    assert_eq!(thread.messages[1].parts[0].revision, 0);
    assert!(
        thread
            .messages
            .iter()
            .all(|message| message.id != "message-invalid")
    );
}

#[test]
fn checkpoint_survives_restart_and_active_run_recovers_once() {
    let database = TestDatabase::new("recovery");
    let actor = StorageActor::open(database.config()).expect("open actor");
    assert_eq!(actor.startup_recovery().recovered_count(), 0);
    let handle = actor.handle();
    handle
        .create_conversation(NewConversation::new("conversation-1", "Recovery", 1))
        .expect("create conversation");
    handle
        .append_message(NewMessage::new(
            "assistant-message-1",
            "conversation-1",
            MessageRole::Assistant,
            0,
            2,
            vec![NewMessagePart::text("text-part-1", "", 2)],
        ))
        .expect("append assistant placeholder");
    handle
        .create_run(NewRun {
            id: "run-1".to_owned(),
            conversation_id: "conversation-1".to_owned(),
            assistant_message_id: Some("assistant-message-1".to_owned()),
            status: RunStatus::Streaming,
            provider_snapshot_json: "{\"kind\":\"synthetic\"}".to_owned(),
            model_id: "synthetic-model".to_owned(),
            started_at: 2,
        })
        .expect("create active run");

    let checkpoint = TextCheckpoint {
        conversation_id: "conversation-1".to_owned(),
        message_id: "assistant-message-1".to_owned(),
        part_id: "text-part-1".to_owned(),
        text_content: "partial response".to_owned(),
        revision: 1,
        updated_at: 3,
    };
    handle
        .checkpoint_text(checkpoint.clone())
        .expect("first checkpoint");
    handle
        .checkpoint_text(checkpoint)
        .expect("identical checkpoint is idempotent");
    let stale = TextCheckpoint {
        conversation_id: "conversation-1".to_owned(),
        message_id: "assistant-message-1".to_owned(),
        part_id: "text-part-1".to_owned(),
        text_content: "older conflicting response".to_owned(),
        revision: 1,
        updated_at: 4,
    };
    assert!(matches!(
        handle
            .checkpoint_text(stale)
            .expect_err("conflicting same revision must fail"),
        StorageError::StaleCheckpoint {
            current_revision: 1,
            attempted_revision: 1,
            ..
        }
    ));
    drop(actor);

    let recovered = StorageActor::open(database.config()).expect("reopen for recovery");
    assert_eq!(
        recovered.startup_recovery().interrupted_run_ids,
        vec!["run-1".to_owned()]
    );
    let recovered_handle = recovered.handle();
    let run = recovered_handle
        .get_run("run-1")
        .expect("load run")
        .expect("run exists");
    assert_eq!(run.status, RunStatus::Interrupted);
    assert_eq!(run.finish_reason.as_deref(), Some("app_restart"));
    assert_eq!(run.error_kind.as_deref(), Some("interrupted"));
    assert!(run.finished_at.is_some());
    let thread = recovered_handle
        .load_thread("conversation-1")
        .expect("load recovered thread")
        .expect("thread exists");
    assert_eq!(
        thread.messages[0].parts[0].text_content.as_deref(),
        Some("partial response")
    );
    assert_eq!(thread.messages[0].parts[0].revision, 1);
    drop(recovered);

    let second_reopen = StorageActor::open(database.config()).expect("second reopen");
    assert_eq!(second_reopen.startup_recovery().recovered_count(), 0);
    let second_handle = second_reopen.handle();
    assert!(
        second_handle
            .purge_conversation("conversation-1")
            .expect("purge conversation")
    );
    assert!(
        second_handle
            .load_thread("conversation-1")
            .expect("load purged thread")
            .is_none()
    );
    assert!(
        second_handle
            .get_run("run-1")
            .expect("load cascaded run")
            .is_none()
    );
}

#[test]
fn run_updates_are_monotonic_and_terminal_runs_cannot_reopen() {
    let database = TestDatabase::new("run-transition");
    let actor = StorageActor::open(database.config()).expect("open actor");
    let handle = actor.handle();
    handle
        .create_conversation(NewConversation::new("conversation-1", "Run", 1))
        .expect("create conversation");
    let created = handle
        .create_run(NewRun {
            id: "run-1".to_owned(),
            conversation_id: "conversation-1".to_owned(),
            assistant_message_id: None,
            status: RunStatus::Requesting,
            provider_snapshot_json: "{}".to_owned(),
            model_id: "synthetic-model".to_owned(),
            started_at: 2,
        })
        .expect("create run");
    assert_eq!(created.status, RunStatus::Requesting);

    let streaming = handle
        .update_run(RunUpdate {
            id: "run-1".to_owned(),
            status: RunStatus::Streaming,
            updated_at: 3,
            finished_at: None,
            finish_reason: None,
            error_kind: None,
            error_message: None,
            usage_json: None,
            cancel_source: None,
        })
        .expect("enter streaming");
    assert_eq!(streaming.status, RunStatus::Streaming);

    let completed = handle
        .update_run(RunUpdate {
            id: "run-1".to_owned(),
            status: RunStatus::Completed,
            updated_at: 4,
            finished_at: Some(4),
            finish_reason: Some("stop".to_owned()),
            error_kind: None,
            error_message: None,
            usage_json: Some("{\"output_tokens\":2}".to_owned()),
            cancel_source: None,
        })
        .expect("complete run");
    assert_eq!(completed.status, RunStatus::Completed);
    assert_eq!(completed.usage_json.as_deref(), Some("{\"output_tokens\":2}"));

    let stale = RunUpdate {
        id: "run-1".to_owned(),
        status: RunStatus::Completed,
        updated_at: 3,
        finished_at: Some(3),
        finish_reason: Some("stop".to_owned()),
        error_kind: None,
        error_message: None,
        usage_json: None,
        cancel_source: None,
    };
    assert!(matches!(
        handle
            .update_run(stale)
            .expect_err("stale run write must fail"),
        StorageError::StaleWrite { .. }
    ));

    let reopen = RunUpdate {
        id: "run-1".to_owned(),
        status: RunStatus::Streaming,
        updated_at: 5,
        finished_at: None,
        finish_reason: None,
        error_kind: None,
        error_message: None,
        usage_json: None,
        cancel_source: None,
    };
    assert!(matches!(
        handle
            .update_run(reopen)
            .expect_err("terminal run must not reopen"),
        StorageError::InvalidRunTransition {
            from: "completed",
            to: "streaming"
        }
    ));
}
