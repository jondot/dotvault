#![cfg(feature = "hashicorp")]

use secret_resolvers::{HashiCorpResolver, SecretResolver, ResolveRequest, ExposeSecret};
use std::collections::HashMap;

const VAULT_ADDR: &str = "http://localhost:8200";
const VAULT_TOKEN: &str = "test-root-token";

/// Returns true if Vault is reachable.
async fn vault_available() -> bool {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap();
    match client
        .get(format!("{}/v1/sys/health", VAULT_ADDR))
        .send()
        .await
    {
        Ok(resp) => {
            // Vault returns 200 (active) or 429 (standby) — both mean it's running
            resp.status().as_u16() < 500
        }
        Err(_) => false,
    }
}

fn vault_config() -> HashMap<String, toml::Value> {
    HashMap::from([
        ("address".to_string(), toml::Value::String(VAULT_ADDR.to_string())),
        ("token".to_string(), toml::Value::String(VAULT_TOKEN.to_string())),
    ])
}

/// Seed a KV v2 secret in Vault via HTTP.
async fn seed_vault_secret(path: &str, data: serde_json::Value) {
    let client = reqwest::Client::new();
    // Vault KV v2 write: PUT /v1/{path} with body { "data": {...} }
    let resp = client
        .post(format!("{}/v1/{}", VAULT_ADDR, path))
        .header("X-Vault-Token", VAULT_TOKEN)
        .json(&serde_json::json!({ "data": data }))
        .send()
        .await
        .expect("failed to seed secret");
    assert!(
        resp.status().is_success(),
        "seed failed with status {}: {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );
}

#[tokio::test]
async fn test_vault_resolve_secret_field() {
    if !vault_available().await {
        eprintln!("SKIP: HashiCorp Vault not running at {}", VAULT_ADDR);
        return;
    }

    seed_vault_secret(
        "secret/data/myapp",
        serde_json::json!({ "api_key": "vault-secret-value" }),
    )
    .await;

    let resolver = HashiCorpResolver::new(vault_config()).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("secret/data/myapp".to_string())),
            ("field".to_string(), toml::Value::String("api_key".to_string())),
        ]),
    };

    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value.expose_secret(), "vault-secret-value");
}

#[tokio::test]
async fn test_vault_missing_field_errors() {
    if !vault_available().await {
        eprintln!("SKIP: HashiCorp Vault not running at {}", VAULT_ADDR);
        return;
    }

    seed_vault_secret(
        "secret/data/myapp2",
        serde_json::json!({ "existing_key": "some-value" }),
    )
    .await;

    let resolver = HashiCorpResolver::new(vault_config()).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("secret/data/myapp2".to_string())),
            ("field".to_string(), toml::Value::String("nonexistent_field".to_string())),
        ]),
    };

    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_vault_missing_token_errors() {
    let config: HashMap<String, toml::Value> = HashMap::from([
        ("address".to_string(), toml::Value::String(VAULT_ADDR.to_string())),
        // No token
    ]);
    // Remove VAULT_TOKEN env var for this test (save and restore)
    let saved = std::env::var("VAULT_TOKEN").ok();
    std::env::remove_var("VAULT_TOKEN");
    let result = HashiCorpResolver::new(config);
    if let Some(t) = saved {
        std::env::set_var("VAULT_TOKEN", t);
    }
    assert!(result.is_err());
}

#[tokio::test]
async fn test_vault_missing_field_param_errors() {
    let resolver = HashiCorpResolver::new(vault_config()).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([
            ("ref".to_string(), toml::Value::String("secret/data/myapp".to_string())),
            // No "field"
        ]),
    };
    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
}
