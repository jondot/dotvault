use crate::{ResolveRequest, ResolvedSecret, ResolverError, Result, SecretResolver};
use async_trait::async_trait;
use secrecy::SecretString;
use std::collections::HashMap;

pub struct EnvResolver;

impl EnvResolver {
    pub fn new(_config: HashMap<String, toml::Value>) -> Result<Self> {
        Ok(Self)
    }
}

#[async_trait]
impl SecretResolver for EnvResolver {
    async fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedSecret> {
        let var_name = request.get_str("ref")?;
        let value = std::env::var(var_name).map_err(|_| {
            ResolverError::ResolutionFailed(format!("environment variable '{var_name}' not set"))
        })?;
        Ok(ResolvedSecret { value: SecretString::from(value), ttl: None })
    }
}
