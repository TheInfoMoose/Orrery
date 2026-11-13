use orrery_core::Error;
use orrery_core::security::{OsKeyringStore, SecretStore, SecretValue};

#[test]
fn os_keyring_round_trip() {
    let store = OsKeyringStore::new().expect("OS keyring store should initialize");
    eprintln!("OsKeyringStore backend: {:?}", store.backend());

    let service = "orrery.test";
    let account = "testuser";
    let secret = SecretValue::new("live-test-value-123");

    store
        .set(service, account, &secret)
        .expect("set should succeed");

    let fetched = store.get(service, account).expect("get should succeed");
    assert_eq!(fetched.len(), "live-test-value-123".len());
    assert!(!fetched.is_empty());

    store
        .delete(service, account)
        .expect("delete should succeed");

    let after_delete = store.get(service, account);
    assert!(
        matches!(after_delete, Err(Error::Keychain(_))),
        "expected Error::Keychain after delete, got {after_delete:?}"
    );
}
