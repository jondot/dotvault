use crate::config::DotVaultConfig;
use anyhow::Result;
use std::path::Path;

const KNOWN_PROVIDERS: &[&str] = &[
    "env", "keychain", "1password", "age", "hashicorp", "aws", "gcp", "keyzero",
];

pub struct ValidationResult {
    pub file: String,
    pub valid: bool,
    pub secrets_count: usize,
    pub providers_count: usize,
    pub errors: Vec<ValidationError>,
}

pub struct ValidationError {
    pub name: String,
    pub error: String,
}

pub fn validate_config(dir: &Path) -> Result<ValidationResult> {
    let local_path = dir.join(".dotvault.local.toml");
    let shared_path = dir.join(".dotvault.toml");

    let (config, file) = if local_path.exists() {
        (DotVaultConfig::load(&local_path)?, ".dotvault.local.toml".to_string())
    } else if shared_path.exists() {
        (DotVaultConfig::load(&shared_path)?, ".dotvault.toml".to_string())
    } else {
        anyhow::bail!(
            "no config file found in {}: expected '.dotvault.local.toml' or '.dotvault.toml'",
            dir.display()
        );
    };

    let mut errors = Vec::new();

    let declared_providers: std::collections::HashSet<&str> =
        config.providers.keys().map(|s| s.as_str()).collect();

    for (name, entry) in &config.secrets {
        let provider = &entry.provider;
        if !KNOWN_PROVIDERS.contains(&provider.as_str()) && !declared_providers.contains(provider.as_str()) {
            errors.push(ValidationError {
                name: name.clone(),
                error: format!("unknown provider '{provider}'"),
            });
        }
    }

    errors.sort_by(|a, b| a.name.cmp(&b.name));

    let providers_count = {
        let mut used: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for entry in config.secrets.values() {
            used.insert(&entry.provider);
        }
        used.len()
    };

    Ok(ValidationResult {
        file,
        valid: errors.is_empty(),
        secrets_count: config.secrets.len(),
        providers_count,
        errors,
    })
}

pub fn format_text(result: &ValidationResult) -> String {
    let mut out = String::new();
    if result.valid {
        out.push_str(&format!(
            "{}: ok\n  {} secrets, {} providers\n",
            result.file, result.secrets_count, result.providers_count
        ));
    } else {
        out.push_str(&format!("{}: errors found\n", result.file));
        for err in &result.errors {
            out.push_str(&format!("  {}: {}\n", err.name, err.error));
        }
    }
    out
}

pub fn format_json(result: &ValidationResult) -> Result<String> {
    let errors_arr: Vec<serde_json::Value> = result
        .errors
        .iter()
        .map(|e| {
            serde_json::json!({
                "name": e.name,
                "error": e.error,
            })
        })
        .collect();

    let output = serde_json::json!({
        "file": result.file,
        "valid": result.valid,
        "secrets_count": result.secrets_count,
        "providers_count": result.providers_count,
        "errors": errors_arr,
    });

    Ok(serde_json::to_string_pretty(&output)?)
}
