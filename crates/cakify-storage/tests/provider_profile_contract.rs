use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use cakify_storage::{
    NewConversation, NewProviderModel, NewProviderProfile, ProviderProfileRecord,
    ProviderProfileStatusUpdate, ProviderProfileUpdate, StorageActor, StorageConfig, StorageError,
};
use rusqlite::Connection;

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
            "cakify-provider-{label}-{}-{nonce}-{sequence}.db",
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

fn model(id: &str, fetched_at: i64) -> NewProviderModel {
    let mut model = NewProviderModel::new(id, fetched_at);
    model.display_name = Some(format!("Display {id}"));
    model.capabilities_json = "{\"tools\":true}".to_owned();
    model
}

fn profile(id: &str, display_name: &str, created_at: i64) -> NewProviderProfile {
    let mut profile = NewProviderProfile::new(id, "openai-compatible", display_name, created_at);
    profile.endpoint = Some("https://api.example.invalid/v1".to_owned());
    profile.credential_ref = Some(format!("Cakify/provider/{id}/api-key"));
    profile.default_model = Some("model-b".to_owned());
    profile.metadata_json = "{\"organization\":\"synthetic\"}".to_owned();
    profile.models = vec![model("model-b", created_at), model("model-a", created_at)];
    profile
}

fn update_from(profile: &ProviderProfileRecord, updated_at: i64) -> ProviderProfileUpdate {
    ProviderProfileUpdate {
        id: profile.id.clone(),
        kind: profile.kind.clone(),
        endpoint: profile.endpoint.clone(),
        display_name: profile.display_name.clone(),
        credential_ref: profile.credential_ref.clone(),
        default_model: profile.default_model.clone(),
        metadata_json: profile.metadata_json.clone(),
        expected_updated_at: profile.updated_at,
        updated_at,
        models: None,
    }
}

#[test]
fn provider_profiles_support_stable_crud_and_explicit_disabled_state() {
    let database = TestDatabase::new("crud");
    let actor = StorageActor::open(database.config()).expect("open actor");
    let handle = actor.handle();

    let zulu = handle
        .create_provider_profile(profile("provider-z", "Zulu", 10))
        .expect("create zulu provider");
    assert_eq!(
        zulu.models
            .iter()
            .map(|model| model.model_id.as_str())
            .collect::<Vec<_>>(),
        vec!["model-a", "model-b"]
    );
    let alpha = handle
        .create_provider_profile(profile("provider-a", "Alpha", 11))
        .expect("create alpha provider");

    let listed = handle
        .list_provider_profiles(false)
        .expect("list active providers");
    assert_eq!(
        listed
            .iter()
            .map(|profile| profile.id.as_str())
            .collect::<Vec<_>>(),
        vec!["provider-a", "provider-z"]
    );

    let mut update = update_from(&alpha, 12);
    update.display_name = "Beta".to_owned();
    update.endpoint = Some("https://gateway.example.invalid/openai/v1".to_owned());
    let updated = handle
        .update_provider_profile(update.clone())
        .expect("update profile while preserving cache");
    assert_eq!(updated.display_name, "Beta");
    assert_eq!(updated.models, alpha.models);

    assert!(matches!(
        handle
            .update_provider_profile(update)
            .expect_err("reusing an old revision must fail"),
        StorageError::StaleWrite { .. }
    ));

    let disabled = handle
        .set_provider_profile_disabled(ProviderProfileStatusUpdate {
            id: updated.id.clone(),
            disabled_at: Some(13),
            expected_updated_at: updated.updated_at,
            updated_at: 13,
        })
        .expect("disable provider");
    assert_eq!(disabled.disabled_at, Some(13));
    assert_eq!(
        handle
            .list_provider_profiles(false)
            .expect("list active providers")
            .iter()
            .map(|profile| profile.id.as_str())
            .collect::<Vec<_>>(),
        vec!["provider-z"]
    );

    let including_disabled = handle
        .list_provider_profiles(true)
        .expect("list all providers");
    assert_eq!(including_disabled[0].id, zulu.id);
    assert_eq!(including_disabled[1].id, disabled.id);

    let enabled = handle
        .set_provider_profile_disabled(ProviderProfileStatusUpdate {
            id: disabled.id,
            disabled_at: None,
            expected_updated_at: disabled.updated_at,
            updated_at: 14,
        })
        .expect("enable provider");
    assert_eq!(enabled.disabled_at, None);
}

#[test]
fn provider_configuration_rejects_credentials_outside_the_opaque_reference() {
    let database = TestDatabase::new("secret-boundary");
    let actor = StorageActor::open(database.config()).expect("open actor");
    let handle = actor.handle();

    let mut embedded_endpoint = profile("endpoint-secret", "Endpoint secret", 1);
    embedded_endpoint.endpoint = Some("https://user:password@example.invalid/v1".to_owned());
    assert!(matches!(
        handle
            .create_provider_profile(embedded_endpoint)
            .expect_err("endpoint userinfo must fail"),
        StorageError::InvalidInput {
            field: "provider.endpoint",
            ..
        }
    ));

    let mut plaintext_reference = profile("plaintext-ref", "Plaintext ref", 2);
    plaintext_reference.credential_ref = Some("synthetic-super-secret-value".to_owned());
    assert!(matches!(
        handle
            .create_provider_profile(plaintext_reference)
            .expect_err("plaintext credential must fail"),
        StorageError::InvalidInput {
            field: "provider.credential_ref",
            ..
        }
    ));

    let mut sensitive_metadata = profile("metadata-secret", "Metadata secret", 3);
    let forbidden_key = ["api", "key"].join("_");
    sensitive_metadata.metadata_json =
        format!("{{\"nested\":{{\"{forbidden_key}\":\"synthetic-secret-marker\"}}}}");
    assert!(matches!(
        handle
            .create_provider_profile(sensitive_metadata)
            .expect_err("credential-bearing metadata must fail"),
        StorageError::SensitiveJsonKey { .. }
    ));

    let mut sensitive_capability = profile("capability-secret", "Capability secret", 4);
    let forbidden_key = ["access", "token"].join("_");
    sensitive_capability.models[0].capabilities_json =
        format!("{{\"{forbidden_key}\":\"synthetic-capability-secret\"}}");
    assert!(matches!(
        handle
            .create_provider_profile(sensitive_capability)
            .expect_err("credential-bearing capability must fail"),
        StorageError::SensitiveJsonKey { .. }
    ));

    let valid = handle
        .create_provider_profile(profile("valid", "Valid", 5))
        .expect("create valid profile");
    let mut duplicate_reference = profile("duplicate-ref", "Duplicate reference", 6);
    duplicate_reference.credential_ref = valid.credential_ref;
    assert!(matches!(
        handle
            .create_provider_profile(duplicate_reference)
            .expect_err("credential references must be unique"),
        StorageError::Sqlite(_)
    ));
    assert!(handle
        .get_provider_profile("endpoint-secret")
        .expect("look up rejected profile")
        .is_none());
    drop(actor);

    let connection = Connection::open(&database.path).expect("inspect database");
    let columns = connection
        .prepare("PRAGMA table_info(provider_profiles)")
        .expect("prepare columns")
        .query_map([], |row| row.get::<_, String>(1))
        .expect("read columns")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect columns");
    assert!(columns.iter().any(|column| column == "credential_ref"));
    assert!(!columns.iter().any(|column| matches!(
        column.as_str(),
        "api_key" | "password" | "secret" | "token" | "credential_blob"
    )));
    drop(connection);

    let database_bytes = fs::read(&database.path).expect("read database bytes");
    for marker in [
        b"synthetic-super-secret-value".as_slice(),
        b"synthetic-secret-marker".as_slice(),
        b"synthetic-capability-secret".as_slice(),
    ] {
        assert!(!database_bytes
            .windows(marker.len())
            .any(|window| window == marker));
    }
}

#[test]
fn profile_and_model_cache_changes_commit_or_roll_back_together() {
    let database = TestDatabase::new("atomic-model-cache");
    let actor = StorageActor::open(database.config()).expect("open actor");
    let handle = actor.handle();
    let original = handle
        .create_provider_profile(profile("provider-1", "Original", 10))
        .expect("create provider");

    let mut invalid_update = update_from(&original, 11);
    invalid_update.display_name = "Must roll back".to_owned();
    let mut invalid_model = model("replacement", 11);
    invalid_model.capabilities_json = "{".to_owned();
    invalid_update.models = Some(vec![invalid_model]);
    assert!(matches!(
        handle
            .update_provider_profile(invalid_update)
            .expect_err("invalid model JSON must reject the aggregate"),
        StorageError::Json(_)
    ));
    assert_eq!(
        handle
            .get_provider_profile("provider-1")
            .expect("reload original")
            .expect("provider exists"),
        original
    );

    let mut valid_update = update_from(&original, 12);
    valid_update.display_name = "Updated".to_owned();
    valid_update.default_model = Some("replacement-b".to_owned());
    valid_update.models = Some(vec![model("replacement-b", 12), model("replacement-a", 12)]);
    let updated = handle
        .update_provider_profile(valid_update)
        .expect("replace profile and models");
    assert_eq!(updated.display_name, "Updated");
    assert_eq!(
        updated
            .models
            .iter()
            .map(|model| model.model_id.as_str())
            .collect::<Vec<_>>(),
        vec!["replacement-a", "replacement-b"]
    );

    assert!(matches!(
        handle
            .replace_provider_models(
                "provider-1",
                vec![model("duplicate", 13), model("duplicate", 13)],
            )
            .expect_err("duplicate model ids must fail"),
        StorageError::InvalidInput {
            field: "provider.models",
            ..
        }
    ));
    assert_eq!(
        handle
            .get_provider_profile("provider-1")
            .expect("reload after rejected cache")
            .expect("provider exists")
            .models,
        updated.models
    );
}

#[test]
fn deleting_a_profile_returns_the_secret_reference_and_cascades_public_data() {
    let database = TestDatabase::new("delete");
    let actor = StorageActor::open(database.config()).expect("open actor");
    let handle = actor.handle();
    let profile = handle
        .create_provider_profile(profile("provider-1", "Delete", 1))
        .expect("create provider");
    let mut conversation = NewConversation::new("conversation-1", "Conversation", 2);
    conversation.provider_id = Some(profile.id.clone());
    conversation.model_id = Some("model-a".to_owned());
    handle
        .create_conversation(conversation)
        .expect("create linked conversation");

    let deleted = handle
        .delete_provider_profile("provider-1")
        .expect("delete provider");
    assert_eq!(deleted.id, "provider-1");
    assert_eq!(
        deleted.credential_ref.as_deref(),
        Some("Cakify/provider/provider-1/api-key")
    );
    assert!(handle
        .get_provider_profile("provider-1")
        .expect("reload deleted provider")
        .is_none());
    let conversation = handle
        .get_conversation("conversation-1")
        .expect("reload conversation")
        .expect("conversation remains");
    assert_eq!(conversation.provider_id, None);
    assert_eq!(conversation.model_id.as_deref(), Some("model-a"));
    assert!(matches!(
        handle
            .replace_provider_models("provider-1", vec![model("orphan", 3)])
            .expect_err("deleted provider cannot accept models"),
        StorageError::NotFound { .. }
    ));
    assert!(matches!(
        handle
            .delete_provider_profile("provider-1")
            .expect_err("second delete must report missing profile"),
        StorageError::NotFound { .. }
    ));
}
