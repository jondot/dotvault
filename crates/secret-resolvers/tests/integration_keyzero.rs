use secret_resolvers::{KeyzeroResolver, ResolveRequest, SecretResolver, ExposeSecret};
use std::collections::HashMap;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn keyzero_config(endpoint: &str) -> HashMap<String, toml::Value> {
    HashMap::from([
        ("endpoint".to_string(), toml::Value::String(endpoint.to_string())),
        ("token".to_string(), toml::Value::String("test-bearer-token".to_string())),
    ])
}

fn keyzero_config_no_token(endpoint: &str) -> HashMap<String, toml::Value> {
    HashMap::from([("endpoint".to_string(), toml::Value::String(endpoint.to_string()))])
}

#[tokio::test]
async fn test_keyzero_resolve_first_secret() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/resolve"))
        .and(header("Authorization", "Bearer test-bearer-token"))
        .and(body_json(serde_json::json!({ "resource": "my-resource-id" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "allowed": true,
            "secrets": {
                "db-password": {
                    "value": "super-secret-db-pass",
                    "ttl": 3600
                }
            }
        })))
        .mount(&mock_server)
        .await;

    let resolver = KeyzeroResolver::new(keyzero_config(&mock_server.uri())).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([(
            "ref".to_string(),
            toml::Value::String("my-resource-id".to_string()),
        )]),
    };

    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value.expose_secret(), "super-secret-db-pass");
    assert_eq!(result.ttl, Some(3600));
}

#[tokio::test]
async fn test_keyzero_resolve_named_secret() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/resolve"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "allowed": true,
            "secrets": {
                "api-key": {
                    "value": "api-key-value",
                    "ttl": 7200
                },
                "db-password": {
                    "value": "db-pass-value",
                    "ttl": 7200
                }
            }
        })))
        .mount(&mock_server)
        .await;

    let resolver = KeyzeroResolver::new(keyzero_config(&mock_server.uri())).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("my-resource".to_string())),
            ("secret_name".to_string(), toml::Value::String("api-key".to_string())),
        ]),
    };

    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value.expose_secret(), "api-key-value");
    assert_eq!(result.ttl, Some(7200));
}

#[tokio::test]
async fn test_keyzero_denied_access_errors() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/resolve"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "allowed": false,
            "secrets": {}
        })))
        .mount(&mock_server)
        .await;

    let resolver = KeyzeroResolver::new(keyzero_config(&mock_server.uri())).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([(
            "ref".to_string(),
            toml::Value::String("restricted-resource".to_string()),
        )]),
    };

    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("denied") || err.contains("allowed"));
}

#[tokio::test]
async fn test_keyzero_missing_secret_name_errors() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/resolve"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "allowed": true,
            "secrets": {
                "existing-key": {
                    "value": "some-value",
                    "ttl": 3600
                }
            }
        })))
        .mount(&mock_server)
        .await;

    let resolver = KeyzeroResolver::new(keyzero_config(&mock_server.uri())).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("my-resource".to_string())),
            ("secret_name".to_string(), toml::Value::String("nonexistent-key".to_string())),
        ]),
    };

    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_keyzero_http_error_propagates() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/resolve"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
        .mount(&mock_server)
        .await;

    let resolver = KeyzeroResolver::new(keyzero_config(&mock_server.uri())).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([(
            "ref".to_string(),
            toml::Value::String("my-resource".to_string()),
        )]),
    };

    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("403"));
}

#[tokio::test]
async fn test_keyzero_missing_endpoint_errors() {
    let config: HashMap<String, toml::Value> = HashMap::new();
    let result = KeyzeroResolver::new(config);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_keyzero_no_token_sends_no_auth() {
    let mock_server = MockServer::start().await;

    // No Authorization header matcher — just check it works without token
    Mock::given(method("POST"))
        .and(path("/v1/resolve"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "allowed": true,
            "secrets": {
                "my-key": {
                    "value": "no-auth-value",
                    "ttl": null
                }
            }
        })))
        .mount(&mock_server)
        .await;

    let resolver = KeyzeroResolver::new(keyzero_config_no_token(&mock_server.uri())).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([(
            "ref".to_string(),
            toml::Value::String("my-resource".to_string()),
        )]),
    };

    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value.expose_secret(), "no-auth-value");
    assert_eq!(result.ttl, None);
}
