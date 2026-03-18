use crate::{ResolveRequest, ResolvedSecret, ResolverError, Result, SecretResolver};
use async_trait::async_trait;
use base64::Engine;
use std::collections::HashMap;

pub struct GcpResolver {
    project: Option<String>,
    endpoint_url: String,
    client: reqwest::Client,
}

impl GcpResolver {
    pub fn new(config: HashMap<String, toml::Value>) -> Result<Self> {
        let project = config
            .get("project")
            .and_then(|v| v.as_str())
            .map(String::from);

        let endpoint_url = config
            .get("endpoint_url")
            .and_then(|v| v.as_str())
            .unwrap_or("https://secretmanager.googleapis.com")
            .trim_end_matches('/')
            .to_string();

        let client = reqwest::Client::builder()
            .build()
            .map_err(|e| ResolverError::ConfigError(format!("failed to build HTTP client: {e}")))?;

        Ok(Self { project, endpoint_url, client })
    }
}

#[async_trait]
impl SecretResolver for GcpResolver {
    async fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedSecret> {
        let secret_ref = request.get_str("ref")?;
        let version = request.get_str_opt("version").unwrap_or("latest");

        // Support full resource name or short name
        let resource_name = if secret_ref.starts_with("projects/") {
            // Already a full resource name — use as-is but normalise version
            // e.g. "projects/X/secrets/Y" → "projects/X/secrets/Y/versions/{version}"
            if secret_ref.contains("/versions/") {
                secret_ref.to_string()
            } else {
                format!("{}/versions/{}", secret_ref, version)
            }
        } else {
            // Short name — requires project
            let project = self.project.as_deref().ok_or_else(|| {
                ResolverError::ConfigError(
                    "GCP project is required when using short secret names; set 'project' in config".to_string(),
                )
            })?;
            format!(
                "projects/{}/secrets/{}/versions/{}",
                project, secret_ref, version
            )
        };

        let url = format!("{}/v1/{}:access", self.endpoint_url, resource_name);

        let mut req = self.client.get(&url);

        if let Ok(token) = std::env::var("GOOGLE_ACCESS_TOKEN") {
            req = req.bearer_auth(token);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| ResolverError::ResolutionFailed(format!("HTTP request to GCP failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ResolverError::ResolutionFailed(format!(
                "GCP Secret Manager returned {status}: {body}"
            )));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ResolverError::ResolutionFailed(format!("failed to parse GCP response: {e}")))?;

        // Response: { "payload": { "data": "<base64-encoded-value>" } }
        let encoded = body
            .pointer("/payload/data")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ResolverError::ResolutionFailed("GCP response missing payload.data".to_string()))?;

        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| ResolverError::ResolutionFailed(format!("failed to base64-decode secret payload: {e}")))?;

        let value = String::from_utf8(decoded)
            .map_err(|e| ResolverError::ResolutionFailed(format!("secret payload is not valid UTF-8: {e}")))?;

        Ok(ResolvedSecret { value, ttl: None })
    }
}
