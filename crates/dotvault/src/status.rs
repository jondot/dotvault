use crate::config::{detect_environment, DotVaultConfig};
use crate::resolve::resolve_each;
use anyhow::Result;
use secret_resolvers::ExposeSecret;
use std::path::Path;

fn mask_value(value: &str) -> String {
    let last4: String = value.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    if value.chars().count() > 4 {
        format!("****{last4}")
    } else {
        "****".to_string()
    }
}

fn config_file_name(dir: &Path) -> &str {
    if dir.join(".dotvault.local.toml").exists() {
        ".dotvault.local.toml"
    } else {
        ".dotvault.toml"
    }
}

pub async fn show_status(
    dir: &Path,
    config: &DotVaultConfig,
    only: Option<&[String]>,
) -> Result<bool> {
    let env = detect_environment();
    let config_name = config_file_name(dir);

    eprintln!("Environment: {env}");
    eprintln!("Config: {config_name}");
    eprintln!();

    let results = resolve_each(config, only).await?;

    let mut all_ok = true;

    for (name, result) in &results {
        let provider = config
            .secrets
            .get(name)
            .map(|e| e.provider.as_str())
            .unwrap_or("?");

        match result {
            Ok(value) => {
                let masked = mask_value(value.expose_secret());
                eprintln!("  {name:<20} {provider:<14} \u{2713}  {masked}");
            }
            Err(err) => {
                all_ok = false;
                eprintln!("  {name:<20} {provider:<14} \u{2717}  {err}");
            }
        }
    }

    Ok(all_ok)
}

pub async fn show_status_json(
    config: &DotVaultConfig,
    only: Option<&[String]>,
) -> Result<bool> {
    let env = detect_environment();
    let results = resolve_each(config, only).await?;

    let mut all_ok = true;
    let mut secrets_arr = Vec::new();

    for (name, result) in &results {
        let provider = config
            .secrets
            .get(name)
            .map(|e| e.provider.as_str())
            .unwrap_or("?");
        let ref_val = config
            .secrets
            .get(name)
            .and_then(|e| e.extra.get("ref"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match result {
            Ok(_) => {
                secrets_arr.push(serde_json::json!({
                    "name": name,
                    "provider": provider,
                    "ref": ref_val,
                    "status": "ok",
                }));
            }
            Err(err) => {
                all_ok = false;
                secrets_arr.push(serde_json::json!({
                    "name": name,
                    "provider": provider,
                    "ref": ref_val,
                    "status": "error",
                    "error": err,
                }));
            }
        }
    }

    let output = serde_json::json!({
        "environment": env,
        "all_ok": all_ok,
        "secrets": secrets_arr,
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(all_ok)
}
