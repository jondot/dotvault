use secret_resolvers::{AwsResolver, SecretResolver, ResolveRequest};
use std::collections::HashMap;

/// Returns true if LocalStack is reachable.
async fn localstack_available() -> bool {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap();
    client
        .get("http://localhost:4566/_localstack/health")
        .send()
        .await
        .is_ok()
}

fn localstack_config() -> HashMap<String, toml::Value> {
    HashMap::from([
        ("region".to_string(), toml::Value::String("us-east-1".to_string())),
        ("endpoint_url".to_string(), toml::Value::String("http://localhost:4566".to_string())),
    ])
}

#[tokio::test]
async fn test_aws_sm_resolve_secret() {
    if !localstack_available().await {
        eprintln!("SKIP: LocalStack not running");
        return;
    }

    // Seed a secret via the AWS SDK
    let config = localstack_config();
    let resolver = AwsResolver::new(config.clone()).await.unwrap();

    // Create the secret in LocalStack using SDK
    use aws_config::BehaviorVersion;
    let loader = aws_config::defaults(BehaviorVersion::latest())
        .region(aws_config::Region::new("us-east-1"))
        .endpoint_url("http://localhost:4566");
    let sdk_config = loader.load().await;
    let sm = aws_sdk_secretsmanager::Client::new(&sdk_config);

    let _ = sm
        .create_secret()
        .name("test/plain-secret")
        .secret_string("my-secret-value")
        .send()
        .await;

    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("sm://test/plain-secret".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value, "my-secret-value");
}

#[tokio::test]
async fn test_aws_sm_resolve_json_field() {
    if !localstack_available().await {
        eprintln!("SKIP: LocalStack not running");
        return;
    }

    use aws_config::BehaviorVersion;
    let loader = aws_config::defaults(BehaviorVersion::latest())
        .region(aws_config::Region::new("us-east-1"))
        .endpoint_url("http://localhost:4566");
    let sdk_config = loader.load().await;
    let sm = aws_sdk_secretsmanager::Client::new(&sdk_config);

    let _ = sm
        .create_secret()
        .name("test/json-secret")
        .secret_string(r#"{"username":"admin","password":"s3cr3t"}"#)
        .send()
        .await;

    let config = localstack_config();
    let resolver = AwsResolver::new(config).await.unwrap();

    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("sm://test/json-secret".to_string())),
            ("field".to_string(), toml::Value::String("password".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value, "s3cr3t");
}

#[tokio::test]
async fn test_aws_ssm_resolve_parameter() {
    if !localstack_available().await {
        eprintln!("SKIP: LocalStack not running");
        return;
    }

    use aws_config::BehaviorVersion;
    let loader = aws_config::defaults(BehaviorVersion::latest())
        .region(aws_config::Region::new("us-east-1"))
        .endpoint_url("http://localhost:4566");
    let sdk_config = loader.load().await;
    let ssm = aws_sdk_ssm::Client::new(&sdk_config);

    let _ = ssm
        .put_parameter()
        .name("/myapp/api-key")
        .value("ssm-param-value")
        .r#type(aws_sdk_ssm::types::ParameterType::SecureString)
        .send()
        .await;

    let config = localstack_config();
    let resolver = AwsResolver::new(config).await.unwrap();

    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("ssm:///myapp/api-key".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value, "ssm-param-value");
}

#[tokio::test]
async fn test_aws_invalid_prefix_errors() {
    // This test doesn't need LocalStack — it just checks local validation
    use aws_config::BehaviorVersion;
    // We build the resolver but skip actual resolution if LocalStack is unavailable
    // The prefix check is synchronous logic, so we use a dummy config
    let config = HashMap::from([
        ("region".to_string(), toml::Value::String("us-east-1".to_string())),
        ("endpoint_url".to_string(), toml::Value::String("http://localhost:4566".to_string())),
    ]);
    let resolver = AwsResolver::new(config).await.unwrap();
    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("unknown://something".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("unknown ref prefix") || err.contains("sm://") || err.contains("ssm://"));
}
