use crate::config::{DotVaultConfig, SecretEntry};
use anyhow::{anyhow, Result};
use futures::future::join_all;
use secret_resolvers::{ExposeSecret, ResolveRequest, SecretResolver, SecretString};
use std::collections::HashMap;
use std::sync::Arc;

pub async fn resolve_all(
    config: &DotVaultConfig,
    only: Option<&[String]>,
) -> Result<HashMap<String, SecretString>> {
    let providers = build_providers(config).await?;

    let secrets_to_resolve: Vec<(&String, &SecretEntry)> = match only {
        Some(keys) => {
            let mut not_found: Vec<&str> = Vec::new();
            for key in keys {
                if !config.secrets.contains_key(key) {
                    not_found.push(key);
                }
            }
            if !not_found.is_empty() {
                not_found.sort();
                return Err(anyhow!(
                    "secret(s) not found in config: {}",
                    not_found.join(", ")
                ));
            }
            config
                .secrets
                .iter()
                .filter(|(name, _)| keys.iter().any(|k| k == *name))
                .collect()
        }
        None => config.secrets.iter().collect(),
    };

    let futures: Vec<_> = secrets_to_resolve
        .into_iter()
        .map(|(secret_name, entry)| {
            let providers = &providers;
            let secret_name = secret_name.clone();
            let entry = entry.clone();
            async move {
                let provider = providers.get(&entry.provider).ok_or_else(|| {
                    anyhow!(
                        "secret '{}': unknown provider '{}'",
                        secret_name,
                        entry.provider
                    )
                })?;
                let request = build_request(&entry);
                let resolved = provider.resolve(&request).await.map_err(|e| {
                    anyhow!("secret '{}': resolution failed: {}", secret_name, e)
                })?;
                if !entry.allow_empty && resolved.value.expose_secret().is_empty() {
                    return Err(anyhow!(
                        "secret '{}': resolved to empty value",
                        secret_name
                    ));
                }
                Ok::<(String, SecretString), anyhow::Error>((secret_name, resolved.value))
            }
        })
        .collect();

    let results = join_all(futures).await;

    let mut secrets = HashMap::new();
    let mut errors: Vec<String> = Vec::new();

    for result in results {
        match result {
            Ok((name, value)) => {
                secrets.insert(name, value);
            }
            Err(e) => {
                errors.push(e.to_string());
            }
        }
    }

    if !errors.is_empty() {
        return Err(anyhow!(
            "failed to resolve {} secret(s):\n  - {}",
            errors.len(),
            errors.join("\n  - ")
        ));
    }

    Ok(secrets)
}

/// Attempts to resolve all secrets, returning a list of (name, entry) pairs
/// for secrets that failed to resolve (i.e., are "missing").
pub async fn find_missing(config: &DotVaultConfig) -> Result<Vec<(String, SecretEntry)>> {
    let providers = build_providers(config).await?;

    let futures: Vec<_> = config
        .secrets
        .iter()
        .map(|(secret_name, entry)| {
            let providers = &providers;
            let secret_name = secret_name.clone();
            let entry = entry.clone();
            async move {
                let provider = match providers.get(&entry.provider) {
                    Some(p) => p,
                    None => return Err((secret_name, entry)),
                };
                let request = build_request(&entry);
                match provider.resolve(&request).await {
                    Ok(resolved) => {
                        if !entry.allow_empty && resolved.value.expose_secret().is_empty() {
                            Err((secret_name, entry))
                        } else {
                            Ok(())
                        }
                    }
                    Err(_) => Err((secret_name, entry)),
                }
            }
        })
        .collect();

    let results = join_all(futures).await;

    let mut missing = Vec::new();
    for result in results {
        if let Err((name, entry)) = result {
            missing.push((name, entry));
        }
    }
    missing.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(missing)
}

async fn build_providers(
    config: &DotVaultConfig,
) -> Result<HashMap<String, Arc<dyn SecretResolver>>> {
    // Collect unique provider names referenced by secrets
    let mut needed: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for entry in config.secrets.values() {
        needed.insert(&entry.provider);
    }

    let mut map: HashMap<String, Arc<dyn SecretResolver>> = HashMap::new();

    for provider_name in needed {
        let provider_config = config
            .providers
            .get(provider_name)
            .cloned()
            .unwrap_or_default();
        let resolver = create_provider(provider_name, provider_config).await?;
        map.insert(provider_name.to_string(), resolver);
    }

    Ok(map)
}

async fn create_provider(
    name: &str,
    mut config: HashMap<String, toml::Value>,
) -> Result<Arc<dyn SecretResolver>> {
    // Extract type from config, falling back to provider name
    let provider_type = config
        .remove("type")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| name.to_string());

    match provider_type.as_str() {
        "env" => {
            use secret_resolvers::EnvResolver;
            let r = EnvResolver::new(config)
                .map_err(|e| anyhow!("failed to create env provider: {}", e))?;
            Ok(Arc::new(r))
        }
        "1password" => {
            use secret_resolvers::OnePasswordResolver;
            let r = OnePasswordResolver::new(config)
                .map_err(|e| anyhow!("failed to create 1password provider: {}", e))?;
            Ok(Arc::new(r))
        }
        "keychain" => {
            use secret_resolvers::KeychainResolver;
            let r = KeychainResolver::new(config)
                .map_err(|e| anyhow!("failed to create keychain provider: {}", e))?;
            Ok(Arc::new(r))
        }
        "age" => {
            use secret_resolvers::AgeResolver;
            let r = AgeResolver::new(config)
                .map_err(|e| anyhow!("failed to create age provider: {}", e))?;
            Ok(Arc::new(r))
        }
        "aws" => {
            use secret_resolvers::AwsResolver;
            let r = AwsResolver::new(config)
                .await
                .map_err(|e| anyhow!("failed to create aws provider: {}", e))?;
            Ok(Arc::new(r))
        }
        "hashicorp" => {
            use secret_resolvers::HashiCorpResolver;
            let r = HashiCorpResolver::new(config)
                .map_err(|e| anyhow!("failed to create hashicorp provider: {}", e))?;
            Ok(Arc::new(r))
        }
        "gcp" => {
            use secret_resolvers::GcpResolver;
            let r = GcpResolver::new(config)
                .map_err(|e| anyhow!("failed to create gcp provider: {}", e))?;
            Ok(Arc::new(r))
        }
        "keyzero" => {
            use secret_resolvers::KeyzeroResolver;
            let r = KeyzeroResolver::new(config)
                .map_err(|e| anyhow!("failed to create keyzero provider: {}", e))?;
            Ok(Arc::new(r))
        }
        other => Err(anyhow!("unknown provider type '{}'", other)),
    }
}

fn build_request(entry: &SecretEntry) -> ResolveRequest {
    let params = entry.extra.clone();
    // provider key is already excluded (it's a named field in SecretEntry)
    ResolveRequest { params }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DotVaultConfig, SecretEntry};
    use secret_resolvers::ExposeSecret;

    #[tokio::test]
    async fn test_resolve_with_env_provider() {
        std::env::set_var("TEST_RESOLVE_VAR_10", "hello_world");

        let mut secrets = HashMap::new();
        secrets.insert(
            "MY_SECRET".to_string(),
            SecretEntry {
                provider: "env".to_string(),
                allow_empty: false,
                extra: {
                    let mut m = HashMap::new();
                    m.insert(
                        "ref".to_string(),
                        toml::Value::String("TEST_RESOLVE_VAR_10".to_string()),
                    );
                    m
                },
            },
        );

        let config = DotVaultConfig {
            providers: HashMap::new(),
            secrets,
        };

        let resolved = resolve_all(&config, None).await.unwrap();
        assert_eq!(resolved["MY_SECRET"].expose_secret(), "hello_world");
    }

    #[tokio::test]
    async fn test_resolve_with_named_provider() {
        std::env::set_var("TEST_NAMED_PROVIDER_VAR", "named-value");

        let mut providers = HashMap::new();
        let mut my_env_config = HashMap::new();
        my_env_config.insert("type".to_string(), toml::Value::String("env".to_string()));
        providers.insert("my-custom-env".to_string(), my_env_config);

        let mut secrets = HashMap::new();
        secrets.insert(
            "MY_SECRET".to_string(),
            SecretEntry {
                provider: "my-custom-env".to_string(),
                allow_empty: false,
                extra: {
                    let mut m = HashMap::new();
                    m.insert(
                        "ref".to_string(),
                        toml::Value::String("TEST_NAMED_PROVIDER_VAR".to_string()),
                    );
                    m
                },
            },
        );

        let config = DotVaultConfig { providers, secrets };
        let resolved = resolve_all(&config, None).await.unwrap();
        assert_eq!(resolved["MY_SECRET"].expose_secret(), "named-value");
    }

    #[tokio::test]
    async fn test_resolve_rejects_empty_value_by_default() {
        std::env::set_var("TEST_EMPTY_VAR", "");

        let mut secrets = HashMap::new();
        secrets.insert(
            "MY_SECRET".to_string(),
            SecretEntry {
                provider: "env".to_string(),
                allow_empty: false,
                extra: {
                    let mut m = HashMap::new();
                    m.insert(
                        "ref".to_string(),
                        toml::Value::String("TEST_EMPTY_VAR".to_string()),
                    );
                    m
                },
            },
        );

        let config = DotVaultConfig {
            providers: HashMap::new(),
            secrets,
        };

        let result = resolve_all(&config, None).await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("empty value"));
    }

    #[tokio::test]
    async fn test_resolve_accepts_empty_when_allow_empty() {
        std::env::set_var("TEST_EMPTY_ALLOWED_VAR", "");

        let mut secrets = HashMap::new();
        secrets.insert(
            "MY_SECRET".to_string(),
            SecretEntry {
                provider: "env".to_string(),
                allow_empty: true,
                extra: {
                    let mut m = HashMap::new();
                    m.insert(
                        "ref".to_string(),
                        toml::Value::String("TEST_EMPTY_ALLOWED_VAR".to_string()),
                    );
                    m
                },
            },
        );

        let config = DotVaultConfig {
            providers: HashMap::new(),
            secrets,
        };

        let result = resolve_all(&config, None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap()["MY_SECRET"].expose_secret(), "");
    }

    #[tokio::test]
    async fn test_find_missing_includes_empty_disallowed() {
        std::env::set_var("TEST_FIND_EMPTY_VAR", "");

        let mut secrets = HashMap::new();
        secrets.insert(
            "EMPTY_SECRET".to_string(),
            SecretEntry {
                provider: "env".to_string(),
                allow_empty: false,
                extra: {
                    let mut m = HashMap::new();
                    m.insert(
                        "ref".to_string(),
                        toml::Value::String("TEST_FIND_EMPTY_VAR".to_string()),
                    );
                    m
                },
            },
        );

        let config = DotVaultConfig {
            providers: HashMap::new(),
            secrets,
        };

        let missing = find_missing(&config).await.unwrap();
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].0, "EMPTY_SECRET");
    }

    #[tokio::test]
    async fn test_find_missing_excludes_empty_allowed() {
        std::env::set_var("TEST_FIND_EMPTY_ALLOWED_VAR", "");

        let mut secrets = HashMap::new();
        secrets.insert(
            "EMPTY_SECRET".to_string(),
            SecretEntry {
                provider: "env".to_string(),
                allow_empty: true,
                extra: {
                    let mut m = HashMap::new();
                    m.insert(
                        "ref".to_string(),
                        toml::Value::String("TEST_FIND_EMPTY_ALLOWED_VAR".to_string()),
                    );
                    m
                },
            },
        );

        let config = DotVaultConfig {
            providers: HashMap::new(),
            secrets,
        };

        let missing = find_missing(&config).await.unwrap();
        assert!(missing.is_empty());
    }

    #[tokio::test]
    async fn test_error_on_unknown_provider() {
        let mut secrets = HashMap::new();
        secrets.insert(
            "SOME_SECRET".to_string(),
            SecretEntry {
                provider: "nonexistent_provider".to_string(),
                allow_empty: false,
                extra: HashMap::new(),
            },
        );

        let config = DotVaultConfig {
            providers: HashMap::new(),
            secrets,
        };

        let result = resolve_all(&config, None).await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("unknown provider type"));
    }

    #[tokio::test]
    async fn test_resolve_all_with_only_filter() {
        std::env::set_var("TEST_ONLY_A", "value_a");
        std::env::set_var("TEST_ONLY_B", "value_b");

        let mut secrets = HashMap::new();
        secrets.insert(
            "SECRET_A".to_string(),
            SecretEntry {
                provider: "env".to_string(),
                allow_empty: false,
                extra: {
                    let mut m = HashMap::new();
                    m.insert("ref".to_string(), toml::Value::String("TEST_ONLY_A".to_string()));
                    m
                },
            },
        );
        secrets.insert(
            "SECRET_B".to_string(),
            SecretEntry {
                provider: "env".to_string(),
                allow_empty: false,
                extra: {
                    let mut m = HashMap::new();
                    m.insert("ref".to_string(), toml::Value::String("TEST_ONLY_B".to_string()));
                    m
                },
            },
        );

        let config = DotVaultConfig {
            providers: HashMap::new(),
            secrets,
        };

        let only = Some(vec!["SECRET_A".to_string()]);
        let resolved = resolve_all(&config, only.as_deref()).await.unwrap();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved["SECRET_A"].expose_secret(), "value_a");
        assert!(!resolved.contains_key("SECRET_B"));
    }

    #[tokio::test]
    async fn test_resolve_all_only_rejects_unknown_key() {
        let config = DotVaultConfig {
            providers: HashMap::new(),
            secrets: HashMap::new(),
        };

        let only = Some(vec!["NONEXISTENT".to_string()]);
        let result = resolve_all(&config, only.as_deref()).await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("NONEXISTENT"));
        assert!(msg.contains("not found in config"));
    }
}
