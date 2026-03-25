use secret_resolvers::{KeychainResolver, SecretResolver, ResolveRequest};
use std::collections::HashMap;

#[tokio::test]
async fn test_keychain_resolves_stored_secret() {
    let entry = keyring::Entry::new("dotvault-test", "test-key").unwrap();
    entry.set_password("my-secret-value").unwrap();

    let resolver = KeychainResolver::new(HashMap::from([
        ("service".to_string(), toml::Value::String("dotvault-test".to_string())),
    ])).unwrap();

    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("test-key".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value, "my-secret-value");

    entry.delete_credential().unwrap();
}

#[tokio::test]
async fn test_keychain_errors_on_missing_secret() {
    let resolver = KeychainResolver::new(HashMap::from([
        ("service".to_string(), toml::Value::String("dotvault-test-nonexistent".to_string())),
    ])).unwrap();

    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("does-not-exist".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
}
