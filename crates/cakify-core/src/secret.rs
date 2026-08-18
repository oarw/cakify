use std::fmt;

use thiserror::Error;
use zeroize::Zeroizing;

const MAX_SECRET_ID_BYTES: usize = 512;
const MAX_SECRET_VALUE_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SecretId(String);

impl SecretId {
    pub fn new(value: impl Into<String>) -> Result<Self, SecretError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_SECRET_ID_BYTES
            || value.trim() != value
            || value.chars().any(char::is_control)
        {
            return Err(SecretError::InvalidId);
        }

        let valid_segment = |segment: &str| {
            !segment.is_empty()
                && segment.len() <= 128
                && segment
                    .chars()
                    .all(|value| value.is_ascii_alphanumeric() || matches!(value, '-' | '_' | '.'))
        };
        let segments = value.split('/').collect::<Vec<_>>();
        let valid_shape = match segments.as_slice() {
            ["Cakify", "provider", opaque, "api-key"] => valid_segment(opaque),
            ["Cakify", "mcp", server, name] => valid_segment(server) && valid_segment(name),
            _ => false,
        };
        if !valid_shape {
            return Err(SecretError::InvalidId);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SecretId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

pub struct SecretInput(Zeroizing<Vec<u8>>);

impl SecretInput {
    pub fn from_bytes(value: impl Into<Vec<u8>>) -> Result<Self, SecretError> {
        let value = Zeroizing::new(value.into());
        if value.is_empty() || value.len() > MAX_SECRET_VALUE_BYTES {
            return Err(SecretError::InvalidValue);
        }
        Ok(Self(value))
    }

    pub fn from_utf8(value: impl Into<String>) -> Result<Self, SecretError> {
        Self::from_bytes(value.into().into_bytes())
    }

    pub fn expose_secret(&self) -> &[u8] {
        self.0.as_slice()
    }
}

pub struct SecretValue(Zeroizing<Vec<u8>>);

impl SecretValue {
    pub fn from_bytes(value: Vec<u8>) -> Result<Self, SecretError> {
        let value = Zeroizing::new(value);
        if value.is_empty() || value.len() > MAX_SECRET_VALUE_BYTES {
            return Err(SecretError::InvalidValue);
        }
        Ok(Self(value))
    }

    pub fn expose_secret(&self) -> &[u8] {
        self.0.as_slice()
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum SecretError {
    #[error("secret id is invalid")]
    InvalidId,
    #[error("secret value is empty or exceeds the supported size")]
    InvalidValue,
    #[error("secret {id} was not found")]
    NotFound { id: SecretId },
    #[error("secret backend operation {operation} failed with code {code}")]
    Backend { operation: &'static str, code: i32 },
    #[error("secret backend operation {operation} returned invalid data")]
    Corrupt { operation: &'static str },
    #[error("secret backend operation {operation} failed with I/O code {code}")]
    Io { operation: &'static str, code: i32 },
}

pub trait SecretStore: Send + Sync {
    fn put(&self, id: &SecretId, value: &SecretInput) -> Result<(), SecretError>;
    fn get(&self, id: &SecretId) -> Result<SecretValue, SecretError>;
    fn delete(&self, id: &SecretId) -> Result<(), SecretError>;
    fn contains(&self, id: &SecretId) -> Result<bool, SecretError>;
}

#[derive(Debug, Error)]
pub enum SecretLifecycleError<E: std::error::Error + Send + Sync + 'static> {
    #[error("secret write failed: {0}")]
    Store(#[source] SecretError),
    #[error("committing the secret reference failed; cleanup={cleanup:?}: {source}")]
    ReferenceCommit {
        #[source]
        source: E,
        cleanup: Option<SecretError>,
    },
    #[error("deleting the secret reference failed: {0}")]
    ReferenceDelete(#[source] E),
    #[error("secret cleanup failed after the reference was deleted: {0}")]
    Cleanup(#[source] SecretError),
}

pub fn put_then_commit_reference<E, F>(
    store: &dyn SecretStore,
    id: &SecretId,
    value: &SecretInput,
    commit_reference: F,
) -> Result<(), SecretLifecycleError<E>>
where
    E: std::error::Error + Send + Sync + 'static,
    F: FnOnce(&SecretId) -> Result<(), E>,
{
    let previous = match store.get(id) {
        Ok(value) => Some(value),
        Err(SecretError::NotFound { .. }) => None,
        Err(error) => return Err(SecretLifecycleError::Store(error)),
    };
    store.put(id, value).map_err(SecretLifecycleError::Store)?;
    if let Err(source) = commit_reference(id) {
        let cleanup = if let Some(previous) = previous {
            SecretInput::from_bytes(previous.expose_secret().to_vec())
                .and_then(|previous| store.put(id, &previous))
                .err()
        } else {
            store.delete(id).err()
        };
        return Err(SecretLifecycleError::ReferenceCommit { source, cleanup });
    }
    Ok(())
}

pub fn delete_reference_then_secret<E, F>(
    store: &dyn SecretStore,
    id: &SecretId,
    delete_reference: F,
) -> Result<(), SecretLifecycleError<E>>
where
    E: std::error::Error + Send + Sync + 'static,
    F: FnOnce() -> Result<(), E>,
{
    delete_reference().map_err(SecretLifecycleError::ReferenceDelete)?;
    store.delete(id).map_err(SecretLifecycleError::Cleanup)
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Mutex};

    use super::*;

    #[derive(Debug, Error)]
    #[error("synthetic reference error")]
    struct SyntheticReferenceError;

    #[derive(Default)]
    struct FakeStore {
        values: Mutex<HashMap<SecretId, Vec<u8>>>,
        fail_delete: Mutex<bool>,
    }

    impl SecretStore for FakeStore {
        fn put(&self, id: &SecretId, value: &SecretInput) -> Result<(), SecretError> {
            self.values
                .lock()
                .expect("fake store lock")
                .insert(id.clone(), value.expose_secret().to_vec());
            Ok(())
        }

        fn get(&self, id: &SecretId) -> Result<SecretValue, SecretError> {
            let value = self
                .values
                .lock()
                .expect("fake store lock")
                .get(id)
                .cloned()
                .ok_or_else(|| SecretError::NotFound { id: id.clone() })?;
            SecretValue::from_bytes(value)
        }

        fn delete(&self, id: &SecretId) -> Result<(), SecretError> {
            if *self.fail_delete.lock().expect("fake store lock") {
                return Err(SecretError::Backend {
                    operation: "synthetic_delete",
                    code: 1,
                });
            }
            self.values.lock().expect("fake store lock").remove(id);
            Ok(())
        }

        fn contains(&self, id: &SecretId) -> Result<bool, SecretError> {
            Ok(self
                .values
                .lock()
                .expect("fake store lock")
                .contains_key(id))
        }
    }

    fn test_id() -> SecretId {
        SecretId::new("Cakify/provider/synthetic/api-key").expect("secret id")
    }

    #[test]
    fn secret_id_and_value_reject_unsafe_shapes() {
        assert_eq!(
            SecretId::new("plain-secret").err(),
            Some(SecretError::InvalidId)
        );
        assert_eq!(
            SecretId::new("Cakify/provider/synthetic/not-api-key").err(),
            Some(SecretError::InvalidId)
        );
        assert_eq!(
            SecretInput::from_bytes(Vec::new()).err(),
            Some(SecretError::InvalidValue)
        );
        assert_eq!(
            SecretInput::from_bytes(vec![0_u8; MAX_SECRET_VALUE_BYTES + 1]).err(),
            Some(SecretError::InvalidValue)
        );
    }

    #[test]
    fn put_then_commit_compensates_when_reference_commit_fails() {
        let store = FakeStore::default();
        let id = test_id();
        let value = SecretInput::from_utf8("synthetic-value").expect("secret input");
        let result =
            put_then_commit_reference::<SyntheticReferenceError, _>(&store, &id, &value, |_| {
                Err(SyntheticReferenceError)
            });
        assert!(matches!(
            result,
            Err(SecretLifecycleError::ReferenceCommit { cleanup: None, .. })
        ));
        assert!(!store.contains(&id).expect("contains after compensation"));
    }

    #[test]
    fn failed_secret_cleanup_is_reported_as_retryable() {
        let store = FakeStore::default();
        *store.fail_delete.lock().expect("fake store lock") = true;
        let id = test_id();
        let value = SecretInput::from_utf8("synthetic-value").expect("secret input");
        let result =
            put_then_commit_reference::<SyntheticReferenceError, _>(&store, &id, &value, |_| {
                Err(SyntheticReferenceError)
            });
        assert!(matches!(
            result,
            Err(SecretLifecycleError::ReferenceCommit {
                cleanup: Some(SecretError::Backend { .. }),
                ..
            })
        ));
        assert!(store.contains(&id).expect("contains orphan secret"));
    }

    #[test]
    fn failed_reference_update_restores_the_previous_secret() {
        let store = FakeStore::default();
        let id = test_id();
        let previous = SecretInput::from_utf8("previous-synthetic-value").expect("previous");
        store.put(&id, &previous).expect("seed previous secret");
        let replacement =
            SecretInput::from_utf8("replacement-synthetic-value").expect("replacement");

        let result = put_then_commit_reference::<SyntheticReferenceError, _>(
            &store,
            &id,
            &replacement,
            |_| Err(SyntheticReferenceError),
        );

        assert!(matches!(
            result,
            Err(SecretLifecycleError::ReferenceCommit { cleanup: None, .. })
        ));
        let restored = store.get(&id).expect("restored previous secret");
        assert!(restored.expose_secret() == previous.expose_secret());
    }

    #[test]
    fn delete_reference_then_secret_preserves_retry_when_secret_delete_fails() {
        let store = FakeStore::default();
        let id = test_id();
        let value = SecretInput::from_utf8("synthetic-value").expect("secret input");
        store.put(&id, &value).expect("seed secret");
        let result =
            delete_reference_then_secret::<SyntheticReferenceError, _>(&store, &id, || Ok(()));
        assert!(result.is_ok());
        assert!(!store.contains(&id).expect("contains after delete"));

        *store.fail_delete.lock().expect("fake store lock") = true;
        store.put(&id, &value).expect("reseed secret");
        let result =
            delete_reference_then_secret::<SyntheticReferenceError, _>(&store, &id, || Ok(()));
        assert!(matches!(result, Err(SecretLifecycleError::Cleanup(_))));
    }

    #[test]
    fn reference_delete_failure_does_not_touch_secret() {
        let store = FakeStore::default();
        let id = test_id();
        let value = SecretInput::from_utf8("synthetic-value").expect("secret input");
        store.put(&id, &value).expect("seed secret");
        let result =
            delete_reference_then_secret::<SyntheticReferenceError, _>(&store, &id, || {
                Err(SyntheticReferenceError)
            });
        assert!(matches!(
            result,
            Err(SecretLifecycleError::ReferenceDelete(_))
        ));
        assert!(store
            .contains(&id)
            .expect("secret remains after db failure"));
    }
}
