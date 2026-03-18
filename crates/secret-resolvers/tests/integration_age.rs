use secret_resolvers::{AgeResolver, SecretResolver, ResolveRequest};
use std::collections::HashMap;
use std::io::Write;
use age::secrecy::ExposeSecret;

fn setup_age_encrypted_file(plaintext: &str) -> (tempfile::NamedTempFile, tempfile::NamedTempFile) {
    let identity = age::x25519::Identity::generate();
    let recipient = identity.to_public();

    let mut identity_file = tempfile::NamedTempFile::new().unwrap();
    writeln!(identity_file, "{}", identity.to_string().expose_secret()).unwrap();

    let encrypted_file = tempfile::NamedTempFile::new().unwrap();
    let encryptor = age::Encryptor::with_recipients(std::iter::once(&recipient as &dyn age::Recipient)).unwrap();
    let mut writer = encryptor.wrap_output(std::fs::File::create(encrypted_file.path()).unwrap()).unwrap();
    writer.write_all(plaintext.as_bytes()).unwrap();
    writer.finish().unwrap();

    (identity_file, encrypted_file)
}

#[tokio::test]
async fn test_age_resolves_encrypted_file() {
    let (identity_file, encrypted_file) = setup_age_encrypted_file("super-secret-value");

    let config = HashMap::from([
        ("identity".to_string(), toml::Value::String(identity_file.path().to_str().unwrap().to_string())),
    ]);
    let resolver = AgeResolver::new(config).unwrap();

    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String(encrypted_file.path().to_str().unwrap().to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value, "super-secret-value");
}

#[tokio::test]
async fn test_age_errors_on_missing_file() {
    let (identity_file, _) = setup_age_encrypted_file("dummy");

    let config = HashMap::from([
        ("identity".to_string(), toml::Value::String(identity_file.path().to_str().unwrap().to_string())),
    ]);
    let resolver = AgeResolver::new(config).unwrap();

    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("/nonexistent/file.age".to_string())),
        ]),
    };
    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
}
