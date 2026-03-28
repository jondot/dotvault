use anyhow::{bail, Result};
use dialoguer::{theme::ColorfulTheme, Input, Password, Select};
use secret_resolvers::{SecretString, SecretWriter, WriteRequest};
use std::collections::HashMap;
use std::path::Path;

use crate::config::DotVaultConfig;
use crate::resolve;

const WRITABLE_PROVIDERS: &[&str] = &["1password", "keychain", "aws", "hashicorp"];

pub async fn set_secret(
    dir: &Path,
    provider: Option<String>,
    reference: Option<String>,
    field: Option<String>,
    value: Option<String>,
) -> Result<()> {
    let theme = ColorfulTheme::default();

    let provider_name = match provider {
        Some(p) => p,
        None => {
            let idx = Select::with_theme(&theme)
                .with_prompt("Provider")
                .items(WRITABLE_PROVIDERS)
                .default(0)
                .interact()?;
            WRITABLE_PROVIDERS[idx].to_string()
        }
    };

    let reference = match reference {
        Some(r) => r,
        None => Input::<String>::with_theme(&theme)
            .with_prompt("Reference (where to store)")
            .interact_text()?,
    };

    let field = if field.is_some() {
        field
    } else if provider_name == "hashicorp" {
        Some(
            Input::<String>::with_theme(&theme)
                .with_prompt("Field name")
                .interact_text()?,
        )
    } else {
        None
    };

    let value = match value {
        Some(v) => v,
        None => Password::with_theme(&theme)
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

    let request = WriteRequest { params, value: SecretString::from(value) };
    writer
        .write(&request)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to write secret: {e}"))?;

    println!("\u{2713} Stored secret at {reference} via {provider_name}");
    Ok(())
}

pub async fn put_missing(_dir: &Path, config: &DotVaultConfig) -> Result<()> {
    let missing = resolve::find_missing(config).await?;

    if missing.is_empty() {
        println!("\u{2713} All secrets resolved successfully — nothing to fill.");
        return Ok(());
    }

    println!(
        "Found {} missing secret(s). Fill them interactively:\n",
        missing.len()
    );

    let theme = ColorfulTheme::default();
    let mut filled = 0;
    let mut skipped = 0;

    for (name, entry) in &missing {
        let ref_val = entry
            .extra
            .get("ref")
            .and_then(|v| v.as_str())
            .unwrap_or("(no ref)");
        let field_val = entry.extra.get("field").and_then(|v| v.as_str());

        let label = match field_val {
            Some(f) => format!("{name}  ({} -> {ref_val}, field={f})", entry.provider),
            None => format!("{name}  ({} -> {ref_val})", entry.provider),
        };

        println!("  {label}");

        if !WRITABLE_PROVIDERS.contains(&entry.provider.as_str()) {
            println!(
                "  \u{26A0} Provider '{}' is read-only — configure it externally.\n",
                entry.provider
            );
            skipped += 1;
            continue;
        }

        let value: String = Password::with_theme(&theme)
            .with_prompt(format!("  Value for {name} (empty to skip)"))
            .allow_empty_password(true)
            .interact()?;

        if value.is_empty() {
            println!("  skipped\n");
            skipped += 1;
            continue;
        }

        let provider_config = config
            .providers
            .get(&entry.provider)
            .cloned()
            .unwrap_or_default();
        let writer = create_writer(&entry.provider, provider_config).await?;

        let mut params = HashMap::new();
        params.insert(
            "ref".to_string(),
            toml::Value::String(ref_val.to_string()),
        );
        if let Some(f) = field_val {
            params.insert("field".to_string(), toml::Value::String(f.to_string()));
        }

        let request = WriteRequest {
            params,
            value: SecretString::from(value),
        };
        writer
            .write(&request)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to write {name}: {e}"))?;

        println!("  \u{2713} Stored {name}\n");
        filled += 1;
    }

    println!(
        "Done: {filled} stored, {skipped} skipped, {} total missing.",
        missing.len()
    );
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
