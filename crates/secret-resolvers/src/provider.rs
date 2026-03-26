use async_trait::async_trait;
use secrecy::SecretString;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ResolverError {
    #[error("missing required param '{0}'")]
    MissingParam(String),

    #[error("resolution failed: {0}")]
    ResolutionFailed(String),

    #[error("configuration error: {0}")]
    ConfigError(String),
}

pub type Result<T> = std::result::Result<T, ResolverError>;

pub struct ResolveRequest {
    pub params: HashMap<String, toml::Value>,
}

impl ResolveRequest {
    pub fn get_str(&self, key: &str) -> Result<&str> {
        self.params
            .get(key)
            .and_then(|v| v.as_str())
            .ok_or_else(|| ResolverError::MissingParam(key.to_string()))
    }

    pub fn get_str_opt(&self, key: &str) -> Option<&str> {
        self.params.get(key).and_then(|v| v.as_str())
    }
}

#[derive(Debug)]
pub struct ResolvedSecret {
    pub value: SecretString,
    pub ttl: Option<u64>,
}

#[async_trait]
pub trait SecretResolver: Send + Sync {
    async fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedSecret>;
}

pub struct WriteRequest {
    pub params: HashMap<String, toml::Value>,
    pub value: SecretString,
}

/// Optional trait for providers that support writing secrets.
/// Not all providers support this (e.g., `env` is read-only).
#[async_trait]
pub trait SecretWriter: Send + Sync {
    async fn write(&self, request: &WriteRequest) -> Result<()>;
}
