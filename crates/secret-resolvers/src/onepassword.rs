use crate::{ResolveRequest, ResolvedSecret, ResolverError, Result, SecretResolver};
use async_trait::async_trait;
use std::collections::HashMap;

pub struct OnePasswordResolver {
    op_path: String,
    account: Option<String>,
    vault: Option<String>,
}

impl OnePasswordResolver {
    pub fn new(config: HashMap<String, toml::Value>) -> Result<Self> {
        Ok(Self {
            op_path: config
                .get("op_path")
                .and_then(|v| v.as_str())
                .unwrap_or("op")
                .to_string(),
            account: config.get("account").and_then(|v| v.as_str()).map(String::from),
            vault: config.get("vault").and_then(|v| v.as_str()).map(String::from),
        })
    }
}

#[async_trait]
impl SecretResolver for OnePasswordResolver {
    async fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedSecret> {
        let reference = request.get_str("ref")?;

        let mut cmd = tokio::process::Command::new(&self.op_path);
        cmd.args(["read", reference]);
        if let Some(account) = &self.account {
            cmd.args(["--account", account]);
        }
        if let Some(vault) = &self.vault {
            cmd.args(["--vault", vault]);
        }

        let output = cmd.output().await.map_err(|e| {
            ResolverError::ResolutionFailed(format!("failed to run op CLI: {e}"))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ResolverError::ResolutionFailed(format!("op CLI: {stderr}")));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let value = stdout.trim().to_string();

        // op read returns the value directly, but some older flows return JSON
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&value) {
            if let Some(v) = parsed.get("value").and_then(|v| v.as_str()) {
                return Ok(ResolvedSecret { value: v.to_string(), ttl: None });
            }
        }

        Ok(ResolvedSecret { value, ttl: None })
    }
}
