#![cfg(feature = "onepassword")]

use secret_resolvers::{OnePasswordResolver, SecretResolver, ResolveRequest, ExposeSecret};
use std::collections::HashMap;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;

/// Create a fake `op` script that prints the given JSON.
/// Returns a TempDir (to keep the file alive) and the path to the script.
fn create_fake_op(response_json: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let script_path = dir.path().join("fake-op.sh");
    let mut f = std::fs::File::create(&script_path).unwrap();
    writeln!(f, "#!/bin/bash").unwrap();
    writeln!(f, "echo '{}'", response_json).unwrap();
    drop(f); // close the fd before chmod+exec
    std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();
    (dir, script_path)
}

fn create_failing_op() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let script_path = dir.path().join("fake-op.sh");
    let mut f = std::fs::File::create(&script_path).unwrap();
    writeln!(f, "#!/bin/bash").unwrap();
    writeln!(f, "echo 'item not found' >&2").unwrap();
    writeln!(f, "exit 1").unwrap();
    drop(f);
    std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();
    (dir, script_path)
}

#[tokio::test]
async fn test_onepassword_resolves_secret() {
    let (_dir, script_path) = create_fake_op(r#"{"value": "sk-test-12345"}"#);
    let config = HashMap::from([
        ("op_path".to_string(), toml::Value::String(script_path.to_str().unwrap().to_string())),
    ]);
    let resolver = OnePasswordResolver::new(config).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("op://Engineering/OpenAI/api-key".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value.expose_secret(), "sk-test-12345");
}

#[tokio::test]
async fn test_onepassword_errors_on_failed_op() {
    let (_dir, script_path) = create_failing_op();
    let config = HashMap::from([
        ("op_path".to_string(), toml::Value::String(script_path.to_str().unwrap().to_string())),
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
