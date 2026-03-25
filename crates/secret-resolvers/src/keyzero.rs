use crate::{ResolveRequest, ResolvedSecret, ResolverError, Result, SecretResolver};
use async_trait::async_trait;
use std::collections::HashMap;

pub struct KeyzeroResolver {
    endpoint: String,
    token: Option<String>,
    client: reqwest::Client,
}

impl KeyzeroResolver {
    pub fn new(config: HashMap<String, toml::Value>) -> Result<Self> {
        let endpoint = config
            .get("endpoint")
            .and_then(|v| v.as_str())
            .map(|s| s.trim_end_matches('/').to_string())
            .ok_or_else(|| ResolverError::ConfigError("'endpoint' is required for keyzero provider".to_string()))?;

        let token = config
            .get("token")
            .and_then(|v| v.as_str())
            .map(String::from);

        let client = reqwest::Client::builder()
            .build()
            .map_err(|e| ResolverError::ConfigError(format!("failed to build HTTP client: {e}")))?;

        Ok(Self { endpoint, token, client })
    }
}

#[async_trait]
impl SecretResolver for KeyzeroResolver {
    async fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedSecret> {
        let resource = request.get_str("ref")?;
        let secret_name = request.get_str_opt("secret_name");

        let url = format!("{}/v1/resolve", self.endpoint);

        let body = serde_json::json!({ "resource": resource });

        let mut req = self.client.post(&url).json(&body);

        if let Some(token) = &self.token {
            req = req.bearer_auth(token);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| ResolverError::ResolutionFailed(format!("HTTP request to keyzero failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ResolverError::ResolutionFailed(format!(
                "keyzero returned {status}: {body}"
            )));
        }

        let payload: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ResolverError::ResolutionFailed(format!("failed to parse keyzero response: {e}")))?;

        // Check allowed flag
        let allowed = payload
            .get("allowed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !allowed {
            return Err(ResolverError::ResolutionFailed(format!(
                "keyzero denied access to resource '{resource}'"
            )));
        }

        let secrets = payload
            .get("secrets")
            .and_then(|v| v.as_object())
            .ok_or_else(|| ResolverError::ResolutionFailed("keyzero response missing 'secrets' object".to_string()))?;

        let secret_entry = if let Some(name) = secret_name {
            secrets
                .get(name)
                .ok_or_else(|| {
                    ResolverError::ResolutionFailed(format!(
                        "secret_name '{}' not found in keyzero response",
                        name
                    ))
                })?
        } else {
            // Use the first secret
            secrets
                .values()
                .next()
                .ok_or_else(|| ResolverError::ResolutionFailed("keyzero response contains no secrets".to_string()))?
        };

        let value = secret_entry
            .get("value")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ResolverError::ResolutionFailed("secret entry missing 'value' field".to_string()))?
            .to_string();

        let ttl = secret_entry
            .get("ttl")
            .and_then(|v| v.as_u64());

        Ok(ResolvedSecret { value, ttl })
    }
}
