use secret_resolvers::{EnvResolver, SecretResolver, ResolveRequest, ExposeSecret};
use std::collections::HashMap;

#[tokio::test]
async fn test_env_resolves_existing_var() {
    std::env::set_var("DOTVAULT_TEST_SECRET", "hello-world");
    let resolver = EnvResolver::new(HashMap::new()).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([("ref".to_string(), toml::Value::String("DOTVAULT_TEST_SECRET".to_string()))]),
    };
    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value.expose_secret(), "hello-world");
    assert_eq!(result.ttl, None);
}

#[tokio::test]
async fn test_env_errors_on_missing_var() {
    std::env::remove_var("DOTVAULT_NONEXISTENT_VAR");
    let resolver = EnvResolver::new(HashMap::new()).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([("ref".to_string(), toml::Value::String("DOTVAULT_NONEXISTENT_VAR".to_string()))]),
    };
    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_env_errors_on_missing_ref_param() {
    let resolver = EnvResolver::new(HashMap::new()).unwrap();
    let request = ResolveRequest {
        params: HashMap::new(),
    };
    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
}
