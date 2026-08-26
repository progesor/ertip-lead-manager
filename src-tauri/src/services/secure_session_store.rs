use std::{fmt, sync::Mutex};

use keyring::v1::{Entry, Error as KeyringError};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

const SERVICE_NAME: &str = "com.ertipmedical.lead-manager";
const ACCOUNT_NAME: &str = "tauri-api-session";
const MAX_TOKEN_BYTES: usize = 4096;

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct StoredSessionToken(String);

impl StoredSessionToken {
    pub fn new(raw: String) -> Result<Self, SecureSessionStoreError> {
        validate_token(&raw)?;
        Ok(Self(raw))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for StoredSessionToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("StoredSessionToken([REDACTED])")
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SecureSessionStoreError {
    #[error("session token is invalid")]
    InvalidToken,
    #[error("native credential store is unavailable")]
    Unavailable,
    #[error("native credential operation failed")]
    BackendFailure,
    #[error("stored session token is not valid UTF-8")]
    CorruptSecret,
}

pub(crate) trait SecretBackend: Send + Sync {
    fn availability(&self) -> Result<(), SecureSessionStoreError>;
    fn set(&self, secret: &[u8]) -> Result<(), SecureSessionStoreError>;
    fn get(&self) -> Result<Option<Vec<u8>>, SecureSessionStoreError>;
    fn delete(&self) -> Result<(), SecureSessionStoreError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NativeKeyringBackend;

impl NativeKeyringBackend {
    fn entry(&self) -> Result<Entry, SecureSessionStoreError> {
        Entry::new(SERVICE_NAME, ACCOUNT_NAME).map_err(map_keyring_error)
    }
}

impl SecretBackend for NativeKeyringBackend {
    fn availability(&self) -> Result<(), SecureSessionStoreError> {
        Entry::store_status()
            .as_ref()
            .map(|_| ())
            .map_err(|_| SecureSessionStoreError::Unavailable)
    }

    fn set(&self, secret: &[u8]) -> Result<(), SecureSessionStoreError> {
        self.entry()?
            .set_secret(secret)
            .map_err(map_keyring_error)
    }

    fn get(&self) -> Result<Option<Vec<u8>>, SecureSessionStoreError> {
        match self.entry()?.get_secret() {
            Ok(secret) => Ok(Some(secret)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(map_keyring_error(error)),
        }
    }

    fn delete(&self) -> Result<(), SecureSessionStoreError> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(map_keyring_error(error)),
        }
    }
}

pub struct SecureSessionStore<B = NativeKeyringBackend> {
    backend: B,
    // Windows Credential Manager does not promise cross-thread operation ordering for
    // the same credential. Serialize this single logical session entry explicitly.
    operation_gate: Mutex<()>,
}

impl SecureSessionStore<NativeKeyringBackend> {
    pub fn native() -> Self {
        Self::new(NativeKeyringBackend)
    }
}

impl<B: SecretBackend> SecureSessionStore<B> {
    pub(crate) fn new(backend: B) -> Self {
        Self {
            backend,
            operation_gate: Mutex::new(()),
        }
    }

    pub fn availability(&self) -> Result<(), SecureSessionStoreError> {
        let _guard = self
            .operation_gate
            .lock()
            .map_err(|_| SecureSessionStoreError::BackendFailure)?;
        self.backend.availability()
    }

    pub fn store(&self, token: &StoredSessionToken) -> Result<(), SecureSessionStoreError> {
        validate_token(token.expose())?;
        let _guard = self
            .operation_gate
            .lock()
            .map_err(|_| SecureSessionStoreError::BackendFailure)?;
        self.backend.set(token.expose().as_bytes())
    }

    pub fn load(&self) -> Result<Option<StoredSessionToken>, SecureSessionStoreError> {
        let _guard = self
            .operation_gate
            .lock()
            .map_err(|_| SecureSessionStoreError::BackendFailure)?;
        let Some(secret) = self.backend.get()? else {
            return Ok(None);
        };
        let token = String::from_utf8(secret).map_err(|_| SecureSessionStoreError::CorruptSecret)?;
        StoredSessionToken::new(token).map(Some)
    }

    pub fn clear(&self) -> Result<(), SecureSessionStoreError> {
        let _guard = self
            .operation_gate
            .lock()
            .map_err(|_| SecureSessionStoreError::BackendFailure)?;
        self.backend.delete()
    }
}

fn validate_token(token: &str) -> Result<(), SecureSessionStoreError> {
    let trimmed = token.trim();
    if trimmed.is_empty()
        || trimmed != token
        || token.len() > MAX_TOKEN_BYTES
        || token.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(SecureSessionStoreError::InvalidToken);
    }
    Ok(())
}

fn map_keyring_error(error: KeyringError) -> SecureSessionStoreError {
    match error {
        KeyringError::NoDefaultStore
        | KeyringError::NoStorageAccess(_)
        | KeyringError::NotSupportedByStore(_) => SecureSessionStoreError::Unavailable,
        _ => SecureSessionStoreError::BackendFailure,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::{
        SecretBackend, SecureSessionStore, SecureSessionStoreError, StoredSessionToken,
    };

    #[derive(Default)]
    struct MemoryBackend {
        secret: Mutex<Option<Vec<u8>>>,
    }

    impl SecretBackend for MemoryBackend {
        fn availability(&self) -> Result<(), SecureSessionStoreError> {
            Ok(())
        }

        fn set(&self, secret: &[u8]) -> Result<(), SecureSessionStoreError> {
            *self.secret.lock().expect("memory backend lock") = Some(secret.to_vec());
            Ok(())
        }

        fn get(&self) -> Result<Option<Vec<u8>>, SecureSessionStoreError> {
            Ok(self.secret.lock().expect("memory backend lock").clone())
        }

        fn delete(&self) -> Result<(), SecureSessionStoreError> {
            *self.secret.lock().expect("memory backend lock") = None;
            Ok(())
        }
    }

    #[test]
    fn token_debug_is_redacted_and_validation_rejects_unsafe_values() {
        let token = StoredSessionToken::new("abc.def".to_string()).expect("valid token");
        assert_eq!(format!("{token:?}"), "StoredSessionToken([REDACTED])");
        assert!(!format!("{token:?}").contains("abc.def"));

        assert!(matches!(
            StoredSessionToken::new(" token".to_string()),
            Err(SecureSessionStoreError::InvalidToken)
        ));
        assert!(matches!(
            StoredSessionToken::new("token\n".to_string()),
            Err(SecureSessionStoreError::InvalidToken)
        ));
        assert!(matches!(
            StoredSessionToken::new(String::new()),
            Err(SecureSessionStoreError::InvalidToken)
        ));
    }

    #[test]
    fn store_load_and_clear_never_require_plaintext_persistence() {
        let store = SecureSessionStore::new(MemoryBackend::default());
        store.availability().expect("store available");

        assert!(store.load().expect("empty load").is_none());

        let token = StoredSessionToken::new("session-token-value".to_string()).expect("token");
        store.store(&token).expect("store token");

        let loaded = store.load().expect("load token").expect("token exists");
        assert_eq!(loaded.expose(), "session-token-value");

        store.clear().expect("clear token");
        assert!(store.load().expect("load after clear").is_none());
    }
}
