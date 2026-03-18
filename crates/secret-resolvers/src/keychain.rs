use crate::{ResolveRequest, ResolvedSecret, ResolverError, Result, SecretResolver, SecretWriter, WriteRequest};
use async_trait::async_trait;
use std::collections::HashMap;

pub struct KeychainResolver {
    service: String,
    biometric: bool,
}

impl KeychainResolver {
    pub fn new(config: HashMap<String, toml::Value>) -> Result<Self> {
        let service = config
            .get("service")
            .and_then(|v| v.as_str())
            .unwrap_or("dotvault")
            .to_string();
        let biometric = config
            .get("biometric")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        Ok(Self { service, biometric })
    }
}

#[async_trait]
impl SecretResolver for KeychainResolver {
    async fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedSecret> {
        let item_name = request.get_str("ref")?;

        if self.biometric {
            self.resolve_biometric(item_name)
        } else {
            let entry = keyring::Entry::new(&self.service, item_name)
                .map_err(|e| ResolverError::ResolutionFailed(format!("keychain entry error: {e}")))?;
            let value = entry.get_password().map_err(|e| {
                ResolverError::ResolutionFailed(format!("keychain lookup failed for '{item_name}': {e}"))
            })?;
            Ok(ResolvedSecret { value, ttl: None })
        }
    }
}

#[async_trait]
impl SecretWriter for KeychainResolver {
    async fn write(&self, request: &WriteRequest) -> Result<()> {
        let item_name = request
            .params
            .get("ref")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ResolverError::MissingParam("ref".to_string()))?;

        if self.biometric {
            self.write_biometric(item_name, &request.value)
        } else {
            let entry = keyring::Entry::new(&self.service, item_name)
                .map_err(|e| ResolverError::ResolutionFailed(format!("keychain entry error: {e}")))?;
            entry.set_password(&request.value).map_err(|e| {
                ResolverError::ResolutionFailed(format!(
                    "keychain write failed for '{item_name}': {e}"
                ))
            })?;
            Ok(())
        }
    }
}

impl KeychainResolver {
    #[cfg(target_os = "macos")]
    fn resolve_biometric(&self, item_name: &str) -> Result<ResolvedSecret> {
        use security_framework::passwords::get_generic_password;

        let bytes = get_generic_password(&self.service, item_name)
            .map_err(|e| ResolverError::ResolutionFailed(format!("keychain biometric read failed for '{item_name}': {e}")))?;
        let value = String::from_utf8(bytes)
            .map_err(|e| ResolverError::ResolutionFailed(format!("keychain value not UTF-8: {e}")))?;
        Ok(ResolvedSecret { value, ttl: None })
    }

    #[cfg(not(target_os = "macos"))]
    fn resolve_biometric(&self, _item_name: &str) -> Result<ResolvedSecret> {
        Err(ResolverError::ConfigError("biometric keychain is only supported on macOS".into()))
    }

    #[cfg(target_os = "macos")]
    fn write_biometric(&self, item_name: &str, value: &str) -> Result<()> {
        use security_framework::passwords::{
            set_generic_password_options, delete_generic_password,
            AccessControlOptions, PasswordOptions,
        };

        // Delete existing entry first (access control can't be updated, must recreate)
        let _ = delete_generic_password(&self.service, item_name);

        let mut options = PasswordOptions::new_generic_password(&self.service, item_name);
        options.set_access_control_options(
            AccessControlOptions::BIOMETRY_CURRENT_SET
                | AccessControlOptions::OR
                | AccessControlOptions::DEVICE_PASSCODE,
        );

        set_generic_password_options(value.as_bytes(), options)
            .map_err(|e| ResolverError::ResolutionFailed(format!("keychain biometric write failed for '{item_name}': {e}")))?;
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    fn write_biometric(&self, _item_name: &str, _value: &str) -> Result<()> {
        Err(ResolverError::ConfigError("biometric keychain is only supported on macOS".into()))
    }
}
