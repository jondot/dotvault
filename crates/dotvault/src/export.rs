use crate::config::{detect_environment, DotVaultConfig};
use crate::resolve::resolve_all;
use anyhow::Result;
use secret_resolvers::ExposeSecret;

pub async fn export_secrets(config: &DotVaultConfig, only: Option<&[String]>) -> Result<String> {
    let secrets = resolve_all(config, only).await?;

    let mut lines: Vec<String> = secrets
        .into_iter()
        .map(|(key, value)| {
            // Escape single quotes by replacing ' with '\''
            let escaped = value.expose_secret().replace('\'', "'\\''");
            format!("export {}='{}'", key, escaped)
        })
        .collect();

    lines.sort(); // deterministic output
    Ok(lines.join("\n"))
}

pub async fn export_secrets_json(config: &DotVaultConfig, only: Option<&[String]>) -> Result<String> {
    let secrets = resolve_all(config, only).await?;
    let env = detect_environment();

    let mut secrets_map = serde_json::Map::new();
    let mut keys: Vec<&String> = secrets.keys().collect();
    keys.sort();
    for key in keys {
        secrets_map.insert(
            key.clone(),
            serde_json::Value::String(secrets[key].expose_secret().to_string()),
        );
    }

    let output = serde_json::json!({
        "environment": env,
        "secrets": secrets_map,
    });

    Ok(serde_json::to_string_pretty(&output)?)
}
