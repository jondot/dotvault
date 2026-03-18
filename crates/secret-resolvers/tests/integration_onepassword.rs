use secret_resolvers::{OnePasswordResolver, SecretResolver, ResolveRequest};
use std::collections::HashMap;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;

fn create_fake_op(response_json: &str) -> tempfile::NamedTempFile {
    let mut f = tempfile::Builder::new().suffix(".sh").tempfile().unwrap();
    writeln!(f, "#!/bin/bash").unwrap();
    writeln!(f, "echo '{}'", response_json).unwrap();
    let path = f.path().to_owned();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    f
}

fn create_failing_op() -> tempfile::NamedTempFile {
    let mut f = tempfile::Builder::new().suffix(".sh").tempfile().unwrap();
    writeln!(f, "#!/bin/bash").unwrap();
    writeln!(f, "echo 'item not found' >&2").unwrap();
    writeln!(f, "exit 1").unwrap();
    let path = f.path().to_owned();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    f
}

#[tokio::test]
async fn test_onepassword_resolves_secret() {
    let fake_op = create_fake_op(r#"{"value": "sk-test-12345"}"#);
    let config = HashMap::from([
        ("op_path".to_string(), toml::Value::String(fake_op.path().to_str().unwrap().to_string())),
    ]);
    let resolver = OnePasswordResolver::new(config).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("op://Engineering/OpenAI/api-key".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value, "sk-test-12345");
}

#[tokio::test]
async fn test_onepassword_errors_on_failed_op() {
    let fake_op = create_failing_op();
    let config = HashMap::from([
        ("op_path".to_string(), toml::Value::String(fake_op.path().to_str().unwrap().to_string())),
    ]);
    let resolver = OnePasswordResolver::new(config).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("op://Vault/Item/field".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
}
