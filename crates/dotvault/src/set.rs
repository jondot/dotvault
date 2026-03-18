use anyhow::{bail, Result};
use dialoguer::{Input, Password, Select};
use secret_resolvers::{SecretWriter, WriteRequest};
use std::collections::HashMap;
use std::path::Path;

const WRITABLE_PROVIDERS: &[&str] = &["1password", "keychain", "aws", "hashicorp"];

pub async fn set_secret(
    dir: &Path,
    provider: Option<String>,
    reference: Option<String>,
    field: Option<String>,
    value: Option<String>,
) -> Result<()> {
    let provider_name = match provider {
        Some(p) => p,
        None => {
            let idx = Select::new()
                .with_prompt("Provider")
                .items(WRITABLE_PROVIDERS)
                .default(0)
                .interact()?;
            WRITABLE_PROVIDERS[idx].to_string()
        }
    };

    let reference = match reference {
        Some(r) => r,
        None => Input::<String>::new()
            .with_prompt("Reference (where to store)")
            .interact_text()?,
    };

    let field = if field.is_some() {
        field
    } else if provider_name == "hashicorp" {
        Some(
            Input::<String>::new()
                .with_prompt("Field name")
                .interact_text()?,
        )
    } else {
        None
    };

    let value = match value {
        Some(v) => v,
        None => Password::new()
            .with_prompt("Value (hidden)")
            .interact()?,
    };

    // Load provider config if available
    let provider_config = load_provider_config(dir, &provider_name)?;

    // Create the writer
    let writer = create_writer(&provider_name, provider_config).await?;

    // Build write request
    let mut params = HashMap::new();
    params.insert("ref".to_string(), toml::Value::String(reference.clone()));
    if let Some(ref f) = field {
        params.insert("field".to_string(), toml::Value::String(f.clone()));
    }

    let request = WriteRequest { params, value };
    writer
        .write(&request)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to write secret: {e}"))?;

    println!("\u{2713} Stored secret at {reference} via {provider_name}");
    Ok(())
}

fn load_provider_config(
    dir: &Path,
    provider_name: &str,
) -> Result<HashMap<String, toml::Value>> {
    // Try to load from project config
    let config_path = dir.join(".dotvault.toml");
    if config_path.exists() {
        if let Ok(config) = crate::config::DotVaultConfig::load(&config_path) {
            if let Some(provider_config) = config.providers.get(provider_name) {
                return Ok(provider_config.clone());
            }
        }
    }

    // Try global config
    if let Some(global_dir) = dirs::config_dir() {
        let global_path = global_dir.join("dotvault").join("config.toml");
        if global_path.exists() {
            if let Ok(config) = crate::config::DotVaultConfig::load(&global_path) {
                if let Some(provider_config) = config.providers.get(provider_name) {
                    return Ok(provider_config.clone());
                }
            }
        }
    }

    Ok(HashMap::new())
}

async fn create_writer(
    name: &str,
    mut config: HashMap<String, toml::Value>,
) -> Result<Box<dyn SecretWriter>> {
    // Extract type from config, falling back to provider name
    let provider_type = config
        .remove("type")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| name.to_string());

    match provider_type.as_str() {
        "1password" => Ok(Box::new(
            secret_resolvers::OnePasswordResolver::new(config)
                .map_err(|e| anyhow::anyhow!("failed to create 1password provider: {e}"))?,
        )),
        "keychain" => Ok(Box::new(
            secret_resolvers::KeychainResolver::new(config)
                .map_err(|e| anyhow::anyhow!("failed to create keychain provider: {e}"))?,
        )),
        "hashicorp" => Ok(Box::new(
            secret_resolvers::HashiCorpResolver::new(config)
                .map_err(|e| anyhow::anyhow!("failed to create hashicorp provider: {e}"))?,
        )),
        "aws" => Ok(Box::new(
            secret_resolvers::AwsResolver::new(config)
                .await
                .map_err(|e| anyhow::anyhow!("failed to create aws provider: {e}"))?,
        )),
        other => bail!(
            "Provider type '{other}' does not support writing. Supported: {}",
            WRITABLE_PROVIDERS.join(", ")
        ),
    }
}
