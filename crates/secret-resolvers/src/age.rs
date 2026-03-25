use crate::{ResolveRequest, ResolvedSecret, ResolverError, Result, SecretResolver};
use async_trait::async_trait;
use std::collections::HashMap;
use std::io::Read;

pub struct AgeResolver {
    identity_path: String,
}

impl AgeResolver {
    pub fn new(config: HashMap<String, toml::Value>) -> Result<Self> {
        let identity_path = config
            .get("identity")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ResolverError::ConfigError("age provider requires 'identity' (path to key file)".into()))?
            .to_string();
        Ok(Self { identity_path })
    }
}

#[async_trait]
impl SecretResolver for AgeResolver {
    async fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedSecret> {
        let file_path = request.get_str("ref")?;

        let identity_contents = std::fs::read_to_string(&self.identity_path)
            .map_err(|e| ResolverError::ResolutionFailed(format!("failed to read identity file: {e}")))?;

        let identities: Vec<Box<dyn age::Identity>> = identity_contents
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .filter_map(|l| {
                l.parse::<age::x25519::Identity>()
                    .ok()
                    .map(|i| Box::new(i) as Box<dyn age::Identity>)
            })
            .collect();

        if identities.is_empty() {
            return Err(ResolverError::ConfigError("no valid age identities found in key file".into()));
        }

        let encrypted = std::fs::File::open(file_path)
            .map_err(|e| ResolverError::ResolutionFailed(format!("failed to open encrypted file '{file_path}': {e}")))?;

        let decryptor = age::Decryptor::new(encrypted)
            .map_err(|e| ResolverError::ResolutionFailed(format!("age decryption init failed: {e}")))?;

        if decryptor.is_scrypt() {
            return Err(ResolverError::ResolutionFailed("passphrase-encrypted age files are not supported".into()));
        }

        let mut decrypted = String::new();
        decryptor
            .decrypt(identities.iter().map(|i| i.as_ref()))
            .map_err(|e| ResolverError::ResolutionFailed(format!("age decryption failed: {e}")))?
            .read_to_string(&mut decrypted)
            .map_err(|e| ResolverError::ResolutionFailed(format!("failed to read decrypted data: {e}")))?;

        Ok(ResolvedSecret { value: decrypted, ttl: None })
    }
}
