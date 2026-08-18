#![cfg(windows)]

use std::{
    fs,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use cakify_core::{SecretError, SecretId, SecretInput, SecretStore};
use cakify_platform_windows::{CredentialManagerSecretStore, DpapiSecretStore};

fn unique_id(label: &str) -> SecretId {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    SecretId::new(format!(
        "Cakify/provider/ci-{label}-{}-{nonce}/api-key",
        process::id()
    ))
    .expect("synthetic secret id")
}

struct CredentialCleanup {
    store: CredentialManagerSecretStore,
    id: SecretId,
}

impl Drop for CredentialCleanup {
    fn drop(&mut self) {
        let _ = self.store.delete(&self.id);
    }
}

struct DirectoryCleanup(PathBuf);

impl Drop for DirectoryCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn credential_manager_put_get_update_delete_round_trip() {
    let store = CredentialManagerSecretStore;
    let id = unique_id("credman");
    let _cleanup = CredentialCleanup {
        store,
        id: id.clone(),
    };
    store
        .delete(&id)
        .expect("remove stale synthetic credential");
    assert!(!store.contains(&id).expect("initial contains"));

    let first = SecretInput::from_utf8("cakify-ci-synthetic-value-v1").expect("first value");
    store.put(&id, &first).expect("write credential");
    assert!(store.contains(&id).expect("contains after write"));
    let fetched = store.get(&id).expect("read credential");
    assert!(fetched.expose_secret() == first.expose_secret());

    let second = SecretInput::from_utf8("cakify-ci-synthetic-value-v2").expect("second value");
    store.put(&id, &second).expect("update credential");
    let fetched = store.get(&id).expect("read updated credential");
    assert!(fetched.expose_secret() == second.expose_secret());

    store.delete(&id).expect("delete credential");
    store.delete(&id).expect("delete is idempotent");
    assert!(!store.contains(&id).expect("contains after delete"));
    assert!(matches!(store.get(&id), Err(SecretError::NotFound { .. })));
}

#[test]
fn dpapi_current_user_round_trip_writes_only_ciphertext_and_rejects_tamper() {
    let id = unique_id("dpapi");
    let root = temporary_root("dpapi");
    let _cleanup = DirectoryCleanup(root.clone());
    let store = DpapiSecretStore::new(&root);
    let value = SecretInput::from_utf8("cakify-ci-dpapi-synthetic-token-bundle")
        .expect("synthetic DPAPI value");

    store.put(&id, &value).expect("protect and write");
    assert!(store.contains(&id).expect("contains encrypted value"));
    let files = secret_files(&root);
    assert_eq!(files.len(), 1, "one DPAPI ciphertext file");
    let payload = fs::read(&files[0]).expect("read ciphertext file");
    assert!(!contains_subslice(&payload, value.expose_secret()));

    let fetched = store.get(&id).expect("unprotect value");
    assert!(fetched.expose_secret() == value.expose_secret());

    let mut tampered = payload;
    let last = tampered.last_mut().expect("ciphertext byte");
    *last ^= 0x5a;
    fs::write(&files[0], tampered).expect("tamper ciphertext");
    assert!(store.get(&id).is_err(), "tampered ciphertext must fail");

    store.delete(&id).expect("delete ciphertext");
    store.delete(&id).expect("delete is idempotent");
    assert!(!store.contains(&id).expect("contains after delete"));
}

fn temporary_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("cakify-{label}-{}-{nonce}", process::id()))
}

fn secret_files(root: &Path) -> Vec<PathBuf> {
    fs::read_dir(root)
        .expect("read secret directory")
        .map(|entry| entry.expect("secret directory entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "dpapi")
        })
        .collect()
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}
