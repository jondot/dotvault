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

const ENV_DETECTION_ORDER: &[&str] = &[
    "DOTVAULT_ENV",
    "NODE_ENV",
    "RAILS_ENV",
    "APP_ENV",
    "RACK_ENV",
];

fn detect_environment() -> String {
    detect_environment_with(&|key| std::env::var(key).ok())
}

fn detect_environment_with(lookup: &dyn Fn(&str) -> Option<String>) -> String {
    for var in ENV_DETECTION_ORDER {
        if let Some(val) = lookup(var) {
            let trimmed = val.trim().to_string();
            if !trimmed.is_empty() {
                return trimmed;
            }
        }
    }
    "development".to_string()
}

impl DotVaultConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read config file: {}", path.display()))?;
        let mut table: toml::Table = toml::from_str(&contents)
            .with_context(|| format!("failed to parse config file: {}", path.display()))?;

        // Identify environment sections: top-level keys (not "providers" or "secrets")
        // that contain a "secrets" sub-key.
        let env_names: Vec<String> = table
            .iter()
            .filter(|(k, v)| {
                *k != "providers"
                    && *k != "secrets"
                    && v.as_table().map_or(false, |t| t.contains_key("secrets"))
            })
            .map(|(k, _)| k.clone())
            .collect();

        let has_bare_secrets = table.contains_key("secrets");
        let has_env_sections = !env_names.is_empty();

        if has_env_sections {
            // Error if both [secrets] and [development.secrets] exist
            if has_bare_secrets && env_names.contains(&"development".to_string()) {
                anyhow::bail!(
                    "config has both [secrets] and [development.secrets]; \
                     use one or the other, not both"
                );
            }

            let env = detect_environment();

            // [secrets] is sugar for [development.secrets]
            let target_key = if env == "development" && has_bare_secrets {
                "secrets".to_string()
            } else {
                env.clone()
            };

            if target_key == "secrets" {
                // Already in the right shape — bare [secrets] is the development env
            } else {
                // Extract [env].secrets and promote it to top-level [secrets]
                let env_table = table.remove(&target_key).ok_or_else(|| {
                    let mut available: Vec<String> = env_names.clone();
                    if has_bare_secrets {
                        available.push("development".to_string());
                    }
                    anyhow::anyhow!(
                        "environment {:?} not found in config (available: {:?})",
                        env,
                        available
                    )
                })?;

                let env_map = env_table
                    .as_table()
                    .ok_or_else(|| anyhow::anyhow!("environment {:?} is not a table", env))?;
                let secrets = env_map
                    .get("secrets")
                    .ok_or_else(|| {
                        anyhow::anyhow!("environment {:?} has no [secrets] section", env)
                    })?
                    .clone();

                // Replace top-level secrets with the env-specific ones
                table.insert("secrets".to_string(), secrets);
            }

            // Remove all environment sections so serde doesn't choke
            for name in &env_names {
                table.remove(name);
            }
        }
        // If no env sections exist, use [secrets] as-is (backward compatible)

        let config: DotVaultConfig = toml::Value::Table(table)
            .try_into()
            .with_context(|| format!("failed to deserialize config: {}", path.display()))?;
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

        // Global config is not environment-aware — deserialize directly
        let contents = std::fs::read_to_string(&global_path)
            .with_context(|| format!("failed to read global config: {}", global_path.display()))?;
        let global: DotVaultConfig = toml::from_str(&contents)
            .with_context(|| format!("failed to parse global config: {}", global_path.display()))?;

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

    fn mock_env<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| {
            vars.iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn test_detect_environment_defaults_to_development() {
        let lookup = mock_env(&[]);
        assert_eq!(detect_environment_with(&lookup), "development");
    }

    #[test]
    fn test_detect_environment_dotvault_env_wins() {
        let lookup = mock_env(&[("DOTVAULT_ENV", "staging"), ("NODE_ENV", "production")]);
        assert_eq!(detect_environment_with(&lookup), "staging");
    }

    #[test]
    fn test_detect_environment_node_env() {
        let lookup = mock_env(&[("NODE_ENV", "production")]);
        assert_eq!(detect_environment_with(&lookup), "production");
    }

    #[test]
    fn test_detect_environment_trims_whitespace() {
        let lookup = mock_env(&[("DOTVAULT_ENV", "  staging  ")]);
        assert_eq!(detect_environment_with(&lookup), "staging");
    }

    #[test]
    fn test_detect_environment_skips_empty() {
        let lookup = mock_env(&[("DOTVAULT_ENV", ""), ("NODE_ENV", "production")]);
        assert_eq!(detect_environment_with(&lookup), "production");
    }

    #[test]
    fn test_detect_environment_skips_whitespace_only() {
        let lookup = mock_env(&[("DOTVAULT_ENV", "   "), ("NODE_ENV", "staging")]);
        assert_eq!(detect_environment_with(&lookup), "staging");
    }

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
    fn test_env_aware_loading_selects_production() {
        let dir = TempDir::new().unwrap();
        write_file(
            &dir,
            ".dotvault.toml",
            r#"
[secrets.API_KEY]
provider = "env"
ref = "DEV_KEY"

[production.secrets.API_KEY]
provider = "env"
ref = "PROD_KEY"
"#,
        );

        unsafe { std::env::set_var("DOTVAULT_ENV", "production"); }
        let cfg = DotVaultConfig::load_from_dir(dir.path()).unwrap();
        unsafe { std::env::remove_var("DOTVAULT_ENV"); }

        assert_eq!(cfg.secrets["API_KEY"].provider, "env");
        assert_eq!(
            cfg.secrets["API_KEY"].extra["ref"].as_str().unwrap(),
            "PROD_KEY"
        );
    }

    #[test]
    fn test_env_aware_loading_defaults_to_development() {
        let dir = TempDir::new().unwrap();
        write_file(
            &dir,
            ".dotvault.toml",
            r#"
[secrets.API_KEY]
provider = "env"
ref = "DEV_KEY"

[production.secrets.API_KEY]
provider = "env"
ref = "PROD_KEY"
"#,
        );

        for var in ["DOTVAULT_ENV", "NODE_ENV", "RAILS_ENV", "APP_ENV", "RACK_ENV"] {
            unsafe { std::env::remove_var(var); }
        }

        let cfg = DotVaultConfig::load_from_dir(dir.path()).unwrap();
        assert_eq!(
            cfg.secrets["API_KEY"].extra["ref"].as_str().unwrap(),
            "DEV_KEY"
        );
    }

    #[test]
    fn test_bare_secrets_only_ignores_env_detection() {
        let dir = TempDir::new().unwrap();
        write_file(
            &dir,
            ".dotvault.toml",
            r#"
[secrets.API_KEY]
provider = "env"
ref = "MY_KEY"
"#,
        );

        unsafe { std::env::set_var("NODE_ENV", "production"); }
        let cfg = DotVaultConfig::load_from_dir(dir.path()).unwrap();
        unsafe { std::env::remove_var("NODE_ENV"); }

        assert_eq!(
            cfg.secrets["API_KEY"].extra["ref"].as_str().unwrap(),
            "MY_KEY"
        );
    }

    #[test]
    fn test_missing_environment_errors_when_env_sections_exist() {
        let dir = TempDir::new().unwrap();
        write_file(
            &dir,
            ".dotvault.toml",
            r#"
[secrets.API_KEY]
provider = "env"
ref = "DEV_KEY"

[production.secrets.API_KEY]
provider = "env"
ref = "PROD_KEY"
"#,
        );

        unsafe { std::env::set_var("DOTVAULT_ENV", "qa"); }
        let result = DotVaultConfig::load_from_dir(dir.path());
        unsafe { std::env::remove_var("DOTVAULT_ENV"); }

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("qa"));
        assert!(msg.contains("not found"));
    }

    #[test]
    fn test_both_secrets_and_development_secrets_errors() {
        let dir = TempDir::new().unwrap();
        write_file(
            &dir,
            ".dotvault.toml",
            r#"
[secrets.API_KEY]
provider = "env"
ref = "DEV_KEY"

[development.secrets.API_KEY]
provider = "env"
ref = "DEV_KEY_2"
"#,
        );

        for var in ["DOTVAULT_ENV", "NODE_ENV", "RAILS_ENV", "APP_ENV", "RACK_ENV"] {
            unsafe { std::env::remove_var(var); }
        }

        let result = DotVaultConfig::load_from_dir(dir.path());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("[secrets]") || msg.contains("[development.secrets]"));
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

    #[test]
    fn test_providers_shared_across_environments() {
        let dir = TempDir::new().unwrap();
        write_file(
            &dir,
            ".dotvault.toml",
            r#"
[providers.my-env]
type = "env"

[secrets.API_KEY]
provider = "my-env"
ref = "DEV_KEY"

[production.secrets.API_KEY]
provider = "my-env"
ref = "PROD_KEY"
"#,
        );

        unsafe { std::env::set_var("DOTVAULT_ENV", "production"); }
        let cfg = DotVaultConfig::load_from_dir(dir.path()).unwrap();
        unsafe { std::env::remove_var("DOTVAULT_ENV"); }

        // Providers are preserved
        assert!(cfg.providers.contains_key("my-env"));
        assert_eq!(
            cfg.providers["my-env"]["type"].as_str().unwrap(),
            "env"
        );
        // Correct secrets selected
        assert_eq!(
            cfg.secrets["API_KEY"].extra["ref"].as_str().unwrap(),
            "PROD_KEY"
        );
    }

    #[test]
    fn test_full_env_aware_config_with_multiple_environments() {
        let dir = TempDir::new().unwrap();
        write_file(
            &dir,
            ".dotvault.toml",
            r#"
[providers.hashi]
type = "hashicorp"
address = "https://vault.staging.example.com"

[secrets]
API_KEY = { provider = "env", ref = "DEV_API_KEY" }
DB_URL = { provider = "env", ref = "DEV_DB_URL" }

[production.secrets]
API_KEY = { provider = "env", ref = "PROD_API_KEY" }
DB_URL = { provider = "env", ref = "PROD_DB_URL" }

[staging.secrets]
API_KEY = { provider = "hashi", ref = "secret/api", field = "key" }
DB_URL = { provider = "hashi", ref = "secret/db", field = "url" }
"#,
        );

        // Test development (default)
        for var in ["DOTVAULT_ENV", "NODE_ENV", "RAILS_ENV", "APP_ENV", "RACK_ENV"] {
            unsafe { std::env::remove_var(var); }
        }
        let cfg = DotVaultConfig::load_from_dir(dir.path()).unwrap();
        assert_eq!(cfg.secrets.len(), 2);
        assert_eq!(
            cfg.secrets["API_KEY"].extra["ref"].as_str().unwrap(),
            "DEV_API_KEY"
        );

        // Test production
        unsafe { std::env::set_var("NODE_ENV", "production"); }
        let cfg = DotVaultConfig::load_from_dir(dir.path()).unwrap();
        unsafe { std::env::remove_var("NODE_ENV"); }
        assert_eq!(
            cfg.secrets["API_KEY"].extra["ref"].as_str().unwrap(),
            "PROD_API_KEY"
        );
        assert_eq!(
            cfg.secrets["DB_URL"].extra["ref"].as_str().unwrap(),
            "PROD_DB_URL"
        );

        // Test staging
        unsafe { std::env::set_var("DOTVAULT_ENV", "staging"); }
        let cfg = DotVaultConfig::load_from_dir(dir.path()).unwrap();
        unsafe { std::env::remove_var("DOTVAULT_ENV"); }
        assert_eq!(cfg.secrets["API_KEY"].provider, "hashi");
        assert_eq!(
            cfg.secrets["API_KEY"].extra["ref"].as_str().unwrap(),
            "secret/api"
        );
    }
}
