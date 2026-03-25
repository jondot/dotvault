use crate::{ResolveRequest, ResolvedSecret, ResolverError, Result, SecretResolver, SecretWriter, WriteRequest};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue};
use std::collections::HashMap;

pub struct HashiCorpResolver {
    address: String,
    token: String,
    namespace: Option<String>,
    client: reqwest::Client,
}

impl HashiCorpResolver {
    pub fn new(config: HashMap<String, toml::Value>) -> Result<Self> {
        let address = config
            .get("address")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| std::env::var("VAULT_ADDR").ok())
            .unwrap_or_else(|| "http://127.0.0.1:8200".to_string());

        let token = config
            .get("token")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| std::env::var("VAULT_TOKEN").ok())
            .ok_or_else(|| ResolverError::ConfigError("vault token is required (set 'token' in config or VAULT_TOKEN env var)".to_string()))?;

        let namespace = config
            .get("namespace")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| std::env::var("VAULT_NAMESPACE").ok());

        let mut default_headers = HeaderMap::new();
        default_headers.insert(
            "X-Vault-Token",
            HeaderValue::from_str(&token)
                .map_err(|e| ResolverError::ConfigError(format!("invalid vault token: {e}")))?,
        );
        if let Some(ref ns) = namespace {
            default_headers.insert(
                "X-Vault-Namespace",
                HeaderValue::from_str(ns)
                    .map_err(|e| ResolverError::ConfigError(format!("invalid namespace: {e}")))?,
            );
        }

        let client = reqwest::Client::builder()
            .default_headers(default_headers)
            .build()
            .map_err(|e| ResolverError::ConfigError(format!("failed to build HTTP client: {e}")))?;

        Ok(Self { address, token, namespace, client })
    }
}

#[async_trait]
impl SecretResolver for HashiCorpResolver {
    async fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedSecret> {
        let path = request.get_str("ref")?;
        let field = request.get_str("field")?;

        // Vault KV v2: /v1/{mount}/data/{path}
        // The caller passes paths like "secret/data/myapp" or "secret/myapp"
        // We call the API at /v1/{path}
        let url = format!("{}/v1/{}", self.address.trim_end_matches('/'), path);

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ResolverError::ResolutionFailed(format!("HTTP request to Vault failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ResolverError::ResolutionFailed(format!(
                "Vault returned {status}: {body}"
            )));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ResolverError::ResolutionFailed(format!("failed to parse Vault response: {e}")))?;

        // KV v2 response structure: { "data": { "data": { "field": "value" } } }
        let value = body
            .pointer("/data/data")
            .and_then(|d| d.get(field))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ResolverError::ResolutionFailed(format!(
                    "field '{}' not found at path '{}' in Vault response",
                    field, path
                ))
            })?
            .to_string();

        Ok(ResolvedSecret { value, ttl: None })
    }
}

#[async_trait]
impl SecretWriter for HashiCorpResolver {
    async fn write(&self, request: &WriteRequest) -> Result<()> {
        let path = request
            .params
            .get("ref")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ResolverError::MissingParam("ref".to_string()))?;
        let field = request
            .params
            .get("field")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ResolverError::MissingParam("field".to_string()))?;

        let url = format!("{}/v1/{}", self.address.trim_end_matches('/'), path);

        // For KV v2, we need to wrap in {"data": {...}}
        let body = serde_json::json!({
            "data": { field: request.value }
        });

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                ResolverError::ResolutionFailed(format!("Vault write failed: {e}"))
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ResolverError::ResolutionFailed(format!(
                "Vault returned {status}: {body}"
            )));
        }

        Ok(())
    }
}

// Suppress unused field warnings for fields only needed for context
impl HashiCorpResolver {
    #[allow(dead_code)]
    fn address(&self) -> &str {
        &self.address
    }
    #[allow(dead_code)]
    fn token(&self) -> &str {
        &self.token
    }
    #[allow(dead_code)]
    fn namespace(&self) -> Option<&str> {
        self.namespace.as_deref()
    }
}
