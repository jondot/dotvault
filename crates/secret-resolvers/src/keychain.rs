use crate::{ResolveRequest, ResolvedSecret, ResolverError, Result, SecretResolver};
use async_trait::async_trait;
use std::collections::HashMap;

pub struct KeychainResolver {
    service: String,
}

impl KeychainResolver {
    pub fn new(config: HashMap<String, toml::Value>) -> Result<Self> {
        let service = config
            .get("service")
            .and_then(|v| v.as_str())
            .unwrap_or("dotvault")
            .to_string();
        Ok(Self { service })
    }
}

#[async_trait]
impl SecretResolver for KeychainResolver {
    async fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedSecret> {
        let item_name = request.get_str("ref")?;
        let entry = keyring::Entry::new(&self.service, item_name)
            .map_err(|e| ResolverError::ResolutionFailed(format!("keychain entry error: {e}")))?;
        let value = entry.get_password().map_err(|e| {
            ResolverError::ResolutionFailed(format!("keychain lookup failed for '{item_name}': {e}"))
        })?;
        Ok(ResolvedSecret { value, ttl: None })
    }
}
