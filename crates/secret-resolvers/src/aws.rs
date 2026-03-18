use crate::{ResolveRequest, ResolvedSecret, ResolverError, Result, SecretResolver};
use async_trait::async_trait;
use aws_config::BehaviorVersion;
use std::collections::HashMap;

pub struct AwsResolver {
    sm_client: aws_sdk_secretsmanager::Client,
    ssm_client: aws_sdk_ssm::Client,
}

impl AwsResolver {
    pub async fn new(config: HashMap<String, toml::Value>) -> Result<Self> {
        let mut loader = aws_config::defaults(BehaviorVersion::latest());

        if let Some(region) = config.get("region").and_then(|v| v.as_str()) {
            loader = loader.region(aws_config::Region::new(region.to_string()));
        }

        if let Some(profile) = config.get("profile").and_then(|v| v.as_str()) {
            loader = loader.profile_name(profile);
        }

        let sdk_config = loader.load().await;

        let sm_client = if let Some(endpoint_url) = config.get("endpoint_url").and_then(|v| v.as_str()) {
            let sm_config = aws_sdk_secretsmanager::config::Builder::from(&sdk_config)
                .endpoint_url(endpoint_url)
                .build();
            aws_sdk_secretsmanager::Client::from_conf(sm_config)
        } else {
            aws_sdk_secretsmanager::Client::new(&sdk_config)
        };

        let ssm_client = if let Some(endpoint_url) = config.get("endpoint_url").and_then(|v| v.as_str()) {
            let ssm_config = aws_sdk_ssm::config::Builder::from(&sdk_config)
                .endpoint_url(endpoint_url)
                .build();
            aws_sdk_ssm::Client::from_conf(ssm_config)
        } else {
            aws_sdk_ssm::Client::new(&sdk_config)
        };

        Ok(Self { sm_client, ssm_client })
    }
}

#[async_trait]
impl SecretResolver for AwsResolver {
    async fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedSecret> {
        let reference = request.get_str("ref")?;

        if let Some(secret_id) = reference.strip_prefix("sm://") {
            // Secrets Manager
            let resp = self
                .sm_client
                .get_secret_value()
                .secret_id(secret_id)
                .send()
                .await
                .map_err(|e| ResolverError::ResolutionFailed(format!("SecretsManager error: {e}")))?;

            let raw = resp
                .secret_string()
                .ok_or_else(|| ResolverError::ResolutionFailed("secret has no string value (binary secrets not supported)".to_string()))?;

            if let Some(field) = request.get_str_opt("field") {
                let parsed: serde_json::Value = serde_json::from_str(raw)
                    .map_err(|e| ResolverError::ResolutionFailed(format!("failed to parse secret as JSON: {e}")))?;
                let value = parsed
                    .get(field)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ResolverError::ResolutionFailed(format!("field '{field}' not found in secret JSON")))?
                    .to_string();
                return Ok(ResolvedSecret { value, ttl: None });
            }

            Ok(ResolvedSecret { value: raw.to_string(), ttl: None })
        } else if let Some(param_name) = reference.strip_prefix("ssm://") {
            // SSM Parameter Store
            let resp = self
                .ssm_client
                .get_parameter()
                .name(param_name)
                .with_decryption(true)
                .send()
                .await
                .map_err(|e| ResolverError::ResolutionFailed(format!("SSM error: {e}")))?;

            let value = resp
                .parameter()
                .and_then(|p| p.value())
                .ok_or_else(|| ResolverError::ResolutionFailed("SSM parameter has no value".to_string()))?
                .to_string();

            Ok(ResolvedSecret { value, ttl: None })
        } else {
            Err(ResolverError::ConfigError(format!(
                "unknown ref prefix in '{}': expected 'sm://' or 'ssm://'",
                reference
            )))
        }
    }
}
