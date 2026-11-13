use std::cmp::Reverse;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, RwLock};

use secrecy::{ExposeSecret, SecretString};

use crate::{Error, Result};

pub const REDACTED: &str = "[REDACTED]";

pub struct SecretValue(SecretString);

impl SecretValue {
    pub fn new(value: impl Into<String>) -> Self {
        Self(SecretString::from(value.into()))
    }

    pub(crate) fn expose(&self) -> &str {
        self.0.expose_secret()
    }

    pub fn len(&self) -> usize {
        self.expose().len()
    }

    pub fn is_empty(&self) -> bool {
        self.expose().is_empty()
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

impl fmt::Display for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

pub trait SecretStore: Send + Sync {
    fn set(&self, service: &str, account: &str, secret: &SecretValue) -> Result<()>;
    fn get(&self, service: &str, account: &str) -> Result<SecretValue>;
    fn delete(&self, service: &str, account: &str) -> Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyringBackend {
    SecretService,
    Keyutils,
}

pub struct OsKeyringStore {
    store: Arc<keyring_core::CredentialStore>,
    backend: KeyringBackend,
}

impl OsKeyringStore {
    pub fn new() -> Result<Self> {
        if let Ok(store) = dbus_secret_service_keyring_store::Store::new() {
            return Ok(Self {
                store,
                backend: KeyringBackend::SecretService,
            });
        }
        let store = linux_keyutils_keyring_store::Store::new().map_err(Error::Keychain)?;
        Ok(Self {
            store,
            backend: KeyringBackend::Keyutils,
        })
    }

    pub fn backend(&self) -> KeyringBackend {
        self.backend
    }
}

impl SecretStore for OsKeyringStore {
    fn set(&self, service: &str, account: &str, secret: &SecretValue) -> Result<()> {
        let entry = self
            .store
            .build(service, account, None)
            .map_err(Error::Keychain)?;
        entry.set_password(secret.expose()).map_err(Error::Keychain)
    }

    fn get(&self, service: &str, account: &str) -> Result<SecretValue> {
        let entry = self
            .store
            .build(service, account, None)
            .map_err(Error::Keychain)?;
        let password = entry.get_password().map_err(Error::Keychain)?;
        Ok(SecretValue::new(password))
    }

    fn delete(&self, service: &str, account: &str) -> Result<()> {
        let entry = self
            .store
            .build(service, account, None)
            .map_err(Error::Keychain)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(err) => Err(Error::Keychain(err)),
        }
    }
}

#[derive(Default)]
pub struct InMemorySecretStore {
    entries: Mutex<HashMap<(String, String), SecretValue>>,
}

impl InMemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for InMemorySecretStore {
    fn set(&self, service: &str, account: &str, secret: &SecretValue) -> Result<()> {
        let mut guard = self.entries.lock().map_err(|_| Error::Security {
            message: "InMemorySecretStore lock poisoned".into(),
        })?;
        guard.insert(
            (service.to_owned(), account.to_owned()),
            SecretValue::new(secret.expose()),
        );
        Ok(())
    }

    fn get(&self, service: &str, account: &str) -> Result<SecretValue> {
        let guard = self.entries.lock().map_err(|_| Error::Security {
            message: "InMemorySecretStore lock poisoned".into(),
        })?;
        match guard.get(&(service.to_owned(), account.to_owned())) {
            Some(secret) => Ok(SecretValue::new(secret.expose())),
            None => Err(Error::Keychain(keyring_core::Error::NoEntry)),
        }
    }

    fn delete(&self, service: &str, account: &str) -> Result<()> {
        let mut guard = self.entries.lock().map_err(|_| Error::Security {
            message: "InMemorySecretStore lock poisoned".into(),
        })?;
        guard.remove(&(service.to_owned(), account.to_owned()));
        Ok(())
    }
}

#[derive(Default)]
pub struct SecretRedactor {
    secrets: RwLock<Vec<SecretValue>>,
}

impl SecretRedactor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, secret: &SecretValue) -> Result<()> {
        if secret.len() < 8 {
            return Ok(());
        }
        let mut guard = self.secrets.write().map_err(|_| Error::Security {
            message: "SecretRedactor lock poisoned".into(),
        })?;
        guard.push(SecretValue::new(secret.expose()));
        guard.sort_by_key(|s| Reverse(s.len()));
        Ok(())
    }

    pub fn redact(&self, text: &str) -> Result<String> {
        let mut out = {
            let guard = self.secrets.read().map_err(|_| Error::Security {
                message: "SecretRedactor lock poisoned".into(),
            })?;
            let mut buf = text.to_owned();
            for secret in guard.iter() {
                buf = buf.replace(secret.expose(), REDACTED);
            }
            buf
        };

        out = redact_prefixed(&out, "Bearer ", 1, |c| !c.is_whitespace());
        out = redact_prefixed(&out, "sk-", 8, |c| {
            c.is_ascii_alphanumeric() || c == '_' || c == '-'
        });
        out = redact_prefixed(&out, "AIza", 20, |c| {
            c.is_ascii_alphanumeric() || c == '_' || c == '-'
        });
        out = redact_prefixed(&out, "ghp_", 20, |c| c.is_ascii_alphanumeric());

        Ok(out)
    }
}

fn redact_prefixed(text: &str, prefix: &str, min_len: usize, allowed: fn(char) -> bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut remaining = text;

    while let Some(idx) = remaining.find(prefix) {
        out.push_str(&remaining[..idx]);
        let after_prefix = &remaining[idx + prefix.len()..];

        let mut run_chars = 0usize;
        let mut run_bytes = 0usize;
        for ch in after_prefix.chars() {
            if allowed(ch) {
                run_chars += 1;
                run_bytes += ch.len_utf8();
            } else {
                break;
            }
        }

        if run_chars >= min_len {
            out.push_str(REDACTED);
        } else {
            out.push_str(prefix);
            out.push_str(&after_prefix[..run_bytes]);
        }

        remaining = &after_prefix[run_bytes..];
    }

    out.push_str(remaining);
    out
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;

    use super::{
        InMemorySecretStore, KeyringBackend, OsKeyringStore, REDACTED, SecretRedactor, SecretStore,
        SecretValue,
    };
    use crate::Error;

    #[test]
    fn secret_value_debug_and_display_are_redacted() {
        let raw = "top-secret-api-key-98765";
        let secret = SecretValue::new(raw);
        let debug_str = format!("{secret:?}");
        let display_str = format!("{secret}");

        assert_eq!(debug_str, REDACTED);
        assert_eq!(display_str, REDACTED);
        assert!(!debug_str.contains(raw));
        assert!(!display_str.contains(raw));
    }

    #[test]
    fn secret_value_len_and_expose() {
        let raw = "my-secret-token";
        let secret = SecretValue::new(raw);
        assert_eq!(secret.len(), raw.len());
        assert!(!secret.is_empty());
        assert_eq!(secret.expose(), raw);

        let empty = SecretValue::new("");
        assert_eq!(empty.len(), 0);
        assert!(empty.is_empty());
        assert_eq!(empty.expose(), "");
    }

    #[test]
    fn in_memory_set_get_delete() {
        let store = InMemorySecretStore::new();
        let secret = SecretValue::new("round-trip-secret");

        store
            .set("orrery.agent.test", "testuser", &secret)
            .expect("set should succeed");

        let loaded = store
            .get("orrery.agent.test", "testuser")
            .expect("get should succeed");
        assert_eq!(loaded.expose(), "round-trip-secret");

        store
            .delete("orrery.agent.test", "testuser")
            .expect("first delete should succeed");

        let missing = store.get("orrery.agent.test", "testuser");
        assert!(matches!(
            missing,
            Err(Error::Keychain(keyring_core::Error::NoEntry))
        ));

        store
            .delete("orrery.agent.test", "testuser")
            .expect("second delete on missing entry should succeed");
    }

    #[test]
    fn in_memory_missing_is_keychain_no_entry() {
        let store = InMemorySecretStore::default();
        let res = store.get("orrery.unknown.service", "testuser");
        assert!(matches!(
            res,
            Err(Error::Keychain(keyring_core::Error::NoEntry))
        ));
    }

    #[test]
    fn in_memory_entries_are_keyed_by_service_and_account() {
        let store = InMemorySecretStore::default();
        let first = SecretValue::new("secret-alpha");
        let second = SecretValue::new("secret-beta");

        store
            .set("orrery.service.one", "testuser", &first)
            .expect("set service one");
        store
            .set("orrery.service.two", "testuser", &second)
            .expect("set service two");

        let got_one = store
            .get("orrery.service.one", "testuser")
            .expect("get service one");
        let got_two = store
            .get("orrery.service.two", "testuser")
            .expect("get service two");

        assert_eq!(got_one.expose(), "secret-alpha");
        assert_eq!(got_two.expose(), "secret-beta");
    }

    #[test]
    fn keychain_error_preserves_source() {
        let err = Error::Keychain(keyring_core::Error::NoEntry);
        let src = err.source();
        assert!(src.is_some());
        assert_eq!(err.to_string(), "keychain error");
    }

    #[test]
    fn os_keyring_store_constructed_from_keyutils_exercises_store_ops() {
        let keyutils_store =
            linux_keyutils_keyring_store::Store::new().expect("keyutils store should construct");
        let os_store = OsKeyringStore {
            store: keyutils_store,
            backend: KeyringBackend::Keyutils,
        };
        assert_eq!(os_store.backend(), KeyringBackend::Keyutils);

        let service = "orrery.unit.keyutils.test";
        let account = "testuser";
        let secret = SecretValue::new("unit-keyutils-secret-42");

        os_store
            .set(service, account, &secret)
            .expect("keyutils set should succeed");
        let fetched = os_store
            .get(service, account)
            .expect("keyutils get should succeed");
        assert_eq!(fetched.expose(), "unit-keyutils-secret-42");

        os_store
            .delete(service, account)
            .expect("keyutils delete should succeed");
        os_store
            .delete(service, account)
            .expect("keyutils idempotent delete should succeed");

        let missing = os_store.get(service, account);
        assert!(matches!(
            missing,
            Err(Error::Keychain(keyring_core::Error::NoEntry))
        ));
    }

    #[test]
    fn redacts_registered_secret() {
        let redactor = SecretRedactor::new();
        redactor
            .register(&SecretValue::new("s3cr3t-value-xyz"))
            .expect("register secret");

        let redacted = redactor
            .redact("token=s3cr3t-value-xyz;")
            .expect("redact should succeed");
        assert_eq!(redacted, "token=[REDACTED];");
    }

    #[test]
    fn short_registered_secret_is_ignored() {
        let redactor = SecretRedactor::new();
        redactor
            .register(&SecretValue::new("abc1234"))
            .expect("register short secret");

        let input = "value is abc1234 in text";
        let redacted = redactor.redact(input).expect("redact should succeed");
        assert_eq!(redacted, input);
    }

    #[test]
    fn longest_registered_secret_wins() {
        let redactor = SecretRedactor::new();
        redactor
            .register(&SecretValue::new("abcdefgh"))
            .expect("register shorter secret");
        redactor
            .register(&SecretValue::new("abcdefgh-ijkl"))
            .expect("register longer secret");

        let redacted = redactor
            .redact("abcdefgh-ijkl")
            .expect("redact should succeed");
        assert_eq!(redacted, REDACTED);
    }

    #[test]
    fn redacts_bearer_token() {
        let redactor = SecretRedactor::new();
        let redacted = redactor
            .redact("Authorization: Bearer abc.def")
            .expect("redact should succeed");
        assert_eq!(redacted, "Authorization: [REDACTED]");
    }

    #[test]
    fn redacts_sk_key_at_min_length() {
        let redactor = SecretRedactor::new();
        let at_min = redactor
            .redact("sk-ABCDEFGH")
            .expect("redact should succeed");
        assert_eq!(at_min, REDACTED);

        let below_min = redactor
            .redact("sk-ABCDEFG")
            .expect("redact should succeed");
        assert_eq!(below_min, "sk-ABCDEFG");
    }

    #[test]
    fn redacts_google_key() {
        let redactor = SecretRedactor::new();
        let twenty_a = format!("AIza{}", "A".repeat(20));
        let redacted_20 = redactor.redact(&twenty_a).expect("redact should succeed");
        assert_eq!(redacted_20, REDACTED);

        let nineteen_a = format!("AIza{}", "A".repeat(19));
        let redacted_19 = redactor.redact(&nineteen_a).expect("redact should succeed");
        assert_eq!(redacted_19, nineteen_a);
    }

    #[test]
    fn redacts_github_token() {
        let redactor = SecretRedactor::new();
        let valid_ghp = format!("ghp_{}", "a".repeat(20));
        let redacted_valid = redactor.redact(&valid_ghp).expect("redact should succeed");
        assert_eq!(redacted_valid, REDACTED);

        let dashes_ghp = format!("ghp_{}", "-".repeat(20));
        let redacted_dashes = redactor.redact(&dashes_ghp).expect("redact should succeed");
        assert_eq!(redacted_dashes, dashes_ghp);
    }

    #[test]
    fn multiple_matches_in_one_line() {
        let redactor = SecretRedactor::new();
        let line = "header Authorization: Bearer tok.123 and key=sk-12345678_abcd!";
        let redacted = redactor.redact(line).expect("redact should succeed");
        assert_eq!(
            redacted,
            "header Authorization: [REDACTED] and key=[REDACTED]!"
        );
    }

    #[test]
    fn plain_text_unchanged() {
        let redactor = SecretRedactor::new();
        let input = "cargo test passed in 3.2s";
        let redacted = redactor.redact(input).expect("redact should succeed");
        assert_eq!(redacted, input);
    }
}
