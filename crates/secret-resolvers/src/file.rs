use crate::{ResolveRequest, ResolvedSecret, ResolverError, Result, SecretResolver, SecretWriter, WriteRequest};
use async_trait::async_trait;
use secrecy::{ExposeSecret as _, SecretString};
use std::collections::HashMap;

pub struct FileResolver;

impl FileResolver {
    pub fn new(_config: HashMap<String, toml::Value>) -> Result<Self> {
        Ok(Self)
    }
}

#[async_trait]
impl SecretResolver for FileResolver {
    async fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedSecret> {
        let file_name = request.get_str("ref")?;

        let value = std::fs::read_to_string(file_name)
            .map_err(|e| ResolverError::ResolutionFailed(format!("failed to read file with secret: {e}")))?;
        Ok(ResolvedSecret { value: SecretString::from(value.trim_end()), ttl: None })
    }
}

#[async_trait]
impl SecretWriter for FileResolver {
    async fn write(&self, request: &WriteRequest) -> Result<()> {
        let file_name = request.params.get("ref")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ResolverError::MissingParam("ref".to_string()))?;
        let secret = request.value.expose_secret();
        std::fs::write(file_name, secret)
            .map_err(|error| ResolverError::ResolutionFailed(format!("Failed to write secret to '{file_name}': {error}")))?;
        Ok(())
    }
}