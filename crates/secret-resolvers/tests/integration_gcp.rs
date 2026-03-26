use base64::Engine;
use secret_resolvers::{GcpResolver, ResolveRequest, SecretResolver, ExposeSecret};
use std::collections::HashMap;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn gcp_b64(value: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(value.as_bytes())
}

#[tokio::test]
async fn test_gcp_resolve_secret_short_name() {
    let mock_server = MockServer::start().await;

    let encoded = gcp_b64("my-gcp-secret-value");
    let response_body = serde_json::json!({
        "name": "projects/my-project/secrets/api-key/versions/latest",
        "payload": {
            "data": encoded
        }
    });

    Mock::given(method("GET"))
        .and(path(
            "/v1/projects/my-project/secrets/api-key/versions/latest:access",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
        .mount(&mock_server)
        .await;

    let config = HashMap::from([
        ("project".to_string(), toml::Value::String("my-project".to_string())),
        (
            "endpoint_url".to_string(),
            toml::Value::String(mock_server.uri()),
        ),
    ]);

    let resolver = GcpResolver::new(config).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([(
            "ref".to_string(),
            toml::Value::String("api-key".to_string()),
        )]),
    };

    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value.expose_secret(), "my-gcp-secret-value");
}

#[tokio::test]
async fn test_gcp_resolve_secret_with_version() {
    let mock_server = MockServer::start().await;

    let encoded = gcp_b64("versioned-secret-value");
    let response_body = serde_json::json!({
        "name": "projects/my-project/secrets/db-password/versions/3",
        "payload": {
            "data": encoded
        }
    });

    Mock::given(method("GET"))
        .and(path(
            "/v1/projects/my-project/secrets/db-password/versions/3:access",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
        .mount(&mock_server)
        .await;

    let config = HashMap::from([
        ("project".to_string(), toml::Value::String("my-project".to_string())),
        (
            "endpoint_url".to_string(),
            toml::Value::String(mock_server.uri()),
        ),
    ]);

    let resolver = GcpResolver::new(config).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("db-password".to_string())),
            ("version".to_string(), toml::Value::String("3".to_string())),
        ]),
    };

    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value.expose_secret(), "versioned-secret-value");
}

#[tokio::test]
async fn test_gcp_resolve_full_resource_name() {
    let mock_server = MockServer::start().await;

    let encoded = gcp_b64("full-resource-secret");
    let response_body = serde_json::json!({
        "payload": {
            "data": encoded
        }
    });

    Mock::given(method("GET"))
        .and(path(
            "/v1/projects/other-project/secrets/my-secret/versions/2:access",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
        .mount(&mock_server)
        .await;

    let config = HashMap::from([(
        "endpoint_url".to_string(),
        toml::Value::String(mock_server.uri()),
    )]);

    let resolver = GcpResolver::new(config).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([
            (
                "ref".to_string(),
                toml::Value::String(
                    "projects/other-project/secrets/my-secret/versions/2".to_string(),
                ),
            ),
        ]),
    };

    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value.expose_secret(), "full-resource-secret");
}

#[tokio::test]
async fn test_gcp_server_error_propagates() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(
            "/v1/projects/my-project/secrets/missing-secret/versions/latest:access",
        ))
        .respond_with(ResponseTemplate::new(404).set_body_string(
            r#"{"error":{"code":404,"message":"Secret not found"}}"#,
        ))
        .mount(&mock_server)
        .await;

    let config = HashMap::from([
        ("project".to_string(), toml::Value::String("my-project".to_string())),
        (
            "endpoint_url".to_string(),
            toml::Value::String(mock_server.uri()),
        ),
    ]);

    let resolver = GcpResolver::new(config).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([(
            "ref".to_string(),
            toml::Value::String("missing-secret".to_string()),
        )]),
    };

    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("404"));
}

#[tokio::test]
async fn test_gcp_missing_project_for_short_name_errors() {
    // No project in config, no GOOGLE_PROJECT env — using short name should fail at config time
    let config: HashMap<String, toml::Value> = HashMap::new();
    let resolver = GcpResolver::new(config).unwrap();
    // Resolution should fail because project is missing
    let request = ResolveRequest {
        params: HashMap::from([(
            "ref".to_string(),
            toml::Value::String("some-secret".to_string()),
        )]),
    };
    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
}
