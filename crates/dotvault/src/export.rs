use crate::config::DotVaultConfig;
use crate::resolve::resolve_all;
use anyhow::Result;

pub async fn export_secrets(config: &DotVaultConfig) -> Result<String> {
    let secrets = resolve_all(config).await?;

    let mut lines: Vec<String> = secrets
        .into_iter()
        .map(|(key, value)| {
            // Escape single quotes by replacing ' with '\''
            let escaped = value.replace('\'', "'\\''");
            format!("export {}='{}'", key, escaped)
        })
        .collect();

    lines.sort(); // deterministic output
    Ok(lines.join("\n"))
}
