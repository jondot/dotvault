use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DotVaultConfig {
    #[serde(default)]
    pub providers: HashMap<String, HashMap<String, toml::Value>>,
    #[serde(default)]
    pub secrets: HashMap<String, SecretEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SecretEntry {
    pub provider: String,
    #[serde(flatten)]
    pub extra: HashMap<String, toml::Value>,
}

impl DotVaultConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read config file: {}", path.display()))?;
        let config: DotVaultConfig = toml::from_str(&contents)
            .with_context(|| format!("failed to parse config file: {}", path.display()))?;
        Ok(config)
    }

    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        let local_path = dir.join(".dotvault.local.toml");
        let shared_path = dir.join(".dotvault.toml");

        if local_path.exists() {
            return Self::load(&local_path);
        }

        if shared_path.exists() {
            return Self::load(&shared_path);
        }

        anyhow::bail!(
            "no config file found in {}: expected '.dotvault.local.toml' or '.dotvault.toml'",
            dir.display()
        )
    }

    /// Reads `~/.config/dotvault/config.toml` and merges provider config into self.
    /// Project config takes precedence (existing keys are not overwritten).
    pub fn merge_global_providers(&mut self) -> Result<()> {
        let global_path = match dirs::config_dir() {
            Some(d) => d.join("dotvault").join("config.toml"),
            None => return Ok(()),
        };

        if !global_path.exists() {
            return Ok(());
        }

        let global: DotVaultConfig = Self::load(&global_path)?;

        for (provider_name, global_cfg) in global.providers {
            let project_cfg = self.providers.entry(provider_name).or_default();
            // Fill in keys from global that are missing in project (project takes precedence)
            for (key, value) in global_cfg {
                project_cfg.entry(key).or_insert(value);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_file(dir: &TempDir, name: &str, content: &str) {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
    }

    #[test]
    fn test_basic_parsing() {
        let dir = TempDir::new().unwrap();
        write_file(
            &dir,
            ".dotvault.toml",
            r#"
[secrets.MY_KEY]
provider = "env"
ref = "MY_ENV_VAR"
"#,
        );

        let cfg = DotVaultConfig::load_from_dir(dir.path()).unwrap();
        assert!(cfg.secrets.contains_key("MY_KEY"));
        assert_eq!(cfg.secrets["MY_KEY"].provider, "env");
        assert_eq!(
            cfg.secrets["MY_KEY"].extra["ref"].as_str().unwrap(),
            "MY_ENV_VAR"
        );
    }

    #[test]
    fn test_provider_config() {
        let dir = TempDir::new().unwrap();
        write_file(
            &dir,
            ".dotvault.toml",
            r#"
[providers.aws]
region = "us-west-2"
profile = "myprofile"

[secrets.DB_PASS]
provider = "aws"
ref = "sm://prod/db"
"#,
        );

        let cfg = DotVaultConfig::load_from_dir(dir.path()).unwrap();
        assert_eq!(
            cfg.providers["aws"]["region"].as_str().unwrap(),
            "us-west-2"
        );
        assert_eq!(
            cfg.providers["aws"]["profile"].as_str().unwrap(),
            "myprofile"
        );
    }

    #[test]
    fn test_local_replaces_shared() {
        let dir = TempDir::new().unwrap();
        write_file(
            &dir,
            ".dotvault.toml",
            r#"
[secrets.KEY]
provider = "env"
ref = "SHARED_VAR"
"#,
        );
        write_file(
            &dir,
            ".dotvault.local.toml",
            r#"
[secrets.KEY]
provider = "env"
ref = "LOCAL_VAR"
"#,
        );

        let cfg = DotVaultConfig::load_from_dir(dir.path()).unwrap();
        // local should be used (full replacement)
        assert_eq!(
            cfg.secrets["KEY"].extra["ref"].as_str().unwrap(),
            "LOCAL_VAR"
        );
    }

    #[test]
    fn test_error_on_no_config() {
        let dir = TempDir::new().unwrap();
        let result = DotVaultConfig::load_from_dir(dir.path());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("no config file found"));
    }

    #[test]
    fn test_merge_global_providers_project_takes_precedence() {
        let mut project = DotVaultConfig {
            providers: {
                let mut m = HashMap::new();
                let mut aws = HashMap::new();
                aws.insert(
                    "region".to_string(),
                    toml::Value::String("us-east-1".to_string()),
                );
                m.insert("aws".to_string(), aws);
                m
            },
            secrets: HashMap::new(),
        };

        let mut global_aws = HashMap::new();
        global_aws.insert(
            "region".to_string(),
            toml::Value::String("eu-west-1".to_string()),
        );
        global_aws.insert(
            "profile".to_string(),
            toml::Value::String("global-profile".to_string()),
        );

        let global = DotVaultConfig {
            providers: {
                let mut m = HashMap::new();
                m.insert("aws".to_string(), global_aws);
                m
            },
            secrets: HashMap::new(),
        };

        // Simulate what merge_global_providers does
        for (provider_name, global_cfg) in global.providers {
            let project_cfg = project.providers.entry(provider_name).or_default();
            for (key, value) in global_cfg {
                project_cfg.entry(key).or_insert(value);
            }
        }

        // Project region takes precedence
        assert_eq!(
            project.providers["aws"]["region"].as_str().unwrap(),
            "us-east-1"
        );
        // Global-only key is filled in
        assert_eq!(
            project.providers["aws"]["profile"].as_str().unwrap(),
            "global-profile"
        );
    }
}
